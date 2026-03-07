//! Type-specific compound trait implementations (Option, Result, Tuple, Str).
//!
//! Implements `equals`, `compare`, and `hash` for compound wrapper types
//! by structural recursion into element types via `emit_element_*` dispatch.
//!
//! ## ARC enum convention
//!
//! - **Option**: `{i64 tag, T payload}` — Some=0, None=1
//! - **Result**: `{i64 tag, payload}`   — Ok=0, Err=1
//! - **Tuple**:  `{A, B, ...}`         — flat struct of resolved element types

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // Option trait methods

    /// `Option<T>.equals(other) -> bool`
    ///
    /// Tags differ → false. Both None → true. Both Some → payload equals.
    pub(super) fn emit_option_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "opt.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "opt.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        let lhs_val = self.builder.extract_value(lhs, 1, "opt.lhs.val")?;
        let rhs_val = self.builder.extract_value(rhs, 1, "opt.rhs.val")?;
        let payload_eq = self.emit_element_equals(lhs_val, rhs_val, inner_ty)?;

        // Both None (tag=1): equal. Both Some (tag=0): check payload.
        let one = self.builder.const_i64(1);
        let is_none = self.builder.icmp_eq(lhs_tag, one, "is_none");
        let true_val = self.builder.const_bool(true);
        let same_tag_result = self
            .builder
            .select(is_none, true_val, payload_eq, "same_eq");

        let false_val = self.builder.const_bool(false);
        Some(
            self.builder
                .select(tags_eq, same_tag_result, false_val, "opt_eq"),
        )
    }

    /// `Option<T>.compare(other) -> Ordering`
    ///
    /// Semantics: None < Some. If both Some, compare payloads.
    /// ARC tags: Some=0, None=1. Tag order is reversed from semantic order,
    /// so we compare `rhs_tag vs lhs_tag` (swapped) for the tag comparison.
    pub(super) fn emit_option_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "opt.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "opt.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        // Tags differ: reversed order (None(1) < Some(0) semantically).
        let tag_cmp = self
            .builder
            .emit_icmp_ordering(rhs_tag, lhs_tag, "tag_cmp", false);

        // Tags equal: None+None → Equal, Some+Some → compare payloads.
        let lhs_val = self.builder.extract_value(lhs, 1, "opt.lhs.val")?;
        let rhs_val = self.builder.extract_value(rhs, 1, "opt.rhs.val")?;
        let payload_cmp = self.emit_element_compare(lhs_val, rhs_val, inner_ty)?;

        let one = self.builder.const_i64(1);
        let is_none = self.builder.icmp_eq(lhs_tag, one, "is_none");
        let equal_ord = self.builder.const_i8(1);
        let same_tag_cmp = self
            .builder
            .select(is_none, equal_ord, payload_cmp, "same_cmp");

        Some(
            self.builder
                .select(tags_eq, same_tag_cmp, tag_cmp, "opt_cmp"),
        )
    }

    /// `Option<T>.hash() -> int`
    ///
    /// None → 0. `Some(x)` → `hash_combine(1, x.hash())`.
    pub(super) fn emit_option_hash(&mut self, val: ValueId, inner_ty: Idx) -> Option<ValueId> {
        let tag = self.builder.extract_value(val, 0, "opt.tag")?;
        let payload = self.builder.extract_value(val, 1, "opt.payload")?;

        let payload_hash = self.emit_element_hash(payload, inner_ty)?;
        let seed = self.builder.const_i64(1);
        let some_hash = self.emit_hash_combine(seed, payload_hash);

        let one = self.builder.const_i64(1);
        let is_none = self.builder.icmp_eq(tag, one, "is_none");
        let zero = self.builder.const_i64(0);
        Some(self.builder.select(is_none, zero, some_hash, "opt_hash"))
    }

    // Result trait methods

    /// Extract the payload from a Result as the correct variant type.
    ///
    /// The Result payload slot is sized for the *larger* of Ok/Err. When
    /// the variant type differs from the storage type, this uses
    /// alloca + GEP + load to reinterpret the payload memory.
    fn extract_result_payload(
        &mut self,
        result: ValueId,
        result_ty: Idx,
        variant_ty: Idx,
    ) -> Option<ValueId> {
        self.extract_tagged_union_payload(result, result_ty, 1, variant_ty)
    }

    /// `Result<Ok, Err>.equals(other) -> bool`
    ///
    /// Tags differ → false. Same tag → branch on Ok vs Err payload type.
    /// Uses alloca-based payload extraction so Ok and Err types can differ.
    pub(super) fn emit_result_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        result_ty: Idx,
        ok_ty: Idx,
        err_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "res.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "res.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        // Create all blocks upfront.
        let same_tag_bb = self
            .builder
            .append_block(self.current_function, "res_eq.same");
        let ok_cmp_bb = self
            .builder
            .append_block(self.current_function, "res_eq.ok");
        let err_cmp_bb = self
            .builder
            .append_block(self.current_function, "res_eq.err");
        let diff_bb = self
            .builder
            .append_block(self.current_function, "res_eq.diff");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "res_eq.merge");

        // Entry: tags differ → false, tags equal → check variant.
        self.builder.cond_br(tags_eq, same_tag_bb, diff_bb);

        // same_tag: Ok (tag=0) → ok_cmp, Err (tag=1) → err_cmp.
        self.builder.position_at_end(same_tag_bb);
        let zero = self.builder.const_i64(0);
        let is_ok = self.builder.icmp_eq(lhs_tag, zero, "is_ok");
        self.builder.cond_br(is_ok, ok_cmp_bb, err_cmp_bb);

        // ok_cmp: extract Ok payloads with correct type, compare.
        self.builder.position_at_end(ok_cmp_bb);
        let ok_lhs = self.extract_result_payload(lhs, result_ty, ok_ty)?;
        let ok_rhs = self.extract_result_payload(rhs, result_ty, ok_ty)?;
        let ok_eq = self.emit_element_equals(ok_lhs, ok_rhs, ok_ty)?;
        let ok_exit_bb = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // err_cmp: extract Err payloads with correct type, compare.
        self.builder.position_at_end(err_cmp_bb);
        let err_lhs = self.extract_result_payload(lhs, result_ty, err_ty)?;
        let err_rhs = self.extract_result_payload(rhs, result_ty, err_ty)?;
        let err_eq = self.emit_element_equals(err_lhs, err_rhs, err_ty)?;
        let err_exit_bb = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // diff: tags differ → false.
        self.builder.position_at_end(diff_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);

        // merge: phi collects results from all three paths.
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        let result = self.builder.phi(bool_ty, "res_eq");
        self.builder.add_phi_incoming(
            result,
            &[
                (ok_eq, ok_exit_bb),
                (err_eq, err_exit_bb),
                (false_val, diff_bb),
            ],
        );

        Some(result)
    }

    /// `Result<Ok, Err>.compare(other) -> Ordering`
    ///
    /// Tags differ → compare tags (Ok=0 < Err=1, matches numeric order).
    /// Same tag → branch on Ok vs Err to select payload comparison type.
    pub(super) fn emit_result_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        result_ty: Idx,
        ok_ty: Idx,
        err_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "res.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "res.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        let tag_cmp = self
            .builder
            .emit_icmp_ordering(lhs_tag, rhs_tag, "tag_cmp", false);

        // Create all blocks upfront.
        let same_tag_bb = self
            .builder
            .append_block(self.current_function, "res_cmp.same");
        let ok_cmp_bb = self
            .builder
            .append_block(self.current_function, "res_cmp.ok");
        let err_cmp_bb = self
            .builder
            .append_block(self.current_function, "res_cmp.err");
        let diff_bb = self
            .builder
            .append_block(self.current_function, "res_cmp.diff");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "res_cmp.merge");

        self.builder.cond_br(tags_eq, same_tag_bb, diff_bb);

        // same_tag: Ok (tag=0) → ok_cmp, Err (tag=1) → err_cmp.
        self.builder.position_at_end(same_tag_bb);
        let zero = self.builder.const_i64(0);
        let is_ok = self.builder.icmp_eq(lhs_tag, zero, "is_ok");
        self.builder.cond_br(is_ok, ok_cmp_bb, err_cmp_bb);

        // ok_cmp: extract Ok payloads, compare.
        self.builder.position_at_end(ok_cmp_bb);
        let ok_lhs = self.extract_result_payload(lhs, result_ty, ok_ty)?;
        let ok_rhs = self.extract_result_payload(rhs, result_ty, ok_ty)?;
        let ok_cmp = self.emit_element_compare(ok_lhs, ok_rhs, ok_ty)?;
        let ok_exit_bb = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // err_cmp: extract Err payloads, compare.
        self.builder.position_at_end(err_cmp_bb);
        let err_lhs = self.extract_result_payload(lhs, result_ty, err_ty)?;
        let err_rhs = self.extract_result_payload(rhs, result_ty, err_ty)?;
        let err_cmp = self.emit_element_compare(err_lhs, err_rhs, err_ty)?;
        let err_exit_bb = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // diff: tags differ → tag ordering.
        self.builder.position_at_end(diff_bb);
        self.builder.br(merge_bb);

        // merge: phi collects results from all three paths.
        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        let result = self.builder.phi(i8_ty, "res_cmp");
        self.builder.add_phi_incoming(
            result,
            &[
                (ok_cmp, ok_exit_bb),
                (err_cmp, err_exit_bb),
                (tag_cmp, diff_bb),
            ],
        );

        Some(result)
    }

    /// `Result<Ok, Err>.hash() -> int`
    ///
    /// `hash_combine(tag, payload.hash())` — branches on tag to select
    /// `ok_ty` vs `err_ty` for the payload hash.
    pub(super) fn emit_result_hash(
        &mut self,
        val: ValueId,
        result_ty: Idx,
        ok_ty: Idx,
        err_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(val, 0, "res.tag")?;

        // Create blocks for tag-based dispatch.
        let ok_bb = self
            .builder
            .append_block(self.current_function, "res_hash.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res_hash.err");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "res_hash.merge");

        let zero = self.builder.const_i64(0);
        let is_ok = self.builder.icmp_eq(tag, zero, "is_ok");
        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // ok: extract Ok payload with correct type, hash it.
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_result_payload(val, result_ty, ok_ty)?;
        let ok_hash = self.emit_element_hash(ok_payload, ok_ty)?;
        let ok_exit_bb = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // err: extract Err payload with correct type, hash it.
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_result_payload(val, result_ty, err_ty)?;
        let err_hash = self.emit_element_hash(err_payload, err_ty)?;
        let err_exit_bb = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // merge: phi selects the correct payload hash.
        self.builder.position_at_end(merge_bb);
        let i64_ty = self.builder.i64_type();
        let payload_hash = self.builder.phi(i64_ty, "res_payload_hash");
        self.builder.add_phi_incoming(
            payload_hash,
            &[(ok_hash, ok_exit_bb), (err_hash, err_exit_bb)],
        );

        Some(self.emit_hash_combine(tag, payload_hash))
    }

    // Tuple trait methods

    /// `Tuple.equals(other) -> bool`
    ///
    /// All fields must be equal (conjunction).
    pub(in crate::codegen::arc_emitter) fn emit_tuple_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        let mut result = self.builder.const_bool(true);

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate() {
            let lhs_field = self.builder.extract_value(lhs, i as u32, "tup.lhs")?;
            let rhs_field = self.builder.extract_value(rhs, i as u32, "tup.rhs")?;
            let field_eq = self.emit_element_equals(lhs_field, rhs_field, elem_ty)?;
            result = self.builder.and(result, field_eq, "tup_eq");
        }

        Some(result)
    }

    /// `Tuple.compare(other) -> Ordering`
    ///
    /// Lexicographic: compare field 0, if Equal compare field 1, etc.
    pub(super) fn emit_tuple_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i8(1)); // Equal
        }

        let equal_ord = self.builder.const_i8(1);
        let mut result = equal_ord;

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate() {
            let lhs_field = self.builder.extract_value(lhs, i as u32, "tup.lhs")?;
            let rhs_field = self.builder.extract_value(rhs, i as u32, "tup.rhs")?;
            let field_cmp = self.emit_element_compare(lhs_field, rhs_field, elem_ty)?;

            // If previous result was Equal, use this field's comparison.
            let prev_is_eq = self.builder.icmp_eq(result, equal_ord, "prev_eq");
            result = self
                .builder
                .select(prev_is_eq, field_cmp, result, "tup_cmp");
        }

        Some(result)
    }

    /// `Tuple.hash() -> int`
    ///
    /// Fold `hash_combine` over field hashes.
    pub(super) fn emit_tuple_hash(&mut self, val: ValueId, elements: &[Idx]) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i64(0));
        }

        let first_field = self.builder.extract_value(val, 0, "tup.f0")?;
        let mut result = self.emit_element_hash(first_field, elements[0])?;

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate().skip(1) {
            let field = self.builder.extract_value(val, i as u32, "tup.f")?;
            let field_hash = self.emit_element_hash(field, elem_ty)?;
            result = self.emit_hash_combine(result, field_hash);
        }

        Some(result)
    }

    // String helpers (exposed for compound trait use)

    /// Call `ori_str_compare(ptr, ptr) -> i8` for string comparison.
    pub(super) fn emit_str_compare_call(&mut self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_compare");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], "str_cmp")
    }

    /// Call `ori_str_hash(ptr) -> i64` for string hashing.
    pub(super) fn emit_str_hash_call(&mut self, val: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_hash");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_hash.arg", str_ty);
        self.builder.store(val, ptr);

        self.emit_rt_call(func_id, &[ptr], "str_hash")
    }
}
