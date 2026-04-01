//! Enum-specific LLVM type resolution.
//!
//! Extracted from `layout_resolver.rs` for file size hygiene.
//! Contains `resolve_enum()` and its three encoding-specific helpers:
//! explicit tag, tagless (single-variant), and niche-encoded.

use inkwell::types::BasicTypeEnum;

use ori_types::{Idx, Tag};

use super::info::EnumVariantInfo;
use super::layout_resolver::TypeLayoutResolver;

impl<'ll> TypeLayoutResolver<'_, 'll, '_> {
    /// Resolve an enum type with two-phase creation for cycle safety.
    ///
    /// Layout: `{ i64 tag, [M x i64] payload }` where M is enough i64s to
    /// hold the largest variant's fields. All-unit enums omit the payload.
    ///
    /// Invariant: `TypeInfo::Enum.variants` and `EnumRepr.variants` use the
    /// same ordering — the type checker's logical variant indices.
    /// `EnumTag::Niche.niche_variant_idx` indexes into this shared order.
    pub(super) fn resolve_enum(
        &self,
        idx: Idx,
        variants: &[EnumVariantInfo],
    ) -> BasicTypeEnum<'ll> {
        if self.resolving.borrow().contains(&idx) {
            if let Some(&named) = self.named_structs.borrow().get(&idx) {
                return named.into();
            }
            return self.scx.type_ptr().into();
        }

        // §07.2: Check ReprPlan for niche/tagless encoding.
        // Use resolve_fully to match the Idx used by canonical population.
        let resolved_idx = self.store.pool().resolve_fully(idx);
        if let Some(enum_repr) = self.repr_plan.and_then(|p| p.get_enum_repr(resolved_idx)) {
            if enum_repr.tag.is_tagless() {
                return self.resolve_enum_tagless(idx, variants.first());
            }
            if enum_repr.tag.is_niche() {
                return self.resolve_enum_niche(idx, variants, enum_repr);
            }
            // Explicit tag — fall through to existing behavior.
        } else if self.repr_plan.is_some() {
            tracing::warn!(
                ?idx,
                "enum type has no ReprPlan entry — falling back to explicit tag"
            );
        }

