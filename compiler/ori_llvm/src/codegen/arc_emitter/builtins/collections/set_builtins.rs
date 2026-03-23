//! Set and Range builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains`, `insert`, `remove`,
//! `union`, `intersection`, `difference`, `to_list`, `into` for sets,
//! plus `range.iter()`.
//!
//! Set mutations use COW semantics: when the collection is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each mutating method returns a `{i64 len, i64 cap, ptr data}` struct.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `set.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_set_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "set.len")
    }

    /// Emit `set.is_empty()` — `len == 0`.
    pub(crate) fn emit_set_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "set.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "set.is_empty"))
    }

    /// Extract set data, len, cap from `{i64 len, i64 cap, ptr data}`.
    fn extract_set_components(&mut self, receiver: ValueId) -> (ValueId, ValueId, ValueId) {
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "set.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "set.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let cap = self
            .builder
            .extract_value(receiver, 1, "set.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        (data_ptr, len, cap)
    }

    /// Emit `set.contains(elem)` — hash table lookup with type-specific equality.
    ///
    /// Calls `ori_set_contains(data, cap, len, elem_ptr, elem_size, elem_eq, elem_hash)`.
    pub(crate) fn emit_set_contains(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_contains");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "contains.elem");
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;

        let result = self.emit_rt_call(
            func_id,
            &[data_ptr, cap, len, elem_ptr, elem_size, elem_eq, elem_hash],
            "set.contains",
        )?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "set.contains.bool"))
    }

    /// Emit `set.insert(elem)` — COW insert returning the (possibly mutated) set.
    ///
    /// No-op if element exists. Fast path (unique): appends in place.
    /// Slow path (shared): copies to new buffer.
    ///
    /// Calls `ori_set_insert_cow(data, len, cap, elem, elem_size, elem_align,
    ///         elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)`.
    pub(crate) fn emit_set_insert(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_insert_cow");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let (elem_size, elem_align) = self.elem_size_and_align(elem_ty);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let set_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "set.insert.out", set_ty);

        self.emit_rt_call(
            func_id,
            &[
                data_ptr, len, cap, elem_ptr, elem_size, elem_align, elem_eq, elem_hash, inc_fn,
                cow_mode, out,
            ],
            "set.insert",
        );

        Some(self.builder.load(set_ty, out, "set.insert.val"))
    }

    /// Emit `set.remove(elem)` — COW remove returning the (possibly mutated) set.
    ///
    /// No-op if element not found. Fast path (unique): decs removed element,
    /// then tombstones. Slow path (shared): copies all except removed.
    ///
    /// Calls `ori_set_remove_cow(data, len, cap, elem, elem_size, elem_align,
    ///         elem_eq, elem_hash, inc_fn, elem_dec_fn, cow_mode, out_ptr)`.
    pub(crate) fn emit_set_remove(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_remove_cow");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "remove.elem");
        let (elem_size, elem_align) = self.elem_size_and_align(elem_ty);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);

        let set_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "set.remove.out", set_ty);

        self.emit_rt_call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_ptr,
                elem_size,
                elem_align,
                elem_eq,
                elem_hash,
                inc_fn,
                elem_dec_fn,
                cow_mode,
                out,
            ],
            "set.remove",
        );

        Some(self.builder.load(set_ty, out, "set.remove.val"))
    }

    /// Emit a two-set COW operation (union/intersection/difference) via sret.
    ///
    /// The receiver (set1) is consumed; the other (set2) is borrowed.
    ///
    /// Calls `ori_set_{op}_cow(d1, l1, c1, d2, l2, c2, elem_size, elem_align,
    ///         elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)`.
    fn emit_set_binary_op(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        func_name: &'static str,
        label: &str,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);

        let (d1, l1, c1) = self.extract_set_components(receiver);
        // Second set: need data, len, and cap for hash table lookups
        let d2 = self
            .builder
            .extract_value(other, 2, "set2.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let l2 = self
            .builder
            .extract_value(other, 0, "set2.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let c2 = self
            .builder
            .extract_value(other, 1, "set2.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let (elem_size, elem_align) = self.elem_size_and_align(elem_ty);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let set_ty = self.list_struct_type();
        let out = self.builder.create_entry_alloca(
            self.current_function,
            &format!("set.{label}.out"),
            set_ty,
        );

        self.emit_rt_call(
            func_id,
            &[
                d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn,
                cow_mode, out,
            ],
            &format!("set.{label}"),
        );

        Some(self.builder.load(set_ty, out, &format!("set.{label}.val")))
    }

    /// Emit `set.union(other)` — COW union.
    pub(crate) fn emit_set_union(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_union_cow",
            "union",
            cow_mode,
        )
    }

    /// Emit `set.intersection(other)` — COW intersection.
    pub(crate) fn emit_set_intersection(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_intersection_cow",
            "intersection",
            cow_mode,
        )
    }

    /// Emit `set.difference(other)` — COW difference.
    pub(crate) fn emit_set_difference(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_difference_cow",
            "difference",
            cow_mode,
        )
    }

    /// Emit `set.to_list()` / `set.into()` — copies set data into a new list via sret.
    ///
    /// Calls `ori_set_to_list(data, cap, len, elem_size, elem_dec_fn,
    /// elem_inc_fn, out_ptr)`. `elem_inc_fn` prevents double-free on
    /// shared RC-tracked element data.
    pub(crate) fn emit_set_to_list(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_to_list");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "set.to_list.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[
                data_ptr,
                cap,
                len,
                elem_size,
                elem_dec_fn,
                elem_inc_fn,
                out_alloca,
            ],
            "set.to_list",
        );

        Some(self.builder.load(list_ty, out_alloca, "set.to_list.val"))
    }

    /// Emit `set.iter()` — convert set to contiguous list, then create list iterator.
    ///
    /// Sets use hash table layout where elements are at non-contiguous positions
    /// (interleaved with metadata). Calling `emit_list_iter` directly on a Set
    /// creates an `IterState::List` that iterates contiguously — wrong for hash
    /// tables. Instead: convert to a contiguous list via `ori_set_to_list`, then
    /// create an iterator over that list.
    ///
    /// After conversion, the set buffer is explicitly decremented — the ARC
    /// pipeline passes the set with `[own]`, expecting the callee to handle
    /// cleanup. For lists, `IterState::List` Drop implicitly handles this
    /// (same buffer). For sets, the iterator holds the converted list buffer
    /// (different allocation), so we must explicitly dec the set buffer.
    pub(crate) fn emit_set_iter(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        // Convert set to contiguous list (copies elements, incs element RCs).
        let list_val = self.emit_set_to_list(receiver, elem_ty)?;

        // Dec the set buffer — the converted list now owns the element references.
        // The set buffer's RC was incremented by AIMS for the [own] parameter;
        // this dec matches that inc, freeing the set buffer if no other refs exist.
        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let func_id = self.builder.runtime_fn("ori_set_buffer_rc_dec");
        self.emit_rt_call(func_id, &[data_ptr, cap, len, elem_size, elem_dec_fn], "");

        // Create iterator from the contiguous list.
        self.emit_list_iter(list_val, elem_ty, elem_ty)
    }

    // Range methods

    /// Emit `range.iter()` — call `ori_iter_from_range(start, end, step, inclusive)`.
    ///
    /// Range is lowered as a 4-element Tuple `{i64 start, i64 end, i64 step,
    /// i64 inclusive}` by `lower_range`. The inclusive flag (field 3) is
    /// stored as i64 (0 or 1) and truncated to i1 for the runtime call.
    pub(crate) fn emit_range_iter(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_range");

        let start = self
            .builder
            .extract_value(receiver, 0, "range.start")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let end = self
            .builder
            .extract_value(receiver, 1, "range.end")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let step = self
            .builder
            .extract_value(receiver, 2, "range.step")
            .unwrap_or_else(|| self.builder.const_i64(1));
        let incl_i64 = self
            .builder
            .extract_value(receiver, 3, "range.incl.raw")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Truncate inclusive flag from i64 to i1 for the runtime
        let bool_ty = self.builder.bool_type();
        let inclusive = self.builder.trunc(incl_i64, bool_ty, "range.inclusive");

        self.emit_rt_call(func_id, &[start, end, step, inclusive], "range.iter")
    }
}
