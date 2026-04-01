//! List builtin method codegen for LLVM.
//!
//! Handles read-only accessors (`length`, `len`, `count`, `is_empty`,
//! `first`, `last`, `contains`, `iter`) and helpers for the `list` type.
//!
//! COW mutation methods (push, pop, concat, reverse, set, insert, remove,
//! sort) are in the sibling `list_cow` module.

mod helpers;
mod sort_thunks;

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

// Read-only accessors

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `list.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_list_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "list.len")
    }

    /// Emit `list.is_empty()` — `len == 0`.
    pub(crate) fn emit_list_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "list.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
    }

    /// Emit `list.first()` — returns `Option<T>` as `{i64 tag, T value}`.
    pub(crate) fn emit_list_first(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, list_ty, "ori_list_first", "first")
    }

    /// Emit `list.last()` — returns `Option<T>` as `{i64 tag, T value}`.
    pub(crate) fn emit_list_last(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, list_ty, "ori_list_last", "last")
    }

    /// Emit `list.contains(x)` — returns `bool`.
    ///
    /// Dispatches to type-specific runtime functions:
    /// - `[int]` → `ori_list_contains_int(data, len, needle)` (canonical i64)
    /// - `[int]` (narrowed) → inline loop with narrowed element type
    /// - `[str]` → `ori_list_contains_str(data, len, needle_ptr)`
    pub(crate) fn emit_list_contains(
        &mut self,
        receiver: ValueId,
        needle: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let (data_ptr, len) = self.extract_list_data_and_len(receiver);

        let elem_info = self.type_info.get(elem_ty);

        // Narrowed int elements cannot use the runtime
        // `ori_list_contains_int` which hardcodes i64 stride. Generate an
        // inline loop with the correct narrowed element type instead.
        if matches!(&elem_info, TypeInfo::Int) {
            let collection_idx = self.pool.resolve_fully(list_ty);
            if self
                .narrowed_collection_element_width(collection_idx)
                .is_some()
            {
                return self.emit_list_contains_int_narrowed(data_ptr, len, needle, list_ty);
            }
        }

        let (func_name, args): (&'static str, Vec<ValueId>) = match &elem_info {
            TypeInfo::Int => ("ori_list_contains_int", vec![data_ptr, len, needle]),
            TypeInfo::Str => {
                let needle_ptr = self.str_to_ptr(needle, "contains.needle");
                ("ori_list_contains_str", vec![data_ptr, len, needle_ptr])
            }
            _ => return None, // Other element types not yet supported
        };

        let func_id = self.builder.runtime_fn(func_name);
        let result = self.emit_rt_call(func_id, &args, "contains")?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains.bool"))
    }

    /// Emit inline `contains` loop for narrowed `[int]` lists.
    ///
    /// Generates a simple linear scan: truncate the canonical i64 needle
    /// to the narrowed width, then GEP/load/compare at the narrowed stride.
    fn emit_list_contains_int_narrowed(
        &mut self,
        data_ptr: ValueId,
        len: ValueId,
        needle: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_ty = self.pool.list_elem(collection_idx);
        let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_ty);

        // Truncate the canonical i64 needle to narrowed width for comparison.
        let narrow_needle =
            self.trunc_for_narrowed_collection_element(needle, collection_idx, "contains.trunc");

        let func = self.current_function;
        let pre_header = self.builder.current_block().expect("current block");
        let header = self.builder.append_block(func, "contains.hdr");
        let body = self.builder.append_block(func, "contains.body");
        let found = self.builder.append_block(func, "contains.found");
        let not_found = self.builder.append_block(func, "contains.notfound");
        let merge = self.builder.append_block(func, "contains.merge");

        self.builder.br(header);

        // Header: check index < len.
        self.builder.position_at_end(header);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "idx");
        let has_more = self.builder.icmp_slt(idx_phi, len, "has_more");
        self.builder.cond_br(has_more, body, not_found);

        // Body: load element, compare with needle.
        self.builder.position_at_end(body);
        let elem_ptr = self.builder.gep(elem_llvm_ty, data_ptr, &[idx_phi], "ep");
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "e");
        let is_match = self.builder.icmp_eq(elem_val, narrow_needle, "match");
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, "next_idx");
        let body_end = self.builder.current_block().expect("body block");
        self.builder.cond_br(is_match, found, header);

        // Wire phi.
        let zero = self.builder.const_i64(0);
        self.builder
            .add_phi_incoming(idx_phi, &[(zero, pre_header), (next_idx, body_end)]);

        // Found/not found merge.
        self.builder.position_at_end(found);
        self.builder.br(merge);
        self.builder.position_at_end(not_found);
        self.builder.br(merge);

        self.builder.position_at_end(merge);
        let bool_ty = self
            .builder
            .register_type(self.builder.scx().type_i1().into());
        let result = self.builder.phi(bool_ty, "contains.res");
        let true_val = self.builder.const_bool(true);
        let false_val = self.builder.const_bool(false);
        self.builder
            .add_phi_incoming(result, &[(true_val, found), (false_val, not_found)]);

        Some(result)
    }

    /// Emit `list[index]` — bounds-checked element access, returns `T` directly.
    ///
    /// Calls `ori_list_get(data, len, index, elem_size, out_ptr)`.
    /// Panics on out-of-bounds (no Option wrapper — direct element return).
    ///
    /// `list_ty` is the collection type (e.g., `List<int>`) used for
    /// Phase C narrowed element size/type lookup.
    pub(crate) fn emit_list_index(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_get");

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);

        // Use narrowed element size/type if available.
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);
        let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_ty);

        // Alloca for the element (uses narrowed type if applicable)
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "index.out", elem_llvm_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, index, elem_size_val, out_alloca],
            "index",
        );

        let elem_val = self.builder.load(elem_llvm_ty, out_alloca, "index.val");

        // Sign-extend narrowed element back to canonical i64.
        let elem_val =
            self.sext_narrowed_collection_element(elem_val, collection_idx, "index.sext");

        // ori_list_get does a raw memcpy — the extracted element shares
        // the collection's RC children (e.g., str data pointers) without
        // incrementing their RC. Emit RcInc on the extracted element so
        // the caller owns its own reference. The AIMS pipeline will emit
        // RcDec when the element goes out of scope, which balances this inc.
        if !self.classifier.is_scalar(elem_ty) {
            self.inc_value_rc(elem_val, elem_ty, 1);
        }

        Some(elem_val)
    }

    /// Emit `list.iter()` — call `ori_iter_from_list(data, len, cap, elem_size)`.
    ///
    /// The iterator takes ownership of one RC reference to the list data buffer.
    /// The caller is responsible for ensuring the buffer RC is incremented before
    /// this call (via ARC arg-ownership for explicit `.iter()`, or via
    /// `emit_slice_aware_rc_inc` in `emit_auto_iter` for auto-promoted methods).
    /// When the iterator is consumed/dropped, `Drop for IterState` calls
    /// `ori_buffer_rc_dec` to release this reference.
    ///
    /// Element cleanup is entirely header-based: `ori_buffer_rc_dec` reads
    /// `elem_dec_fn` from the V5 RC header at cleanup time (Section 02.1).
    pub(crate) fn emit_list_iter(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_list");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        // Narrowed element size for iterators.
        let collection_idx = self.pool.resolve_fully(receiver_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);

        self.emit_rt_call(func_id, &[data_ptr, len, cap, elem_size_val], "list.iter")
    }

    /// Emit `list.slice(start, end)` — zero-copy seamless slice.
    ///
    /// Creates a view into the original buffer. No elements are copied.
    /// The original buffer's RC is incremented (the slice references it).
    pub(crate) fn emit_list_slice(
        &mut self,
        receiver: ValueId,
        start: ValueId,
        end: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_slice");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        // Use narrowed element size if available.
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "slice.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, start, end, elem_size_val, out],
            "slice",
        );

        Some(self.builder.load(list_ty, out, "slice.val"))
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
        let func_id = self.builder.runtime_fn("ori_list_slice_take");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        // Use narrowed element size if available.
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "take.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, n, elem_size_val, out],
            "take",
        );

        Some(self.builder.load(list_ty, out, "take.val"))
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
        let func_id = self.builder.runtime_fn("ori_list_slice_drop");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        // Use narrowed element size if available.
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "drop.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, n, elem_size_val, out],
            "drop",
        );

        Some(self.builder.load(list_ty, out, "drop.val"))
    }
}
