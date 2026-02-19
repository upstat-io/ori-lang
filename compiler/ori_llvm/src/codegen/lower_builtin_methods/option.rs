//! Option type method lowering: `is_some`, `is_none`, unwrap, `unwrap_or`, compare, equals, hash.

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Built-in Option methods.
    ///
    /// Option is `{i8 tag, T payload}` where tag=0 is None, tag=1 is Some.
    pub(super) fn lower_option_method(
        &mut self,
        recv: ValueId,
        inner_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "opt.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_some" => Some(self.builder.icmp_ne(tag, zero, "opt.is_some")),
            "is_none" => Some(self.builder.icmp_eq(tag, zero, "opt.is_none")),
            "unwrap" => self.builder.extract_value(recv, 1, "opt.unwrap"),
            "unwrap_or" => {
                let is_some = self.builder.icmp_ne(tag, zero, "opt.is_some");
                let payload = self.builder.extract_value(recv, 1, "opt.payload")?;
                let arg_ids = self.canon.arena.get_expr_list(args);
                let default_val = self.lower(*arg_ids.first()?)?;
                Some(
                    self.builder
                        .select(is_some, payload, default_val, "opt.unwrap_or"),
                )
            }
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_option_compare(recv, other, inner_type)
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_option_equals(recv, other, inner_type)
            }
            "hash" => self.emit_option_hash(recv, inner_type),
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// `Option.compare()`: None < Some, then compare payloads.
    pub(super) fn emit_option_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "opt.cmp.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "opt.cmp.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "opt.cmp.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.check");
        let diff_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.diff");

        self.builder.cond_br(tags_eq, check_bb, diff_bb);

        // Tags differ: None(0) < Some(1), so compare tags unsigned
        self.builder.position_at_end(diff_bb);
        let diff_ord = self.emit_icmp_ordering(tag_self, tag_other, "opt.cmp.tag", false);
        self.builder.br(merge_bb);
        let diff_bb_final = self.builder.current_block()?;

        // Tags equal: if None, Equal; if Some, compare payloads
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_none = self.builder.icmp_eq(tag_self, zero, "opt.cmp.is_none");

        let payload_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.payload");
        let none_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.none");

        self.builder.cond_br(is_none, none_bb, payload_bb);

        // Both None → Equal
        self.builder.position_at_end(none_bb);
        let equal_val = self.builder.const_i8(1);
        self.builder.br(merge_bb);
        let none_bb_final = self.builder.current_block()?;

        // Both Some → compare payloads
        self.builder.position_at_end(payload_bb);
        let pay_self = self.builder.extract_value(lhs, 1, "opt.cmp.pay.self")?;
        let pay_other = self.builder.extract_value(rhs, 1, "opt.cmp.pay.other")?;
        let payload_ord = self.emit_inner_compare(pay_self, pay_other, inner_type, "opt.cmp.inner");
        self.builder.br(merge_bb);
        let payload_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder.phi_from_incoming(
            i8_ty,
            &[
                (diff_ord, diff_bb_final),
                (equal_val, none_bb_final),
                (payload_ord, payload_bb_final),
            ],
            "opt.cmp.result",
        )
    }

    /// `Option.equals()`: tags must match, then compare payloads.
    pub(super) fn emit_option_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "opt.eq.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "opt.eq.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "opt.eq.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.check");
        let false_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.false");

        self.builder.cond_br(tags_eq, check_bb, false_bb);

        // Tags differ → false
        self.builder.position_at_end(false_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);
        let false_bb_final = self.builder.current_block()?;

        // Tags equal: if None → true, if Some → compare payloads
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_none = self.builder.icmp_eq(tag_self, zero, "opt.eq.is_none");

        let payload_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.payload");
        let none_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.none");

        self.builder.cond_br(is_none, none_bb, payload_bb);

        // Both None → true
        self.builder.position_at_end(none_bb);
        let true_val = self.builder.const_bool(true);
        self.builder.br(merge_bb);
        let none_bb_final = self.builder.current_block()?;

        // Both Some → compare payloads
        self.builder.position_at_end(payload_bb);
        let pay_self = self.builder.extract_value(lhs, 1, "opt.eq.pay.self")?;
        let pay_other = self.builder.extract_value(rhs, 1, "opt.eq.pay.other")?;
        let payload_eq = self.emit_inner_eq(pay_self, pay_other, inner_type, "opt.eq.inner");
        self.builder.br(merge_bb);
        let payload_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[
                (false_val, false_bb_final),
                (true_val, none_bb_final),
                (payload_eq, payload_bb_final),
            ],
            "opt.eq.result",
        )
    }

    /// `Option.hash()`: None → 0, Some(x) → `hash_combine(1, x.hash())`.
    pub(super) fn emit_option_hash(&mut self, recv: ValueId, inner_type: Idx) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "opt.hash.tag")?;
        let zero = self.builder.const_i8(0);
        let is_none = self.builder.icmp_eq(tag, zero, "opt.hash.is_none");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "opt.hash.merge");
        let some_bb = self
            .builder
            .append_block(self.current_function, "opt.hash.some");
        let none_bb = self
            .builder
            .append_block(self.current_function, "opt.hash.none");

        self.builder.cond_br(is_none, none_bb, some_bb);

        // None → 0
        self.builder.position_at_end(none_bb);
        let none_hash = self.builder.const_i64(0);
        self.builder.br(merge_bb);
        let none_bb_final = self.builder.current_block()?;

        // Some → hash_combine(1, payload.hash())
        self.builder.position_at_end(some_bb);
        let payload = self.builder.extract_value(recv, 1, "opt.hash.payload")?;
        let inner_hash = self.emit_inner_hash(payload, inner_type, "opt.hash.inner");
        let seed = self.builder.const_i64(1);
        let some_hash = self.emit_hash_combine(seed, inner_hash, "opt.hash.combine");
        self.builder.br(merge_bb);
        let some_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i64_ty = self.builder.i64_type();
        self.builder.phi_from_incoming(
            i64_ty,
            &[(none_hash, none_bb_final), (some_hash, some_bb_final)],
            "opt.hash.result",
        )
    }
}
