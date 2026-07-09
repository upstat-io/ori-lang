//! COW (Copy-on-Write) mutation codegen for lists.
//!
//! All list mutation methods use COW semantics: when the list is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each method returns a `{i64 len, i64 cap, ptr data}` struct.

use ori_ir::{FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::value_id::{FunctionId, ValueId};

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Shared out-param `sret`-style runtime-call tail for list COW mutation
    /// methods: allocates the `{i64,i64,ptr}` out slot, calls `func_id` with
    /// `args` followed by the out pointer, and loads the resulting list
    /// value. Every COW list method in this file shares this exact tail —
    /// only the runtime function and its argument list differ per method.
    fn emit_list_cow_call(
        &mut self,
        func_id: FunctionId,
        label: &str,
        mut args: Vec<ValueId>,
    ) -> Option<ValueId> {
        let list_struct_ty = self.list_struct_type();
        let out = self.builder.create_entry_alloca(
            self.current_function,
            &format!("{label}.out"),
            list_struct_ty,
        );
        args.push(out);
        self.emit_rt_call(func_id, &args, label);
        Some(
            self.builder
                .load(list_struct_ty, out, &format!("{label}.val")),
        )
    }

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
        list_ty: Idx,
        receiver_returned: bool,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_push_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let result = self.emit_list_cow_call(
            func_id,
            "push",
            vec![
                data_ptr,
                len,
                cap,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )?;
        // Store elem_dec_fn and elem_count in the result buffer's RC header —
        // ONLY for a RETURNED receiver lineage (the Phase-6.68b element-escape
        // keep-alive's balancing release is this collection's `elem_dec_fn` run
        // by the CALLER's drop; the runtime slow path cannot propagate a header
        // when the source list is empty/null). An in-scope receiver holds
        // UNFUNDED element views (the base accounting balances the source
        // iter-drop against it) and MUST NOT dec them at free — no header.
        // Same store discipline as the collect / list_take result sites.
        // Env: ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE — restores the header-less
        // push-grown buffer for bisection, debug-only
        if !receiver_returned
            || std::env::var_os("ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE").is_some()
        {
            return Some(result);
        }
        let result_data = self
            .builder
            .extract_value(result, FIELD_DATA, "push.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let result_len = self
            .builder
            .extract_value(result, FIELD_LEN, "push.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec, &[result_data, elem_dec_fn], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder
            .call(store_count, &[result_data, result_len], "");
        Some(result)
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
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_pop_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "pop",
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
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_set_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "set.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "set",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.updated(key, value)` — COW replacement returning modified list
    /// (`IndexSet.updated`).
    ///
    /// Fast path (unique): releases the replaced element, overwrites in place.
    /// Slow path (shared): copies buffer, overwrites target index.
    /// Out-of-bounds keys PANIC in the runtime (matching `list[key]`).
    ///
    /// The value is MOVED into the list (`arg_ownership` marks it `Owned`, the
    /// runtime takes the caller's reference) — no caller-side `RcDec` follows.
    pub(crate) fn emit_list_updated_cow(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_updated_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "updated.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);
        let dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "updated",
            vec![
                data_ptr,
                len,
                cap,
                key,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                dec_fn,
                cow_mode,
            ],
        )
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
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_insert_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "insert",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
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
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_remove_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "remove",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
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
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_concat_cow");

        let (data1, len1, cap1) = self.extract_list_fields(receiver);
        let (data2, len2, cap2) = self.extract_list_fields(other);
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

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
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
    /// Currently supports primitive element types (int, float, bool, char, byte, str).
    pub(crate) fn emit_list_sort_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        // Use narrowed compare thunk for narrowed int lists.
        let compare_fn_ptr = self
            .get_or_create_narrowed_compare_thunk(elem_ty)
            .or_else(|| self.get_or_create_compare_thunk(elem_ty))?;

        let func_id = self.builder.runtime_fn("ori_list_sort_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "sort",
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

    /// Emit a stable sort (`TimSort`) — preserves relative order of equal elements.
    /// Identical to `emit_list_sort_cow` but calls `ori_list_sort_stable_cow`.
    pub(crate) fn emit_list_sort_stable_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        // Use narrowed compare thunk for narrowed int lists.
        let compare_fn_ptr = self
            .get_or_create_narrowed_compare_thunk(elem_ty)
            .or_else(|| self.get_or_create_compare_thunk(elem_ty))?;

        let func_id = self.builder.runtime_fn("ori_list_sort_stable_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "sort_stable",
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
