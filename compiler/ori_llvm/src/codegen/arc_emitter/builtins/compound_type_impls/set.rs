//! Set equality and hash projection.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// `Set<T>.equals(other) -> bool`, independent of bucket order.
    pub(crate) fn emit_set_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        set_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_eq");
        let llvm_set_ty = self.resolve_type(set_ty);
        let lhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "seq.lhs", llvm_set_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "seq.rhs", llvm_set_ty);
        self.builder.store(rhs, rhs_ptr);

        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;

        self.emit_rt_call(
            func_id,
            &[lhs_ptr, rhs_ptr, elem_size, elem_eq, elem_hash],
            "set_eq",
        )
    }

    /// `Set<T>.hash() -> int`, using the evaluator's order-independent XOR.
    pub(crate) fn emit_set_hash(
        &mut self,
        receiver: ValueId,
        set_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_hash");
        let llvm_set_ty = self.resolve_type(set_ty);
        let receiver_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "shash.receiver", llvm_set_ty);
        self.builder.store(receiver, receiver_ptr);

        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;

        self.emit_rt_call(func_id, &[receiver_ptr, elem_size, elem_hash], "set_hash")
    }
}
