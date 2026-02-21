//! Tuple type method lowering: len, compare, equals, hash, clone.

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    pub(super) fn lower_tuple_method(
        &mut self,
        recv: ValueId,
        elements: &[Idx],
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "clone" => Some(recv),
            "len" => Some(self.builder.const_i64(elements.len() as i64)),
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_tuple_compare(recv, other, elements)
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_tuple_equals(recv, other, elements)
            }
            "hash" => self.emit_tuple_hash(recv, elements),
            _ => None,
        }
    }

    /// `Tuple.compare()`: lexicographic field comparison.
    ///
    /// Uses phi merging at a final block — we can't emit `ret` since this
    /// is inline in the caller's function, not a standalone derived method.
    pub(super) fn emit_tuple_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i8(1)); // Equal
        }

        let merge_bb = self
            .builder
            .append_block(self.current_function, "tup.cmp.merge");
        let mut incoming = Vec::with_capacity(elements.len() + 1);

        for (i, &elem_type) in elements.iter().enumerate() {
            let lhs_field = self
                .builder
                .extract_value(lhs, i as u32, &format!("tup.cmp.l.{i}"))?;
            let rhs_field = self
                .builder
                .extract_value(rhs, i as u32, &format!("tup.cmp.r.{i}"))?;

            let ord =
                self.emit_inner_compare(lhs_field, rhs_field, elem_type, &format!("tup.cmp.{i}"));

            let one = self.builder.const_i8(1);
            let is_eq = self
                .builder
                .icmp_eq(ord, one, &format!("tup.cmp.{i}.is_eq"));

            if i + 1 < elements.len() {
                let early_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.cmp.early.{i}"));
                let next_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.cmp.next.{}", i + 1));
                self.builder.cond_br(is_eq, next_bb, early_bb);

                // Non-equal: jump to merge with this ordering
                self.builder.position_at_end(early_bb);
                self.builder.br(merge_bb);
                incoming.push((ord, self.builder.current_block()?));

                self.builder.position_at_end(next_bb);
            } else {
                let early_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.cmp.early.{i}"));
                let equal_bb = self
                    .builder
                    .append_block(self.current_function, "tup.cmp.equal");
                self.builder.cond_br(is_eq, equal_bb, early_bb);

                // Non-equal: jump to merge
                self.builder.position_at_end(early_bb);
                self.builder.br(merge_bb);
                incoming.push((ord, self.builder.current_block()?));

                // All fields equal
                self.builder.position_at_end(equal_bb);
                let equal_val = self.builder.const_i8(1);
                self.builder.br(merge_bb);
                incoming.push((equal_val, self.builder.current_block()?));
            }
        }

        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder
            .phi_from_incoming(i8_ty, &incoming, "tup.cmp.result")
    }

    /// `Tuple.equals()`: field-wise equality with short-circuit.
    pub(super) fn emit_tuple_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_bool(true));
        }

        let true_bb = self
            .builder
            .append_block(self.current_function, "tup.eq.true");
        let false_bb = self
            .builder
            .append_block(self.current_function, "tup.eq.false");

        for (i, &elem_type) in elements.iter().enumerate() {
            let lhs_field = self
                .builder
                .extract_value(lhs, i as u32, &format!("tup.eq.l.{i}"))?;
            let rhs_field = self
                .builder
                .extract_value(rhs, i as u32, &format!("tup.eq.r.{i}"))?;

            let eq = self.emit_inner_eq(lhs_field, rhs_field, elem_type, &format!("tup.eq.{i}"));

            if i + 1 < elements.len() {
                let next_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.eq.next.{}", i + 1));
                self.builder.cond_br(eq, next_bb, false_bb);
                self.builder.position_at_end(next_bb);
            } else {
                self.builder.cond_br(eq, true_bb, false_bb);
            }
        }

        let merge_bb = self
            .builder
            .append_block(self.current_function, "tup.eq.merge");

        self.builder.position_at_end(true_bb);
        let true_val = self.builder.const_bool(true);
        self.builder.br(merge_bb);
        let true_bb_final = self.builder.current_block()?;

        self.builder.position_at_end(false_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);
        let false_bb_final = self.builder.current_block()?;

        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[(true_val, true_bb_final), (false_val, false_bb_final)],
            "tup.eq.result",
        )
    }

    /// `Tuple.hash()`: FNV-1a over all field hashes.
    pub(super) fn emit_tuple_hash(&mut self, recv: ValueId, elements: &[Idx]) -> Option<ValueId> {
        let mut hash = self.builder.const_i64(0);

        for (i, &elem_type) in elements.iter().enumerate() {
            let field_val = self
                .builder
                .extract_value(recv, i as u32, &format!("tup.hash.{i}"))?;
            let field_hash = self.emit_inner_hash(field_val, elem_type, &format!("tup.hash.{i}"));
            hash = self.emit_hash_combine(hash, field_hash, &format!("tup.hash.combine.{i}"));
        }

        Some(hash)
    }
}
