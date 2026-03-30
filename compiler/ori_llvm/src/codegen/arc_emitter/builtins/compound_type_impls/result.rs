//! Result trait method codegen.
//!
//! Implements `equals`, `compare`, and `hash` for `Result<T, E>`.
//!
//! ## ARC enum convention
//!
//! `Result<Ok, Err>` is represented as `{i64 tag, payload}` — Ok=0, Err=1.
//! The payload slot is sized for the *larger* of Ok/Err.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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
    pub(in super::super) fn emit_result_equals(
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
        let zero = self.builder.const_int_matching(lhs_tag, 0);
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
    pub(in super::super) fn emit_result_compare(
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
        let zero = self.builder.const_int_matching(lhs_tag, 0);
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
    pub(in super::super) fn emit_result_hash(
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

        let zero = self.builder.const_int_matching(tag, 0);
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

        // Zero-extend narrowed tag to i64 for hash_combine which expects i64
        let tag_i64 = self.builder.zext(tag, i64_ty, "res.tag.ext");
        Some(self.emit_hash_combine(tag_i64, payload_hash))
    }
}
