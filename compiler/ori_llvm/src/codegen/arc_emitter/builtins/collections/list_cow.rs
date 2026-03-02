//! COW (Copy-on-Write) mutation codegen for lists.
//!
//! All list mutation methods use COW semantics: when the list is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each method returns a `{i64 len, i64 cap, ptr data}` struct.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `list.push(x)` — COW push returning the (possibly mutated) list.
    ///
    /// Fast path (unique + capacity): appends in place, O(1).
    /// Slow path (shared): copies to new buffer, O(n).
    pub(crate) fn emit_list_push_cow(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_push_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "push.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
                out,
            ],
            "push",
        );

        Some(self.builder.load(list_ty, out, "push.val"))
    }

    /// Emit `list.pop()` — COW pop returning the list with last element removed.
    ///
    /// Fast path (unique): decrements len in place, O(1).
    /// Slow path (shared): copies to new buffer with len-1 elements.
    #[expect(dead_code, reason = "pop dispatch not yet wired — see task #3")]
    pub(crate) fn emit_list_pop_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_pop_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "pop.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
                out,
            ],
            "pop",
        );

        Some(self.builder.load(list_ty, out, "pop.val"))
    }

    /// Emit `list.set(index, value)` — COW index assignment returning modified list.
    ///
    /// Fast path (unique): overwrites element at index in place.
    /// Slow path (shared): copies buffer, overwrites target index.
    pub(crate) fn emit_list_set_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_set_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "set.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "set.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
                out,
            ],
            "set",
        );

        Some(self.builder.load(list_ty, out, "set.val"))
    }

    /// Emit `list.insert(index, value)` — COW insert returning modified list.
    ///
    /// Fast path (unique + capacity): memmove + write in place.
    /// Slow path (shared): new allocation with element inserted.
    pub(crate) fn emit_list_insert_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_insert_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "insert.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
                out,
            ],
            "insert",
        );

        Some(self.builder.load(list_ty, out, "insert.val"))
    }

    /// Emit `list.remove(index)` — COW remove returning modified list.
    ///
    /// Fast path (unique): memmove shift left in place.
    /// Slow path (shared): new allocation without the removed element.
    pub(crate) fn emit_list_remove_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_remove_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "remove.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                index,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
                out,
            ],
            "remove",
        );

        Some(self.builder.load(list_ty, out, "remove.val"))
    }

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
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_concat_cow");

        let (data1, len1, cap1) = self.extract_list_fields(receiver);
        let (data2, len2, cap2) = self.extract_list_fields(other);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "concat.out", list_ty);

        self.builder.call(
            func_id,
            &[
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
                out,
            ],
            "concat",
        );

        Some(self.builder.load(list_ty, out, "concat.val"))
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
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_reverse_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "reverse.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
                out,
            ],
            "reverse",
        );

        Some(self.builder.load(list_ty, out, "reverse.val"))
    }

    /// Emit `list.sort()` — COW sort returning the sorted list.
    ///
    /// Generates a comparison thunk function for the element type, then passes
    /// it to `ori_list_sort_cow`. The thunk has signature
    /// `fn(*const u8, *const u8) -> i32` and loads elements by type before
    /// comparing.
    ///
    /// Currently supports primitive element types (int, float, bool, char, byte, str).
    pub(crate) fn emit_list_sort_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let compare_fn_ptr = self.get_or_create_compare_thunk(elem_ty)?;

        let func_id = self.builder.runtime_fn("ori_list_sort_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "sort.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                compare_fn_ptr,
                inc_fn,
                cow_mode,
                out,
            ],
            "sort",
        );

        Some(self.builder.load(list_ty, out, "sort.val"))
    }

    /// Emit a stable sort (`TimSort`) — preserves relative order of equal elements.
    /// Identical to `emit_list_sort_cow` but calls `ori_list_sort_stable_cow`.
    pub(crate) fn emit_list_sort_stable_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let compare_fn_ptr = self.get_or_create_compare_thunk(elem_ty)?;

        let func_id = self.builder.runtime_fn("ori_list_sort_stable_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out =
            self.builder
                .create_entry_alloca(self.current_function, "sort_stable.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                compare_fn_ptr,
                inc_fn,
                cow_mode,
                out,
            ],
            "sort_stable",
        );

        Some(self.builder.load(list_ty, out, "sort_stable.val"))
    }
}
