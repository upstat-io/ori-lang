//! List transformations share the COW out-parameter runtime ABI.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `list.concat(other)` / `list.add(other)` — COW concatenation.
    ///
    /// Both lists are consumed (ownership transferred to the runtime). The
    /// runtime checks uniqueness at runtime to skip RC increments when the
    /// source list is uniquely owned.
    pub(crate) fn emit_list_concat_cow(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_concat_cow");

        let (data1, len1, cap1) = self.extract_list_fields(receiver)?;
        let (data2, len2, cap2) = self.extract_list_fields(other)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "concat",
            vec![
                data1,
                len1,
                cap1,
                data2,
                len2,
                cap2,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.reverse()` — COW reverse returning the reversed list.
    ///
    /// Fast path (unique): swaps pairs from both ends inward, O(n), no allocation.
    /// Slow path (shared): new allocation with elements in reverse order.
    pub(crate) fn emit_list_reverse_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_reverse_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "reverse",
            vec![
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.sort()` — COW sort returning the sorted list.
    ///
    /// Generates a comparison thunk function for the element type, then passes
    /// it to `ori_list_sort_cow`. The thunk has signature
    /// `fn(*const u8, *const u8) -> i32` and loads elements by type before
    /// comparing.
    ///
    /// Supports primitive element types (int, float, bool, char, byte, str).
    pub(crate) fn emit_list_sort_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_list_sort_with(
            "ori_list_sort_cow",
            "sort",
            receiver,
            elem_ty,
            cow_mode,
            list_ty,
        )
    }

    /// Emit a stable sort (`TimSort`) — preserves relative order of equal elements.
    /// Identical to `emit_list_sort_cow` but calls `ori_list_sort_stable_cow`.
    pub(crate) fn emit_list_sort_stable_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_list_sort_with(
            "ori_list_sort_stable_cow",
            "sort_stable",
            receiver,
            elem_ty,
            cow_mode,
            list_ty,
        )
    }

    fn emit_list_sort_with(
        &mut self,
        runtime_fn: &'static str,
        label: &'static str,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let compare_fn_ptr = self
            .get_or_create_narrowed_compare_thunk(list_ty)
            .or_else(|| self.get_or_create_compare_thunk(elem_ty))?;

        let func_id = self.builder.runtime_fn(runtime_fn);

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            label,
            vec![
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                compare_fn_ptr,
                inc_fn,
                cow_mode,
            ],
        )
    }
}
