//! Internal protocol intercepts for ARC IR function calls.
//!
//! These are pseudo-calls emitted by the ARC lowering pipeline that don't
//! correspond to user-visible functions. Each protocol has a specific
//! calling convention and result type that differs from the standard
//! Apply emission path.
//!
//! Protocol builtins are dispatched via [`ProtocolBuiltin::from_name()`],
//! ensuring an exhaustive match — adding a new variant to the enum forces
//! a handler here. `ori_list_take` is NOT a protocol builtin (it's a real
//! runtime function with special sret handling).
//!
//! | Protocol              | Purpose                                  | Intercepted?     |
//! |-----------------------|------------------------------------------|------------------|
//! | `__iter_next`         | For-loop iteration protocol              | Yes              |
//! | `__collect_set`       | Collect iterator into `Set<T>`           | Yes              |
//! | `__index`             | `receiver[index]` desugaring             | Yes              |
//! | `__cast`              | `value as Target` conversion             | Yes (supported pairs) |
//! | `iter`                | Iterator creation (borrow inference only) | No (normal call) |
//! | `ori_iter_drop`       | Iterator cleanup (borrow inference only)  | No (normal call) |
//! | `ori_list_take`       | For-yield list finalization (non-PB)     | Yes (non-PB)     |
//! | `ori_list_slice_drop` | List rest pattern (non-PB)               | Yes (non-PB)     |

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
use ori_ir::{Name, FIELD_CAP, FIELD_DATA, FIELD_LEN};