        self.resolve_enum_explicit(idx, variants)
    }

    /// Resolve enum with explicit tag: `{ tag, [M x i64] payload }`.
    fn resolve_enum_explicit(&self, idx: Idx, variants: &[EnumVariantInfo]) -> BasicTypeEnum<'ll> {
        let name = self.type_name(idx, "Enum");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

        // Enum payloads use [M x i64] layout where each field occupies
        // at least one full i64 slot (8 bytes). Must match
        // compute_variant_field_offsets() in drop_enum.rs and
        // enum_payload_size() / pool_type_store_size() in ori_arc.
        //
        // BUG-04-008: Unit/Never fields are zero-sized in Ori's type system
        // but map to i64 in LLVM (because LLVM void can't be stored/phi'd).
        // Skip them here so they don't inflate the payload size.
        let mut max_payload_bytes: u64 = 0;
        for variant in variants {
            let variant_bytes: u64 = variant
                .fields
                .iter()
                .map(|&f| {
                    if !self.is_non_void_field(f) {
                        return 0;
                    }
                    let ty = self.resolve(f);
                    let size = Self::type_store_size(ty);
                    // Round up to 8-byte i64 slot boundary
                    size.div_ceil(8) * 8
                })
                .sum();
            max_payload_bytes = max_payload_bytes.max(variant_bytes);
        }

        // §07.1: Use narrowed tag type (i8 for ≤256 variants) instead of i64.
        let tag_ty = match ori_repr::min_tag_width(variants.len()) {
            ori_repr::IntWidth::I8 => self.scx.type_i8(),
            ori_repr::IntWidth::I16 => self.scx.type_i16(),
            ori_repr::IntWidth::I32 => self.scx.type_i32(),
            ori_repr::IntWidth::I64 => self.scx.type_i64(),
        };
        if max_payload_bytes == 0 {
            self.scx
                .set_struct_body(named_struct, &[tag_ty.into()], false);
        } else {
            let payload_i64_count = max_payload_bytes.div_ceil(8);
            let payload_ty = self.scx.type_i64().array_type(payload_i64_count as u32);
            self.scx
                .set_struct_body(named_struct, &[tag_ty.into(), payload_ty.into()], false);
        }

        self.resolving.borrow_mut().remove(&idx);
        named_struct.into()
    }

    /// Resolve single-variant enum (newtype erasure): no tag, struct IS the payload.
    ///
    /// §07.2: `EnumTag::None` means a single-variant enum. The LLVM type is
    /// just the payload fields — no tag field at all.
    fn resolve_enum_tagless(
        &self,
        idx: Idx,
        variant: Option<&EnumVariantInfo>,
    ) -> BasicTypeEnum<'ll> {
        let name = self.type_name(idx, "Enum");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

        // Single variant — resolve its fields as the struct body.
        // Tagless variants typically have 1-2 fields; Vec allocation is minimal.
        if let Some(variant) = variant {
            let field_types: Vec<BasicTypeEnum<'ll>> = variant
                .fields
                .iter()
                .filter(|&&f| self.is_non_void_field(f))
                .map(|&f| self.resolve(f))
                .collect();

            if field_types.is_empty() {
                // Unit newtype — use i8 as a placeholder (ZST in Ori, but
                // LLVM needs a non-empty struct for named types).
                self.scx
                    .set_struct_body(named_struct, &[self.scx.type_i8().into()], false);
            } else {
                self.scx.set_struct_body(named_struct, &field_types, false);
            }
        } else {
            // Empty enum (no variants) — shouldn't happen, but handle gracefully.
            self.scx
                .set_struct_body(named_struct, &[self.scx.type_i8().into()], false);
        }

        self.resolving.borrow_mut().remove(&idx);
        named_struct.into()
    }

    /// Resolve niche-encoded enum: no explicit tag field, payload IS the struct.
    ///
    /// §07.2: `EnumTag::Niche` means the discriminant is encoded in an invalid
    /// bit pattern of a payload field. The LLVM type is the data variant's
    /// payload (same layout as the inner type for simple cases like `Option<bool>`).
    fn resolve_enum_niche(
        &self,
        idx: Idx,
        variants: &[EnumVariantInfo],
        enum_repr: &ori_repr::EnumRepr,
    ) -> BasicTypeEnum<'ll> {
        let name = self.type_name(idx, "Enum");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

        // Find the data variant (the non-niche variant with payload fields).
        // For 2-variant niche enums, the data variant is the one that is NOT
        // the niche variant. For future N-variant niche encoding, this logic
        // will need to iterate — add new cases here when that's implemented.
        let niche_variant_idx = match &enum_repr.tag {
            ori_repr::EnumTag::Niche {
                niche_variant_idx, ..
            } => *niche_variant_idx,
            _ => 0,
        };
        debug_assert!(
            (niche_variant_idx as usize) < variants.len(),
            "niche_variant_idx {niche_variant_idx} out of bounds for {}-variant enum",
            variants.len()
        );
        let data_variant_idx = u32::from(niche_variant_idx == 0);
        let variant = &variants[data_variant_idx as usize];

        let field_types: Vec<BasicTypeEnum<'ll>> = variant
            .fields
            .iter()
            .filter(|&&f| self.is_non_void_field(f))
            .map(|&f| self.resolve(f))
            .collect();

        if field_types.is_empty() {
            // Niche on a unit type — use i8 as a placeholder.
            self.scx
                .set_struct_body(named_struct, &[self.scx.type_i8().into()], false);
        } else {
            self.scx.set_struct_body(named_struct, &field_types, false);
        }

        self.resolving.borrow_mut().remove(&idx);
        named_struct.into()
    }

    /// Whether a field type is non-void (not Unit or Never).
    ///
    /// Unit/Never fields are zero-sized in Ori but map to i64 in LLVM
    /// (because LLVM void can't be stored/phi'd). Used by enum layout
    /// methods to skip phantom fields that don't contribute to payload size.
    fn is_non_void_field(&self, field: Idx) -> bool {
        let pool = self.store.pool();
        let resolved = pool.resolve_fully(field);
        let tag = pool.tag(resolved);
        !matches!(tag, Tag::Unit | Tag::Never)
    }
}
