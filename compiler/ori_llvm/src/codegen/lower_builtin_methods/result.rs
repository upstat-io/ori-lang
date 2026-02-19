//! Result type method lowering: `is_ok`, `is_err`, unwrap, compare, equals, hash.

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Built-in Result methods.
    ///
    /// Result is `{i8 tag, max(T, E) payload}` where tag=0 is Ok, tag=1 is Err.
    pub(super) fn lower_result_method(
        &mut self,
        recv: ValueId,
        ok_type: Idx,
        err_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "res.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_ok" => Some(self.builder.icmp_eq(tag, zero, "res.is_ok")),
            "is_err" => Some(self.builder.icmp_ne(tag, zero, "res.is_err")),
            "unwrap" => {
                let payload = self.builder.extract_value(recv, 1, "res.unwrap")?;
                Some(self.coerce_payload(payload, ok_type))
            }
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_result_compare(recv, other, ok_type, err_type)
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_result_equals(recv, other, ok_type, err_type)
            }
            "hash" => self.emit_result_hash(recv, ok_type, err_type),
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// `Result.compare()`: Ok < Err, then compare payloads.
    pub(super) fn emit_result_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        ok_type: Idx,
        err_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "res.cmp.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "res.cmp.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "res.cmp.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.check");
        let diff_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.diff");

        self.builder.cond_br(tags_eq, check_bb, diff_bb);

        // Tags differ: Ok(0) < Err(1), so compare tags unsigned
        self.builder.position_at_end(diff_bb);
        let diff_ord = self.emit_icmp_ordering(tag_self, tag_other, "res.cmp.tag", false);
        self.builder.br(merge_bb);
        let diff_bb_final = self.builder.current_block()?;

        // Tags equal: compare payloads (coerced to the correct variant type)
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_ok = self.builder.icmp_eq(tag_self, zero, "res.cmp.is_ok");

        let ok_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.err");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Both Ok → compare ok payloads
        self.builder.position_at_end(ok_bb);
        let pay_self_ok = self.builder.extract_value(lhs, 1, "res.cmp.pay.self.ok")?;
        let pay_other_ok = self.builder.extract_value(rhs, 1, "res.cmp.pay.other.ok")?;
        let self_ok = self.coerce_payload(pay_self_ok, ok_type);
        let other_ok = self.coerce_payload(pay_other_ok, ok_type);
        let ok_ord = self.emit_inner_compare(self_ok, other_ok, ok_type, "res.cmp.ok");
        self.builder.br(merge_bb);
        let ok_bb_final = self.builder.current_block()?;

        // Both Err → compare err payloads
        self.builder.position_at_end(err_bb);
        let pay_self_err = self.builder.extract_value(lhs, 1, "res.cmp.pay.self.err")?;
        let pay_other_err = self
            .builder
            .extract_value(rhs, 1, "res.cmp.pay.other.err")?;
        let self_err = self.coerce_payload(pay_self_err, err_type);
        let other_err = self.coerce_payload(pay_other_err, err_type);
        let err_ord = self.emit_inner_compare(self_err, other_err, err_type, "res.cmp.err");
        self.builder.br(merge_bb);
        let err_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder.phi_from_incoming(
            i8_ty,
            &[
                (diff_ord, diff_bb_final),
                (ok_ord, ok_bb_final),
                (err_ord, err_bb_final),
            ],
            "res.cmp.result",
        )
    }

    /// `Result.equals()`: tags must match, then compare payloads.
    pub(super) fn emit_result_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        ok_type: Idx,
        err_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "res.eq.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "res.eq.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "res.eq.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "res.eq.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "res.eq.check");
        let false_bb = self
            .builder
            .append_block(self.current_function, "res.eq.false");

        self.builder.cond_br(tags_eq, check_bb, false_bb);

        // Tags differ → false
        self.builder.position_at_end(false_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);
        let false_bb_final = self.builder.current_block()?;

        // Tags equal: dispatch on Ok vs Err
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_ok = self.builder.icmp_eq(tag_self, zero, "res.eq.is_ok");

        let ok_bb = self
            .builder
            .append_block(self.current_function, "res.eq.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res.eq.err");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Both Ok → compare ok payloads
        self.builder.position_at_end(ok_bb);
        let pay_self_ok = self.builder.extract_value(lhs, 1, "res.eq.pay.self.ok")?;
        let pay_other_ok = self.builder.extract_value(rhs, 1, "res.eq.pay.other.ok")?;
        let self_ok = self.coerce_payload(pay_self_ok, ok_type);
        let other_ok = self.coerce_payload(pay_other_ok, ok_type);
        let ok_eq = self.emit_inner_eq(self_ok, other_ok, ok_type, "res.eq.ok");
        self.builder.br(merge_bb);
        let ok_bb_final = self.builder.current_block()?;

        // Both Err → compare err payloads
        self.builder.position_at_end(err_bb);
        let pay_self_err = self.builder.extract_value(lhs, 1, "res.eq.pay.self.err")?;
        let pay_other_err = self.builder.extract_value(rhs, 1, "res.eq.pay.other.err")?;
        let self_err = self.coerce_payload(pay_self_err, err_type);
        let other_err = self.coerce_payload(pay_other_err, err_type);
        let err_eq = self.emit_inner_eq(self_err, other_err, err_type, "res.eq.err");
        self.builder.br(merge_bb);
        let err_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[
                (false_val, false_bb_final),
                (ok_eq, ok_bb_final),
                (err_eq, err_bb_final),
            ],
            "res.eq.result",
        )
    }

    /// `Result.hash()`: Ok(x) → `hash_combine(2, x.hash())`, Err(x) → `hash_combine(3, x.hash())`.
    pub(super) fn emit_result_hash(
        &mut self,
        recv: ValueId,
        ok_type: Idx,
        err_type: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "res.hash.tag")?;
        let zero = self.builder.const_i8(0);
        let is_ok = self.builder.icmp_eq(tag, zero, "res.hash.is_ok");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "res.hash.merge");
        let ok_bb = self
            .builder
            .append_block(self.current_function, "res.hash.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res.hash.err");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok → hash_combine(2, ok_payload.hash())
        self.builder.position_at_end(ok_bb);
        let pay_ok = self.builder.extract_value(recv, 1, "res.hash.pay.ok")?;
        let pay_ok_coerced = self.coerce_payload(pay_ok, ok_type);
        let ok_inner = self.emit_inner_hash(pay_ok_coerced, ok_type, "res.hash.ok");
        let seed_ok = self.builder.const_i64(2);
        let ok_hash = self.emit_hash_combine(seed_ok, ok_inner, "res.hash.ok.combine");
        self.builder.br(merge_bb);
        let ok_bb_final = self.builder.current_block()?;

        // Err → hash_combine(3, err_payload.hash())
        self.builder.position_at_end(err_bb);
        let pay_err = self.builder.extract_value(recv, 1, "res.hash.pay.err")?;
        let pay_err_coerced = self.coerce_payload(pay_err, err_type);
        let err_inner = self.emit_inner_hash(pay_err_coerced, err_type, "res.hash.err");
        let seed_err = self.builder.const_i64(3);
        let err_hash = self.emit_hash_combine(seed_err, err_inner, "res.hash.err.combine");
        self.builder.br(merge_bb);
        let err_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i64_ty = self.builder.i64_type();
        self.builder.phi_from_incoming(
            i64_ty,
            &[(ok_hash, ok_bb_final), (err_hash, err_bb_final)],
            "res.hash.result",
        )
    }
}
