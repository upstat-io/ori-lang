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
//! | `iter`                | Iterator creation (borrow inference only) | No (normal call) |
//! | `ori_iter_drop`       | Iterator cleanup (borrow inference only)  | No (normal call) |
//! | `ori_list_take`       | For-yield list finalization (non-PB)     | Yes (non-PB)     |
//! | `ori_list_slice_drop` | List rest pattern (non-PB)               | Yes (non-PB)     |

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::builtin_constants::protocol::ProtocolBuiltin;

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
        callee_name: &str,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        // Central protocol dispatch — exhaustive match enforced by compiler.
        // Adding a new ProtocolBuiltin variant will cause a compile error here.
        if let Some(protocol) = ProtocolBuiltin::from_name(callee_name) {
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

            let result = match protocol {
                // Iter and IterDrop are registered as ProtocolBuiltin for
                // borrow inference only — they go through normal function
                // dispatch (real runtime calls), not protocol intercept.
                ProtocolBuiltin::Iter | ProtocolBuiltin::IterDrop => return false,

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
                        TypeInfo::List { element } => self.emit_list_index(recv, idx, *element),
                        TypeInfo::Map { key, value } => self.emit_map_get(recv, idx, *key, *value),
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
            };
            if let Some(val) = result {
                self.def_var_repr(dst, val, func);
            }
            return true;
        }

        // ori_list_take: NOT a protocol builtin — it's a real runtime function
        // with special sret handling (has `ori_` prefix, exists in RT_FUNCTIONS).
        if callee_name == "ori_list_take" && !args.is_empty() {
            if let Some(val) = self.emit_list_take(args[0], func) {
                self.def_var_repr(dst, val, func);
            }
            return true;
        }

        // ori_list_slice_drop: emitted by decision tree ListRest path resolution.
        // args[0] = list struct, args[1] = start index (int constant).
        // Expands to ori_list_slice_drop(data, len, cap, n, elem_size, out_ptr).
        if callee_name == "ori_list_slice_drop" && args.len() >= 2 {
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
        let len = self.builder.extract_value(list_val, 0, "slice.len")?;
        let cap = self.builder.extract_value(list_val, 1, "slice.cap")?;
        let data = self.builder.extract_value(list_val, 2, "slice.data")?;

        // Compute element size from the list's element type
        let resolved = self.pool.resolve_fully(list_ty);
        let elem_ty = if self.pool.tag(resolved) == ori_types::Tag::List {
            self.pool.list_elem(resolved)
        } else {
            ori_types::Idx::INT // fallback
        };
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Alloca {i64, i64, ptr} for the sret result
        let list_struct_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            "slice_drop.out",
            list_struct_ty,
        );

        // Call ori_list_slice_drop(data, len, cap, n, elem_size, out_ptr)
        self.builder.call(
            func_id,
            &[data, len, cap, start_val, elem_size_val, out_alloca],
            "slice_drop",
        );

        // Load the result struct
        Some(
            self.builder
                .load(list_struct_ty, out_alloca, "slice_drop.val"),
        )
    }

    /// Emit `ori_list_take(list_ptr, out_ptr)` with manual sret handling.
    ///
    /// `ori_list_take` uses an explicit sret pattern: `void(ptr list, ptr out)`.
    /// We alloca a `{i64, i64, ptr}` result, call the function, then load.
    fn emit_list_take(&mut self, list_var: ArcVarId, _func: &ArcFunction) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_take");
        let list_ptr = self.var(list_var);

        // Alloca {i64, i64, ptr} for the result
        let list_struct_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            "list_take.out",
            list_struct_ty,
        );

        // Call ori_list_take(list_ptr, out_alloca) — void return
        self.builder
            .call(func_id, &[list_ptr, out_alloca], "list_take");

        // Load the result struct from the alloca
        Some(
            self.builder
                .load(list_struct_ty, out_alloca, "list_take.val"),
        )
    }
}
