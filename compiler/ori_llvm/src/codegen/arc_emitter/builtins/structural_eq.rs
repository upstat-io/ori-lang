//! Structural `Eq` fallback codegen for user structs/enums that lack
//! `#derive(Eq)`. Field-by-field (struct) and tag+homogeneous-payload (enum)
//! comparison, dispatched recursively through [`ArcIrEmitter::emit_element_equals`].

use crate::codegen::type_info::EnumVariantInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit structural field-by-field equality for a struct without `#derive(Eq)`.
    ///
    /// Compares each field using `emit_element_equals` recursively, with
    /// short-circuit AND semantics (returns false on first mismatch).
    /// Falls back to integer comparison if field types are unknown.
    pub(super) fn emit_structural_eq(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        fields: &[(ori_ir::Name, ori_types::Idx)],
    ) -> Option<ValueId> {
        if fields.is_empty() {
            return Some(self.builder.const_bool(true));
        }

        // Multi-field: accumulate AND of all field comparisons.
        // Branch-free AND chain — most structs have 2-5 fields.
        let mut result = None;
        for (i, &(_, field_ty)) in fields.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field index from type definition, always small"
            )]
            let idx = i as u32;
            let lhs_f = self.builder.extract_value(lhs, idx, &format!("seq.l.{i}"));
            let rhs_f = self.builder.extract_value(rhs, idx, &format!("seq.r.{i}"));
            let (Some(lhs_f), Some(rhs_f)) = (lhs_f, rhs_f) else {
                continue; // Skip fields that can't be extracted (void/unit)
            };
            let Some(field_eq) = self.emit_element_equals(lhs_f, rhs_f, field_ty) else {
                // Field type can't be compared (e.g., enum without #derive(Eq)).
                // Structural equality is not possible for this struct.
                return None;
            };
            result = Some(match result {
                None => field_eq,
                Some(acc) => self.builder.and(acc, field_eq, &format!("seq.and.{i}")),
            });
        }
        Some(result.unwrap_or_else(|| self.builder.const_bool(true)))
    }

    /// Emit structural equality for an enum without `#derive(Eq)`.
    ///
    /// Compares tags first — if different, returns false. If same tag:
    /// - Unit-only enums: tag comparison is sufficient
    /// - Homogeneous payload enums (all payload variants share field types):
    ///   compare payload fields directly (safe because tags match)
    /// - Heterogeneous payload enums: return None (need `#derive(Eq)`)
    pub(super) fn emit_structural_eq_enum(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        variants: &[EnumVariantInfo],
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "eeq.ltag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "eeq.rtag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "eeq.tags");

        // Unit-only enums: tag comparison is the full answer
        let payload_variants: Vec<_> = variants.iter().filter(|v| !v.fields.is_empty()).collect();
        if payload_variants.is_empty() {
            return Some(tags_eq);
        }

        // Check homogeneity: all payload variants must have same field types.
        // This is safe because tags already match — we know which variant we're
        // comparing, and the LLVM payload union shares the same layout.
        let first_fields = &payload_variants[0].fields;
        let homogeneous = payload_variants
            .iter()
            .all(|v| v.fields.len() == first_fields.len() && v.fields == *first_fields);
        if !homogeneous {
            return None;
        }

        // Restrict to scalar-only payloads — aggregate fields (lists, maps, sets,
        // tuples, structs) are stored as multi-slot i64 arrays and can't be
        // reinterpreted via reinterpret_from_i64. Use #derive(Eq) for those.
        let all_scalar = first_fields.iter().all(|ty| {
            let llvm_ty = self.resolve_type(*ty);
            self.builder.is_single_slot_type(llvm_ty)
        });
        if !all_scalar {
            return None;
        }

        // Extract payload (index 1) then compare fields within it.
        // Enum LLVM layout: {i64 tag, [N x i64] payload_union}
        // Payload is array type — use extract_value_any (handles arrays + structs).
        let lhs_payload = self.builder.extract_value_any(lhs, 1, "eeq.lpay");
        let rhs_payload = self.builder.extract_value_any(rhs, 1, "eeq.rpay");

        let mut field_eq = tags_eq;
        for (fi, field_ty) in first_fields.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field index from type definition, always small"
            )]
            let field_idx = fi as u32;
            let lhs_f =
                self.builder
                    .extract_value_any(lhs_payload, field_idx, &format!("eeq.l.{fi}"));
            let rhs_f =
                self.builder
                    .extract_value_any(rhs_payload, field_idx, &format!("eeq.r.{fi}"));
            // Payload fields are stored as i64 slots — reinterpret to field type
            let llvm_ty = self.resolve_type(*field_ty);
            let lhs_f = self
                .builder
                .reinterpret_from_i64(lhs_f, llvm_ty, &format!("eeq.rl.{fi}"));
            let rhs_f = self
                .builder
                .reinterpret_from_i64(rhs_f, llvm_ty, &format!("eeq.rr.{fi}"));
            if let Some(feq) = self.emit_element_equals(lhs_f, rhs_f, *field_ty) {
                field_eq = self.builder.and(field_eq, feq, &format!("eeq.f{fi}"));
            }
        }
        Some(field_eq)
    }
}
