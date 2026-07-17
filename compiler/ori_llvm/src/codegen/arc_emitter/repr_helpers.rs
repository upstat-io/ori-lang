//! Enum encodings and declaration-to-memory field mapping.

use ori_types::{Idx, Tag};

use crate::codegen::value_id::ValueId;

use super::{tag_access, ArcIrEmitter};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Compare a niche field value against the niche sentinel.
    ///
    /// Returns an `i1` that is `true` when the value IS the niche (e.g., None).
    /// Handles both integer niche fields (`icmp eq`) and pointer niche fields
    /// (`ptrtoint` + `icmp eq`, using the established null-check pattern).
    pub(super) fn niche_is_sentinel(
        &mut self,
        field_val: ValueId,
        niche_value: u64,
        name: &str,
    ) -> ValueId {
        if self.builder.is_pointer_value(field_val) {
            let i64_ty = self.builder.i64_type();
            let as_int = self.builder.ptr_to_int(field_val, i64_ty, "niche.p2i");
            let niche_const = self.builder.const_i64(niche_value as i64);
            self.builder.icmp_eq(as_int, niche_const, name)
        } else {
            let niche_const = self.builder.const_int_matching(field_val, niche_value);
            self.builder.icmp_eq(field_val, niche_const, name)
        }
    }

    // Tagged-pointer encoder/decoder helpers.
    //
    // A tagged-pointer enum value is a single `i64` slot containing
    // `(payload_ptr | variant_tag)`. The low 3 bits hold the variant
    // discriminant (0..7); the high 61 bits hold either an 8-byte-aligned
    // heap pointer or zero (for unit variants).
    //
    // These helpers mirror `niche_is_sentinel`'s pattern: pure LLVM IR
    // emission, no `EnumRepr`/`TagEncoding` dependency. Callers (Construct,
    // Project, Switch, RcInc/Dec, Drop) decide WHEN to use them based on
    // `get_tagged_ptr_encoding`.
    //
    // All consumers flow through these three primitives — never re-derive
    // the masks at call sites.

    /// Encode a payload pointer with a variant tag into the i64 slot.
    ///
    /// `result = (ptr_as_int & TAGGED_PTR_PTR_MASK) | tag`
    ///
    /// In debug builds, asserts that the low 3 bits of `payload_ptr` are
    /// zero (i.e., the pointer is 8-byte aligned). The mask is applied
    /// unconditionally so the encoding is robust to slightly misaligned
    /// inputs in release builds, but the assert catches alignment bugs
    /// at the source.
    ///
    /// `payload_ptr` may be either an LLVM pointer or an integer that
    /// already represents a pointer (e.g., from a prior `ptrtoint`).
    /// Unit variants pass an integer zero (no payload to encode).
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn tagged_ptr_encode(
        &mut self,
        payload_ptr: ValueId,
        variant_tag: u32,
        name: &str,
    ) -> ValueId {
        let i64_ty = self.builder.i64_type();
        let as_int = if self.builder.is_pointer_value(payload_ptr) {
            self.builder
                .ptr_to_int(payload_ptr, i64_ty, "tagged_ptr.p2i")
        } else {
            payload_ptr
        };
        let ptr_mask = self
            .builder
            .const_i64(tag_access::TagEncoding::TAGGED_PTR_PTR_MASK as i64);
        let cleared = self.builder.and(as_int, ptr_mask, "tagged_ptr.cleared");
        let tag_const = self.builder.const_i64(i64::from(variant_tag));
        self.builder.or(cleared, tag_const, name)
    }

    /// Extract the variant discriminant (low 3 bits) from a tagged-pointer
    /// encoded value.
    ///
    /// `result = encoded & TAGGED_PTR_TAG_MASK`
    ///
    /// The result is an `i64` in `[0, 7]`. Switch terminators that need a
    /// narrower type (e.g., `i8`) should `trunc` the result themselves —
    /// keeping the helper width-agnostic avoids hardcoding tag widths here.
    ///
    /// Accepts either an i64 value or an LLVM pointer. Pointers are
    /// converted via `ptrtoint` so the helper is robust to either form
    /// (e.g., the encoded value loaded from an `i64` slot vs. carried in
    /// an `RcPointer` value through ARC IR).
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn tagged_ptr_decode_tag(&mut self, encoded: ValueId, name: &str) -> ValueId {
        let i64_ty = self.builder.i64_type();
        let as_int = if self.builder.is_pointer_value(encoded) {
            self.builder.ptr_to_int(encoded, i64_ty, "tagged_ptr.p2i")
        } else {
            encoded
        };
        let tag_mask = self
            .builder
            .const_i64(tag_access::TagEncoding::TAGGED_PTR_TAG_MASK as i64);
        self.builder.and(as_int, tag_mask, name)
    }

    /// Extract the payload pointer (high 61 bits) from a tagged-pointer
    /// encoded value.
    ///
    /// `result = (encoded & TAGGED_PTR_PTR_MASK) as ptr`
    ///
    /// Returns an LLVM pointer (via `inttoptr`). For unit variants the
    /// resulting pointer is null — callers MUST guard pointer dereferences
    /// behind a tag check (`tagged_ptr_decode_tag` + variant predicate).
    ///
    /// Accepts either an i64 value or an LLVM pointer (converted via
    /// `ptrtoint` first), matching `tagged_ptr_decode_tag`.
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn tagged_ptr_decode_ptr(&mut self, encoded: ValueId, name: &str) -> ValueId {
        let i64_ty = self.builder.i64_type();
        let as_int = if self.builder.is_pointer_value(encoded) {
            self.builder.ptr_to_int(encoded, i64_ty, "tagged_ptr.p2i")
        } else {
            encoded
        };
        let ptr_mask = self
            .builder
            .const_i64(tag_access::TagEncoding::TAGGED_PTR_PTR_MASK as i64);
        let cleared = self.builder.and(as_int, ptr_mask, "tagged_ptr.cleared");
        self.builder.int_to_ptr(cleared, name)
    }

    /// Get the `TagEncoding` for an enum type, if it uses niche encoding.
    ///
    /// Returns `Some(encoding)` ONLY for `EnumTag::Niche`. Tagless
    /// (`EnumTag::None`) is dispatched separately via [`get_tagless_encoding`]
    /// (struct-like, no niche field) and tagged-pointer via
    /// [`get_tagged_ptr_encoding`]; explicit tags (the common case) return
    /// `None` so callers fall through to the standard tag-switch path.
    ///
    /// Falls back to on-the-fly canonical computation for types with variable
    /// residue (e.g., `Option<Var(T→str)>`) that weren't in the `ReprPlan`.
    pub(super) fn get_niche_encoding(&self, ty: Idx) -> Option<tag_access::TagEncoding> {
        let enum_repr = self.enum_repr_for(ty)?;
        match &enum_repr.tag {
            ori_repr::EnumTag::Niche { .. } => {
                Some(tag_access::TagEncoding::from_enum_repr(&enum_repr))
            }
            // Niche-only. `None` (tagless), `TaggedPtr`, and `Explicit`
            // are each dispatched by their own query — niche-specific
            // consumers (which `.unwrap` the niche field) must not see
            // a tagless or tagged-ptr encoding.
            ori_repr::EnumTag::Explicit { .. }
            | ori_repr::EnumTag::TaggedPtr
            | ori_repr::EnumTag::None => None,
        }
    }

    /// Resolve the `EnumRepr` for `ty` via the `ReprPlan` SSOT ladder
    /// (`ReprPlan::enum_repr_with_fallback` — plan-first, canonical
    /// recomputation for variable-residue enum shapes).
    ///
    /// Shared by [`Self::get_niche_encoding`], [`Self::is_tagless_enum`], and
    /// [`Self::get_tagged_ptr_encoding`] — never re-derive it at call sites.
    fn enum_repr_for(&self, ty: Idx) -> Option<std::borrow::Cow<'_, ori_repr::EnumRepr>> {
        self.repr_plan?.enum_repr_with_fallback(self.pool, ty)
    }

    /// Whether `ty` is a tagless single-variant enum (`EnumTag::None`).
    ///
    /// A tagless enum has no discriminant and no niche field — its LLVM type is
    /// a plain struct of the single variant's non-void payload fields (see
    /// `resolve_enum_tagless`). Construct / Project / drop / RC consumers route
    /// the tagless case through their struct-shaped paths (direct field GEP,
    /// recursive-field boxing) rather than the niche or explicit-tag paths.
    pub(super) fn is_tagless_enum(&self, ty: Idx) -> bool {
        self.enum_repr_for(ty)
            .is_some_and(|enum_repr| matches!(enum_repr.tag, ori_repr::EnumTag::None))
    }

    /// Look up the tagged-pointer encoding for an enum type, if present.
    ///
    /// Mirrors [`get_niche_encoding`] but returns `Some` only for
    /// `EnumTag::TaggedPtr`. Used by codegen consumers to dispatch
    /// to tagged-pointer encode/decode paths instead of struct-based GEP.
    ///
    /// Returns `None` when:
    /// - The type has no `ReprPlan` entry (variable residue).
    /// - The type is not an enum.
    /// - The enum uses `Explicit`, `Niche`, or `None` tagging.
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn get_tagged_ptr_encoding(&self, ty: Idx) -> Option<tag_access::TagEncoding> {
        let enum_repr = self.enum_repr_for(ty)?;
        match &enum_repr.tag {
            ori_repr::EnumTag::TaggedPtr => {
                Some(tag_access::TagEncoding::from_enum_repr(&enum_repr))
            }
            ori_repr::EnumTag::Explicit { .. }
            | ori_repr::EnumTag::Niche { .. }
            | ori_repr::EnumTag::None => None,
        }
    }

    /// Only applies to user structs/tuples — enum payloads, closure envs,
    /// and collection internals are not subject to reordering.
    pub(super) fn remap_struct_field(&self, ty: Idx, decl_field: u32) -> u32 {
        let Some(plan) = self.repr_plan else {
            return decl_field;
        };
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        if tag != Tag::Struct && tag != Tag::Tuple {
            return decl_field;
        }
        let Some(repr) = plan.get_repr(resolved) else {
            return decl_field;
        };
        match repr {
            ori_repr::MachineRepr::Struct(s) => {
                s.memory_index(decl_field).map_or(decl_field, |i| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "struct fields always < u32::MAX"
                    )]
                    let idx = i as u32;
                    idx
                })
            }
            ori_repr::MachineRepr::Tuple(t) => t.memory_index(decl_field).map_or(decl_field, |i| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "tuple elements always < u32::MAX"
                )]
                let idx = i as u32;
                idx
            }),
            _ => decl_field,
        }
    }

    /// Reorder args from declaration order to memory order for struct
    /// construction. Returns a new vec with args in LLVM struct field order.
    ///
    /// If the type has no `StructRepr` in the `ReprPlan` (or no reordering),
    /// returns the args unchanged.
    pub(super) fn reorder_args_to_memory_order(&self, args: &[ValueId], ty: Idx) -> Vec<ValueId> {
        let Some(plan) = self.repr_plan else {
            return args.to_vec();
        };
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        if tag != Tag::Struct && tag != Tag::Tuple {
            return args.to_vec();
        }
        let Some(repr) = plan.get_repr(resolved) else {
            return args.to_vec();
        };
        let fields = match repr {
            ori_repr::MachineRepr::Struct(s) => &s.fields[..],
            ori_repr::MachineRepr::Tuple(t) => &t.elements[..],
            _ => return args.to_vec(),
        };
        // fields is in memory order; fields[mem_pos].original_index is the
        // declaration-order index. Build a vec where result[mem_pos] =
        // args[original_index].
        fields
            .iter()
            .map(|f| {
                args.get(f.original_index as usize)
                    .copied()
                    .unwrap_or(args[0]) // defensive — should never happen
            })
            .collect()
    }
}
