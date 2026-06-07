//! Enum-specific LLVM type resolution.
//!
//! Contains `resolve_enum` and its three encoding-specific helpers:
//! explicit tag, tagless (single-variant), and niche-encoded.

use inkwell::types::BasicTypeEnum;

use ori_types::Idx;

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
            // Recursive back-edge: always a heap-boxed pointer, never the
            // named struct by-value (which would be infinitely sized).
            return self.scx.type_ptr().into();
        }

        // Check ReprPlan for niche/tagless encoding via the SSOT ladder
        // (plan-first, canonical fallback for variable-residue enum shapes).
        if let Some(enum_repr) = self
            .repr_plan
            .and_then(|p| p.enum_repr_with_fallback(self.store.pool(), idx))
        {
            if enum_repr.tag.is_tagless() {
                return self.resolve_enum_tagless(idx, variants.first());
            }
            if enum_repr.tag.is_niche() {
                return self.resolve_enum_niche(idx, variants, &enum_repr);
            }
            if enum_repr.tag.is_tagged_ptr() {
                return self.resolve_enum_tagged_ptr(idx);
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

    /// Resolve a tagged-pointer enum to a single LLVM `i64` value.
    ///
    /// The entire enum is encoded as `(payload_ptr | variant_tag)` in a
    /// 64-bit slot — there is no struct, no GEP, no per-variant LLVM type.
    /// All codegen consumers (Construct, Project, Switch, RC, drop, ABI)
    /// dispatch on `is_tagged_ptr` and use mask-based encode/decode.
    ///
    /// Cycle safety: tagged-pointer eligibility (`can_use_tagged_pointer`)
    /// requires every payload to be a single-word pointer with no field
    /// recursion, so a named-struct cycle escape is not needed here.
    fn resolve_enum_tagged_ptr(&self, _idx: Idx) -> BasicTypeEnum<'ll> {
        self.scx.type_i64().into()
    }

    /// Resolve enum with explicit tag: `{ tag, [M x i64] payload }`.
    fn resolve_enum_explicit(&self, idx: Idx, variants: &[EnumVariantInfo]) -> BasicTypeEnum<'ll> {
        let name = self.type_name(idx, "Enum");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

        // Enum payloads use [M x i64] layout where each field occupies
        // at least one full i64 slot (8 bytes). Must match
        // compute_variant_field_offsets in drop_enum.rs and
        // enum_payload_size / pool_type_store_size in ori_arc.
        //
        // Unit/Never fields are zero-sized in Ori's type system
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
                    // A recursive variant field is a boxed `ptr` (one i64 slot)
                    // per the boxing SSOT — sized from the box, not by recursing
                    // into the field type (which would inline a mutually- or
                    // self-recursive back-edge when resolved standalone).
                    let size = if self.position_is_rc_boxed(idx, f) {
                        Self::type_store_size(self.scx.type_ptr().into())
                    } else {
                        Self::type_store_size(self.resolve(f))
                    };
                    // Round up to 8-byte i64 slot boundary
                    size.div_ceil(8) * 8
                })
                .sum();
            max_payload_bytes = max_payload_bytes.max(variant_bytes);
        }

        // Use narrowed tag type (i8 for ≤256 variants) instead of i64.
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
    /// `EnumTag::None` means a single-variant enum. The LLVM type is
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
        //
        // A recursive back-edge field is a heap-boxed `ptr` per the boxing SSOT
        // (`position_is_rc_boxed`) — resolving it standalone would inline a
        // self/mutually-recursive type. Mirror `resolve_struct_field`: box it
        // to `ptr` so Construct/Project/drop/RC (which consult the same oracle)
        // agree with the layout.
        if let Some(variant) = variant {
            let field_types: Vec<BasicTypeEnum<'ll>> = variant
                .fields
                .iter()
                .filter(|&&f| self.is_non_void_field(f))
                .map(|&f| {
                    if self.position_is_rc_boxed(idx, f) {
                        self.scx.type_ptr().into()
                    } else {
                        self.resolve(f)
                    }
                })
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
    /// `EnumTag::Niche` means the discriminant is encoded in an invalid
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
        super::field_is_non_void(self.store.pool(), field)
    }
}