use super::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Try to handle an internal protocol call.
    ///
    /// Returns `true` if the callee was a recognized protocol and was emitted
    /// (or silently skipped for unsupported types). Returns `false` if the
    /// callee is not a protocol and should go through normal dispatch.
    pub(super) fn try_emit_protocol(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        // Central protocol dispatch — exhaustive match enforced by compiler.
        // Adding a new ProtocolBuiltin variant will cause a compile error here.
        // `from_name` is the registry's str-keyed accessor; identity checks
        // below use pre-interned `Name`s per interning discipline.
        if let Some(protocol) = ProtocolBuiltin::from_name(self.interner.lookup(callee)) {
            // IterNext uses decomposed emission (tag + scratch pointer separately)
            // to avoid building an intermediate {i64, T} wrapper struct.
            if matches!(protocol, ProtocolBuiltin::IterNext) {
                assert!(
                    args.len() >= 2,
                    "__iter_next requires 2 args, got {}",
                    args.len()
                );
                let iter_ptr = self.var(args[0]);
                let elem_ty = func.var_type(args[1]);
                if let Some((tag, scratch, elem_llvm_ty)) = self.emit_iter_next(iter_ptr, elem_ty) {
                    self.iter_next_decomposed
                        .insert(dst, (tag, scratch, elem_llvm_ty));
                    self.def_var(dst, super::context::EmittedValue::Immediate(tag));
                }
                return true;
            }

            // Iter and IterDrop are registered as ProtocolBuiltin for borrow
            // inference only — they go through normal function dispatch
            // (real runtime calls), not protocol intercept.
            if !protocol.is_intercepted() {
                return false;
            }

            let result = match protocol {
                ProtocolBuiltin::Cast => {
                    // __cast(value) — `as` conversions (ARC IR has no cast
                    // instruction). Supported primitive pairs emit inline
                    // (incl. range-checked int->byte / int->char), matching
                    // `ori_eval::eval_can_cast` for dual-execution parity.
                    // Unsupported pairs (str->int / str->float parse,
                    // value->str) fall through to normal dispatch
                    // (unresolved-function error).
                    return match self.try_emit_cast(dst, args, func) {
                        Some(val) => {
                            self.def_var_repr(dst, val, func);
                            true
                        }
                        None => false,
                    };
                }

                ProtocolBuiltin::CollectSet => {
                    // __collect_set(iter).
                    // Type-directed rewrite from collect() when target type is Set<T>.
                    assert!(!args.is_empty(), "__collect_set requires at least 1 arg");
                    let iter_ptr = self.var(args[0]);
                    let iter_ty = func.var_type(args[0]);
                    let elem_ty = self.pool.iterator_elem(iter_ty);
                    self.emit_iter_collect_set(iter_ptr, elem_ty)
                }
                ProtocolBuiltin::Index => {
                    // __index(receiver, index).
                    // Desugared from `receiver[index]` by ARC lowering.
                    // List: returns T directly (panics OOB). Map: returns Option<V>.
                    assert!(
                        args.len() >= 2,
                        "__index requires 2 args, got {}",
                        args.len()
                    );
                    let receiver_ty = func.var_type(args[0]);
                    let type_info = self.type_info.get(receiver_ty);
                    let recv = self.var(args[0]);
                    let idx = self.var(args[1]);
                    match &type_info {
                        TypeInfo::List { element } => {
                            self.emit_list_index(recv, idx, *element, receiver_ty)
                        }
                        TypeInfo::Map { key, value } => {
                            self.emit_map_get(recv, idx, *key, *value, Some(receiver_ty))
                        }
                        _ => {
                            tracing::warn!(
                                ?type_info,
                                "__index on unsupported type — type checker should prevent this"
                            );
                            None
                        }
                    }
                }
                // IterNext handled above via early return.
                ProtocolBuiltin::IterNext => unreachable!("IterNext handled above"),
                // Filtered out by the is_intercepted() check above.
                ProtocolBuiltin::Iter | ProtocolBuiltin::IterDrop => {
                    unreachable!("Iter/IterDrop filtered by is_intercepted() above")
                }
            };
            if let Some(val) = result {
                self.def_var_repr(dst, val, func);
            }
            return true;
        }

        // ori_list_take: NOT a protocol builtin — it's a real runtime function
        // with special sret handling (has `ori_` prefix, exists in RT_FUNCTIONS).
        if callee == self.list_rt_names.take && !args.is_empty() {
            if let Some(val) = self.emit_list_take(dst, args[0], func) {
                self.def_var_repr(dst, val, func);
            }
            return true;
        }

        // ori_list_slice_drop: emitted by decision tree ListRest path resolution.
        // args[0] = list struct, args[1] = start index (int constant).
        // Expands to ori_list_slice_drop(data, len, cap, n, elem_size, out_ptr).
        if callee == self.list_rt_names.slice_drop && args.len() >= 2 {
            let list_ty = func.var_type(args[0]);
            if let Some(val) = self.emit_list_slice_drop(args[0], args[1], list_ty, func) {
                self.def_var_repr(dst, val, func);
            }
            return true;
        }

        false
    }

    /// Emit `ori_list_slice_drop(data, len, cap, n, elem_size, out_ptr)` for
    /// list rest patterns (`[a, b, ..rest]`).
    ///
    /// Extracts the list's data/len/cap components, computes the element size
    /// from the list type, then calls the runtime function with sret output.
    fn emit_list_slice_drop(
        &mut self,
        list_var: ArcVarId,
        start_var: ArcVarId,
        list_ty: ori_types::Idx,
        _func: &ArcFunction,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_slice_drop");
        let list_val = self.var(list_var);
        let start_val = self.var(start_var);

        // Extract list components: {len, cap, data}
        let len = self
            .builder
            .extract_value(list_val, FIELD_LEN, "slice.len")?;
        let cap = self
            .builder
            .extract_value(list_val, FIELD_CAP, "slice.cap")?;
        let data = self
            .builder
            .extract_value(list_val, FIELD_DATA, "slice.data")?;

        // Compute element size from the list's element type
        // (narrowed element size when available). A non-List receiver on the
        // ListRest slice-drop path is a phase-contract violation — a silent
        // element-size fallback would feed a wrong stride into the runtime's
        // buffer walk (memory corruption), so fail loud instead.
        let resolved = self.pool.resolve_fully(list_ty);
        assert!(
            self.pool.tag(resolved) == ori_types::Tag::List,
            "ori_list_slice_drop receiver must resolve to Tag::List (got {:?})",
            self.pool.tag(resolved)
        );
        let elem_ty = self.pool.list_elem(resolved);
        let elem_size = self.collection_elem_size(resolved, elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_struct_ty = self.fat_ptr_llvm_type();
        self.call_with_manual_sret_out(
            func_id,
            &[data, len, cap, start_val, elem_size_val],
            list_struct_ty,
            "slice_drop",
        )
    }

    /// Emit `ori_list_take(list_ptr, out_ptr)` with manual sret handling.
    ///
    /// `ori_list_take` uses an explicit sret pattern: `void(ptr list, ptr out)`.
    /// We alloca a `{i64, i64, ptr}` result, call the function, then load.
    ///
    /// The moved-out result buffer's V5 RC header carries no `elem_dec_fn` —
    /// `ori_list_take` moves the data buffer out without populating it. When the
    /// for-yield result is freed solely via `IterState::Drop` (the iter-consumed
    /// shape, caller `elem_dec_fn = None`), the null header runs zero element
    /// cleanup and heap elements leak. Store `elem_dec_fn` + `elem_count` on the
    /// result here, mirroring the Construct List / collect-consumer finalizers
    /// (CG:RT-4 `elem_dec_fn` contract). The element type is derived from the
    /// RESULT `dst`'s `[T]` type — `list_var` is the scratch `OriList` handle
    /// (`Tag::Int` in ARC IR) and would mis-type the dec fn.
    fn emit_list_take(
        &mut self,
        dst: ArcVarId,
        list_var: ArcVarId,
        func: &ArcFunction,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_take");
        let list_ptr = self.var(list_var);

        let list_struct_ty = self.fat_ptr_llvm_type();
        let result =
            self.call_with_manual_sret_out(func_id, &[list_ptr], list_struct_ty, "list_take")?;

        // Populate the result buffer's V5 elem header from the dst `[T]` type.
        // `get_or_generate_elem_dec_fn` returns a null pointer for scalar
        // elements (CG:RT-4), so `[int]`/`[byte]` results store a NULL
        // elem_dec_fn — no spurious per-element dec fn is attached.
        let dst_ty = func.var_type(dst);
        let resolved = self.pool.resolve_fully(dst_ty);
        if self.pool.tag(resolved) == ori_types::Tag::List {
            let elem_ty = self.pool.list_elem(resolved);
            let result_data = self
                .builder
                .extract_value(result, FIELD_DATA, "list_take.data")
                .unwrap_or_else(|| self.builder.const_null_ptr());
            let result_len = self
                .builder
                .extract_value(result, FIELD_LEN, "list_take.len")
                .unwrap_or_else(|| self.builder.const_i64(0));
            let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
            let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
            self.builder
                .call(store_dec, &[result_data, elem_dec_fn], "");
            let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
            self.builder
                .call(store_count, &[result_data, result_len], "");
        }

        Some(result)
    }

    /// The canonical `{ i64, i64, ptr }` fat-pointer LLVM type — the runtime
    /// ABI shape (RT-6) `ori_list_take` / `ori_list_slice_drop` write through
    /// their out-pointer, independent of the receiver's repr. Single
    /// definition site for the manual-sret result layout.
    fn fat_ptr_llvm_type(&mut self) -> crate::codegen::value_id::LLVMTypeId {
        let scx = self.builder.scx();
        let st = scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        );
        self.builder.register_type(st.into())
    }

    /// Call a runtime function using the manual-sret convention: the result
    /// pointer is appended as the LAST argument (`void(args..., ptr out)`),
    /// then the result struct is loaded from the alloca.
    fn call_with_manual_sret_out(
        &mut self,
        func_id: crate::codegen::value_id::FunctionId,
        args: &[ValueId],
        result_ty: crate::codegen::value_id::LLVMTypeId,
        label: &str,
    ) -> Option<ValueId> {
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            &format!("{label}.out"),
            result_ty,
        );
        let mut full_args = Vec::with_capacity(args.len() + 1);
        full_args.extend_from_slice(args);
        full_args.push(out_alloca);
        self.builder.call(func_id, &full_args, label);
        Some(
            self.builder
                .load(result_ty, out_alloca, &format!("{label}.val")),
        )
    }
}
