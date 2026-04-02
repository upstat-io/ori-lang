//! Tuple trait method codegen.
//!
//! Implements `equals`, `compare`, and `hash` for tuple types.
//!
//! ## ARC representation
//!
//! `Tuple` is a flat struct of resolved element types: `{A, B, ...}`.
//! Field access uses `remap_struct_field` to account for memory ordering.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// `Tuple.equals(other) -> bool`
    ///
    /// All fields must be equal (conjunction). Remap to memory order.
    pub(in crate::codegen::arc_emitter) fn emit_tuple_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
        tuple_ty: Idx,
    ) -> Option<ValueId> {
        let mut result = self.builder.const_bool(true);

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate() {
            let mem_i = self.remap_struct_field(tuple_ty, i as u32);
            let lhs_field = self.builder.extract_value(lhs, mem_i, "tup.lhs")?;
            let rhs_field = self.builder.extract_value(rhs, mem_i, "tup.rhs")?;
            let field_eq = self.emit_element_equals(lhs_field, rhs_field, elem_ty)?;
            result = self.builder.and(result, field_eq, "tup_eq");
        }

        Some(result)
    }

    /// `Tuple.compare(other) -> Ordering`
    ///
    /// Lexicographic: compare field 0, if Equal compare field 1, etc.
    /// Remap to memory order.
    pub(in super::super) fn emit_tuple_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
        tuple_ty: Idx,
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
            let mem_i = self.remap_struct_field(tuple_ty, i as u32);
            let lhs_field = self.builder.extract_value(lhs, mem_i, "tup.lhs")?;
            let rhs_field = self.builder.extract_value(rhs, mem_i, "tup.rhs")?;
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
    /// Fold `hash_combine` over field hashes. Remap to memory order.
    pub(in super::super) fn emit_tuple_hash(
        &mut self,
        val: ValueId,
        elements: &[Idx],
        tuple_ty: Idx,
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i64(0));
        }

        let mem_0 = self.remap_struct_field(tuple_ty, 0);
        let first_field = self.builder.extract_value(val, mem_0, "tup.f0")?;
        let mut result = self.emit_element_hash(first_field, elements[0])?;

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate().skip(1) {
            let mem_i = self.remap_struct_field(tuple_ty, i as u32);
            let field = self.builder.extract_value(val, mem_i, "tup.f")?;
            let field_hash = self.emit_element_hash(field, elem_ty)?;
            result = self.emit_hash_combine(result, field_hash);
        }

        Some(result)
    }
}
