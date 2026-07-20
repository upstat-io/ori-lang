//! LLVM lowering for list builtin methods.
//!
//! Handles read-only accessors (`length`, `len`, `count`, `is_empty`,
//! `first`, `last`, `contains`, `iter`) and helpers for the `list` type.
//!
//! COW mutation methods (push, pop, concat, reverse, set, insert, remove,
//! sort) are in the sibling `list_cow` module.

use ori_arc::ir::ArgOwnership;
use ori_types::{Idx, Tag};

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

// Read-only accessors

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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

    /// Emit `list.flatten()` — one-level flatten, `[[T]] -> [T]`.
    ///
    /// `element` is the RECEIVER's element type from `TypeInfo::List`, i.e.
    /// `[T]` for a genuine `[[T]]` receiver. Two branches, chosen at
    /// compile time from the type pool (never a runtime per-element check):
    ///
    /// - **Nested** (`element` resolves to `Tag::List`): peel one more
    ///   level to `T` (`pool.list_elem`, tag-guarded per the RCA's
    ///   correctness detail — `list_elem` silently misreads a non-List tag
    ///   in release), then call the `ori_list_flatten` runtime primitive.
    /// - **Non-nested** (`[T].flatten()`, `element` is not a List): the
    ///   whole input is already flat — an RC-bumped identity clone, exactly
    ///   `ori_eval::methods::list::list_flatten`'s passthrough branch and
    ///   `("list","clone")`'s own emission.
    pub(crate) fn emit_list_flatten(
        &mut self,
        receiver: ValueId,
        element: Idx,
        receiver_ty: Idx,
    ) -> Option<ValueId> {
        let resolved_element = self.pool.resolve_fully(element);
        if self.pool.tag(resolved_element) != Tag::List {
            return self.emit_rc_inc_clone(receiver, receiver_ty);
        }
        let inner_ty = self.pool.list_elem(resolved_element);

        let func_id = self.builder.runtime_fn("ori_list_flatten");
        let (outer_data, outer_len) = self.extract_list_data_and_len(receiver)?;
        let (elem_size_val, elem_align_val) =
            self.elem_size_and_align(inner_ty, Some(resolved_element));
        let inc_fn = self.get_or_generate_elem_inc_fn(inner_ty);

        let list_struct_ty = self.list_struct_type();
        let out =
            self.builder
                .create_entry_alloca(self.current_function, "flatten.out", list_struct_ty);

        self.emit_rt_call(
            func_id,
            &[
                outer_data,
                outer_len,
                elem_size_val,
                elem_align_val,
                inc_fn,
                out,
            ],
            "flatten",
        );

        Some(self.builder.load(list_struct_ty, out, "flatten.val"))
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
        let (data_ptr, len) = self.extract_list_data_and_len(receiver)?;

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
    /// Emits the in-bounds load directly and retains `ori_list_get` only on
    /// the cold out-of-bounds edge. This keeps the runtime's panic/unwind
    /// contract while exposing ordinary indexed loads to LLVM's loop passes.
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
        let (data_ptr, len) = self.extract_list_data_and_len(receiver)?;

        // Use narrowed element size/type if available.
        let collection_idx = self.pool.resolve_fully(list_ty);
        let bool_elem = self.pool.tag(self.pool.resolve_fully(elem_ty)) == Tag::Bool;
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);
        // Lists store bools in one addressable byte. Loading through i1 makes
        // LLVM preserve sub-byte value semantics with masks and blocks the
        // load/xor/store combine used by ordinary byte-backed Boolean arrays.
        let elem_llvm_ty = if bool_elem {
            self.builder
                .register_type(self.builder.scx().type_i8().into())
        } else {
            self.collection_elem_llvm_type(collection_idx, elem_ty)
        };

        // The fallback writes through the established runtime ABI. It exists
        // only on the cold edge so the common path is an ordinary typed load.
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "index.out", elem_llvm_ty);
        let direct = self
            .builder
            .append_block(self.current_function, "index.direct");
        let fallback = self
            .builder
            .append_block(self.current_function, "index.fallback");
        let merge = self
            .builder
            .append_block(self.current_function, "index.merge");
        // An unsigned comparison rejects negative indices as well as indices
        // at or beyond len without a separate signed check.
        let in_bounds = self.builder.icmp_ult(index, len, "index.in_bounds");
        self.builder.cond_br(in_bounds, direct, fallback);

        self.builder.position_at_end(direct);
        let elem_ptr = self
            .builder
            .gep(elem_llvm_ty, data_ptr, &[index], "index.elem_ptr");
        let direct_val = self
            .builder
            .load(elem_llvm_ty, elem_ptr, "index.direct_val");
        let direct_end = self.builder.current_block().expect("direct index block");
        self.builder.br(merge);

        self.builder.position_at_end(fallback);
        let func_id = self.builder.runtime_fn("ori_list_get");
        self.emit_rt_call(
            func_id,
            &[data_ptr, len, index, elem_size_val, out_alloca],
            "index",
        );
        let fallback_val = self
            .builder
            .load(elem_llvm_ty, out_alloca, "index.fallback_val");
        let fallback_end = self.builder.current_block().expect("fallback index block");
        self.builder.br(merge);

        self.builder.position_at_end(merge);
        let elem_val = self.builder.phi(elem_llvm_ty, "index.val");
        self.builder.add_phi_incoming(
            elem_val,
            &[(direct_val, direct_end), (fallback_val, fallback_end)],
        );

        // Sign-extend narrowed element back to canonical i64.
        let elem_val = if bool_elem {
            let bool_ty = self
                .builder
                .register_type(self.builder.scx().type_i1().into());
            self.builder.trunc(elem_val, bool_ty, "index.bool")
        } else {
            self.sext_narrowed_collection_element(elem_val, collection_idx, "index.sext")
        };

        // ori_list_get does a raw memcpy — the extracted element shares
        // the collection's RC children (e.g., str data pointers) without
        // incrementing their RC. This compiled adapter emits RcInc so the
        // caller receives the owner credit frozen by the shared plan. The
        // adapter's later RcDec realizes the matching release. Neither physical
        // counter operation defines AIMS policy.
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
    /// `elem_dec_fn` from the V5 RC header at cleanup time.
    ///
    /// **Narrowing boundary:** when the list has narrowed int
    /// elements (i8/i16/i32 via repr-opt), the list iterator yields narrowed
    /// bytes. To prevent the narrowing from leaking into the iterator pipeline
    /// (trampolines, scratch buffers, consumers), we inject a sext widening
    /// map trampoline that converts narrowed elements to canonical i64. The
    /// entire iterator pipeline then operates on canonical element types.
    pub(crate) fn emit_list_iter(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        elem_ty: Idx,
        ownership: ArgOwnership,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_list");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let collection_idx = self.pool.resolve_fully(receiver_ty);
        let narrowed_elem_size = self.collection_elem_size(collection_idx, elem_ty);
        let elem_size_val = self.builder.const_i64(narrowed_elem_size as i64);
        // owns_data threads the ARC arg-ownership of the .iter() receiver into the
        // runtime: an owned receiver (the ARC pipeline gave the iterator its own
        // RC ref) → true (Drop decs); a borrowed-rooted receiver (the flatten
        // inner sub.iter() on a trampoline-closure param) → false (Drop does NOT
        // dec; the outer container's elem_dec_fn frees the buffer once).
        let owns_data_val = self.builder.const_bool(ownership == ArgOwnership::Owned);

        let list_iter = self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, elem_size_val, owns_data_val],
            "list.iter",
        )?;

        // If the collection has narrowed int elements, inject a sext widening
        // trampoline at the iter() boundary. This converts narrowed elements
        // (i8/i16/i32) to canonical i64 so the entire iterator pipeline
        // operates on canonical types. Without this, downstream trampolines,
        // scratch buffers, and collect allocations would use the narrowed size
        // and corrupt data.
        let canonical_elem_size = self.element_store_size(elem_ty);
        if narrowed_elem_size < canonical_elem_size {
            if let Some(narrowed_width) = self.narrowed_collection_element_width(collection_idx) {
                let sext_tramp_fn_id = self.generate_sext_widening_trampoline(narrowed_width);
                let sext_tramp_ptr = self.builder.get_function_ptr(sext_tramp_fn_id);
                let null_env = self.builder.const_null_ptr();
                let null_env_inc = self.builder.const_null_ptr();
                let null_env_dec = self.builder.const_null_ptr();
                let null_output_dec = self.builder.const_null_ptr();

                let map_fn_id = self.builder.runtime_fn("ori_iter_map");
                return self.emit_rt_call(
                    map_fn_id,
                    &[
                        list_iter,
                        sext_tramp_ptr,
                        null_env,
                        null_env_inc,
                        null_env_dec,
                        elem_size_val,
                        null_output_dec,
                    ],
                    "list.iter.widen",
                );
            }
        }

        Some(list_iter)
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

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
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
        // Use narrowed element size if available.
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
