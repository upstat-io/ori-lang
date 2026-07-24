//! Zero-copy list slicing operations.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Shared body for `emit_list_take_slice` / `emit_list_drop_slice` — both
    /// are a zero-copy N-element slice differing only in the runtime entry
    /// point and the emitted-value label.
    fn emit_list_take_or_drop_slice(
        &mut self,
        runtime_fn_name: &'static str,
        label: &str,
        receiver: ValueId,
        n: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(runtime_fn_name);

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out =
            self.builder
                .create_entry_alloca(self.current_function, "list.slice.out", list_ty);

        self.emit_rt_call(func_id, &[data_ptr, len, cap, n, elem_size_val, out], label);

        Some(self.builder.load(list_ty, out, "list.slice.val"))
    }

    /// Emit `list.take(n)` — zero-copy first-N elements slice.
    ///
    /// Equivalent to `list.slice(0, n)`. No elements are copied.
    pub(crate) fn emit_list_take_slice(
        &mut self,
        receiver: ValueId,
        n: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_list_take_or_drop_slice(
            "ori_list_slice_take",
            "take",
            receiver,
            n,
            elem_ty,
            list_ty,
        )
    }

    /// Emit `list.drop(n)` — zero-copy skip-first-N elements slice.
    ///
    /// Equivalent to `list.slice(n, len)`. No elements are copied.
    pub(crate) fn emit_list_drop_slice(
        &mut self,
        receiver: ValueId,
        n: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_list_take_or_drop_slice(
            "ori_list_slice_drop",
            "drop",
            receiver,
            n,
            elem_ty,
            list_ty,
        )
    }
}
