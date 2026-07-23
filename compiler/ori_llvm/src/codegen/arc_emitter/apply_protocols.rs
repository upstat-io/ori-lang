//! Internal ARC protocol-call intercepts.
//!
//! Pseudo-calls with nonstandard results or ABIs bypass ordinary Apply dispatch.
//! Exhaustive [`ProtocolBuiltin`] dispatch covers intercepted registry entries;
//! list finalization runtime calls use a separate manual-sret path.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
use ori_ir::Name;

use super::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Try to handle an internal protocol call.
    ///
    /// Returns `true` if the callee was a recognized protocol and was emitted.
    /// Returns `false` if the callee is not a protocol and should go through
    /// normal dispatch.
    pub(super) fn try_emit_protocol(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        // INVARIANT: Exhaustive dispatch makes new protocol variants compiler-checked.
        if let Some(protocol) = ProtocolBuiltin::from_name(self.interner.lookup(callee)) {
            // Why: IterNext keeps its tag and scratch pointer decomposed.
            if matches!(protocol, ProtocolBuiltin::IterNext) {
                self.emit_decomposed_iter_next(dst, args, func);
                return true;
            }

            // Why: Borrow inference registers Iter and IterDrop without intercepting them.
            if !protocol.is_intercepted() {
                return false;
            }

            let result = match protocol {
                ProtocolBuiltin::Cast => return self.try_emit_cast_protocol(dst, args, func),
                ProtocolBuiltin::CollectSet => require_protocol_result(
                    "__collect_set",
                    self.emit_collect_set_protocol(args, func),
                ),
                ProtocolBuiltin::Index => {
                    require_protocol_result("__index", self.emit_index_protocol(args, func))
                }
                ProtocolBuiltin::IterNext => unreachable!("IterNext uses decomposed emission"),
                ProtocolBuiltin::Iter | ProtocolBuiltin::IterDrop => {
                    unreachable!("Iter/IterDrop are not intercepted protocols")
                }
            };
            self.def_var_repr(dst, result, func);
            return true;
        }

        // Why: `ori_list_take` is a runtime function with a nonstandard sret ABI.
        if callee == self.list_rt_names.take && !args.is_empty() {
            let result =
                require_protocol_result("ori_list_take", self.emit_list_take(dst, args[0], func));
            self.def_var_repr(dst, result, func);
            return true;
        }

        if callee == self.list_rt_names.slice_drop && args.len() >= 2 {
            let list_ty = func.var_type(args[0]);
            let result = require_protocol_result(
                "ori_list_slice_drop",
                self.emit_list_slice_drop(args[0], args[1], list_ty, func),
            );
            self.def_var_repr(dst, result, func);
            return true;
        }

        false
    }

    fn emit_decomposed_iter_next(&mut self, dst: ArcVarId, args: &[ArcVarId], func: &ArcFunction) {
        assert!(
            args.len() >= 2,
            "__iter_next requires 2 args, got {}",
            args.len()
        );
        let iter_ptr = self.var(args[0]);
        let elem_ty = func.var_type(args[1]);
        let (tag, scratch, elem_llvm_ty) =
            require_protocol_result("__iter_next", self.emit_iter_next(iter_ptr, elem_ty));
        self.iter_next_decomposed
            .insert(dst, (tag, scratch, elem_llvm_ty));
        self.def_var(dst, super::context::EmittedValue::Immediate(tag));
    }

    /// Emit a supported `as` conversion, or leave unsupported conversions for
    /// normal dispatch so they retain the unresolved-function diagnostic.
    fn try_emit_cast_protocol(
        &mut self,
        dst: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        match self.try_emit_cast(dst, args, func) {
            Some(val) => {
                self.def_var_repr(dst, val, func);
                true
            }
            None => false,
        }
    }

    /// Emit the type-directed `collect()` rewrite for a `Set<T>` target.
    fn emit_collect_set_protocol(
        &mut self,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        assert!(!args.is_empty(), "__collect_set requires at least 1 arg");
        let iter_ptr = self.var(args[0]);
        let iter_ty = func.var_type(args[0]);
        let elem_ty = self.pool.iterator_elem(iter_ty);
        self.emit_iter_collect_set(iter_ptr, elem_ty)
    }

    /// Emit `receiver[index]` after ARC lowering has selected the index
    /// protocol. The receiver representation determines the runtime operation.
    fn emit_index_protocol(&mut self, args: &[ArcVarId], func: &ArcFunction) -> Option<ValueId> {
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
            TypeInfo::List { element } => self.emit_list_index(recv, idx, *element, receiver_ty),
            TypeInfo::Map { key, value } => {
                self.emit_map_get(recv, idx, *key, *value, Some(receiver_ty))
            }
            TypeInfo::Str => self.emit_str_index(recv, idx),
            unsupported => {
                self.builder.record_codegen_error_with_msg(format!(
                    "LLVM `__index` received non-indexable receiver {unsupported:?}; restrict \
                     index protocol lowering to List, Map, or Str before ARC code generation"
                ));
                None
            }
        }
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

        let (data, len, cap) =
            self.extract_collection_fields(list_val, "slice.data", "slice.len", "slice.cap")?;

        // INVARIANT: ListRest must supply a list; a fallback stride could corrupt memory.
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

    /// Emit `ori_list_take(list_ptr, out_ptr)` and attach result-buffer cleanup metadata.
    ///
    /// The element type comes from `dst`; `list_var` is an untyped scratch handle.
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

        // INVARIANT: Scalar elements carry a null destructor in the result header.
        let dst_ty = func.var_type(dst);
        let resolved = self.pool.resolve_fully(dst_ty);
        if self.pool.tag(resolved) == ori_types::Tag::List {
            let elem_ty = self.pool.list_elem(resolved);
            let (result_data, result_len) =
                self.extract_collection_data_and_len(result, "list_take.data", "list_take.len")?;
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

    /// Return the canonical `{ i64, i64, ptr }` manual-sret result type.
    /// Runtime list finalizers write this shape independent of receiver repr.
    pub(super) fn fat_ptr_llvm_type(&mut self) -> crate::codegen::value_id::LLVMTypeId {
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

    /// Call `void(args..., ptr out)` and load the manual-sret result.
    fn call_with_manual_sret_out(
        &mut self,
        func_id: crate::codegen::value_id::FunctionId,
        args: &[ValueId],
        result_ty: crate::codegen::value_id::LLVMTypeId,
        label: &str,
    ) -> Option<ValueId> {
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "manual.sret.out", result_ty);
        let full_arity = args.len().checked_add(1)?;
        let mut full_args = Vec::with_capacity(full_arity);
        full_args.extend_from_slice(args);
        full_args.push(out_alloca);
        self.builder.call(func_id, &full_args, label);
        Some(self.builder.load(result_ty, out_alloca, "manual.sret.val"))
    }
}

fn require_protocol_result<T>(protocol: &str, result: Option<T>) -> T {
    let Some(result) = result else {
        // Why: Intercepted protocols are validated with a concrete result layout before codegen.
        unreachable!(
            "LLVM `{protocol}` protocol emission produced no result; verify its receiver type and \
             result layout before ARC code generation"
        );
    };
    result
}

#[cfg(test)]
mod tests;
