//! Internal protocol intercepts for ARC IR function calls.
//!
//! These are pseudo-calls emitted by the ARC lowering pipeline that don't
//! correspond to user-visible functions. Each protocol has a specific
//! calling convention and result type that differs from the standard
//! Apply emission path.
//!
//! | Protocol         | Purpose                                  | Result type      |
//! |------------------|------------------------------------------|------------------|
//! | `__iter_next`    | For-loop iteration protocol              | `{i64, T}`       |
//! | `__collect_set`  | Collect iterator into `Set<T>`           | `{i64, i64, ptr}`|
//! | `ori_list_take`  | For-yield list finalization (explicit sret)| `{i64, i64, ptr}`|
//! | `__index`        | `receiver[index]` desugaring             | `T` or `Option<V>`|

use ori_arc::ir::{ArcFunction, ArcVarId};

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
        match callee_name {
            // Internal protocol: __iter_next(iter, elem_ty_marker).
            // args[0] = iterator pointer, args[1] = zero marker carrying elem_ty.
            // Result type is INT (no RC management); actual element type comes
            // from the marker argument.
            "__iter_next" if args.len() >= 2 => {
                let iter_ptr = self.var(args[0]);
                let elem_ty = func.var_type(args[1]);
                if let Some(val) = self.emit_iter_next(iter_ptr, elem_ty) {
                    self.def_var_repr(dst, val, func);
                }
                true
            }

            // Internal protocol: __collect_set(iter).
            // Type-directed rewrite from `collect()` when target type is Set<T>.
            // Uses sret pattern like emit_iter_collect but calls ori_iter_collect_set.
            "__collect_set" if !args.is_empty() => {
                let iter_ptr = self.var(args[0]);
                let iter_ty = func.var_type(args[0]);
                let elem_ty = self.pool.iterator_elem(iter_ty);
                if let Some(val) = self.emit_iter_collect_set(iter_ptr, elem_ty) {
                    self.def_var_repr(dst, val, func);
                }
                true
            }

            // ori_list_take uses explicit sret pattern: void(list_ptr, out_ptr).
            // The ARC IR emits Apply "ori_list_take"(list_ptr) expecting a struct return.
            // We handle the sret plumbing here: alloca result struct, call, load.
            "ori_list_take" if !args.is_empty() => {
                if let Some(val) = self.emit_list_take(args[0], func) {
                    self.def_var_repr(dst, val, func);
                }
                true
            }

            // Internal protocol: __index(receiver, index).
            // Desugared from `receiver[index]` by ARC lowering.
            // List: returns T directly (panics OOB). Map: returns Option<V>.
            "__index" if args.len() >= 2 => {
                let receiver_ty = func.var_type(args[0]);
                let type_info = self.type_info.get(receiver_ty);
                let recv = self.var(args[0]);
                let idx = self.var(args[1]);
                let result = match &type_info {
                    TypeInfo::List { element } => self.emit_list_index(recv, idx, *element),
                    TypeInfo::Map { key, value } => self.emit_map_get(recv, idx, *key, *value),
                    _ => {
                        tracing::warn!(
                            ?type_info,
                            "__index on unsupported type — type checker should prevent this"
                        );
                        None
                    }
                };
                if let Some(val) = result {
                    self.def_var_repr(dst, val, func);
                }
                true
            }

            _ => false,
        }
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
