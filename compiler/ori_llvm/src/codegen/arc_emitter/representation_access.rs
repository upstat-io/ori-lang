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

    /// Encode a pointer and variant tag into one tagged-pointer slot.
    ///
    /// Pointer values and prior `ptrtoint` results are both accepted; unit
    /// variants use zero as their payload.
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

    /// Extract the low three variant-tag bits as an `i64`.
    ///
    /// Accepts either an integer slot or an LLVM pointer.
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

    /// Extract the high payload bits as an LLVM pointer.
    ///
    /// Unit variants produce null, so callers must check the variant tag
    /// before dereferencing the result.
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

    /// Return the niche encoding for an enum, including canonical fallback for
    /// variable-residue types absent from the representation plan.
    pub(super) fn get_niche_encoding(&self, ty: Idx) -> Option<tag_access::TagEncoding> {
        let enum_repr = self.enum_repr_for(ty)?;
        match &enum_repr.tag {
            ori_repr::EnumTag::Niche { .. } => {
                Some(tag_access::TagEncoding::from_enum_repr(&enum_repr))
            }
            ori_repr::EnumTag::Explicit { .. }
            | ori_repr::EnumTag::TaggedPtr
            | ori_repr::EnumTag::None => None,
        }
    }

    /// Resolve an enum representation from the plan or its canonical fallback.
    fn enum_repr_for(&self, ty: Idx) -> Option<std::borrow::Cow<'_, ori_repr::EnumRepr>> {
        self.repr_plan?.enum_repr_with_fallback(self.pool, ty)
    }

    /// Return whether `ty` is a tagless single-variant enum.
    pub(super) fn is_tagless_enum(&self, ty: Idx) -> bool {
        self.enum_repr_for(ty)
            .is_some_and(|enum_repr| matches!(enum_repr.tag, ori_repr::EnumTag::None))
    }

    /// Return the tagged-pointer encoding for an enum, when present.
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

    /// Reorder construction arguments from declaration order to memory order.
    ///
    /// Types without a planned aggregate representation retain declaration order.
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
        fields
            .iter()
            .map(|field| args[field.original_index as usize])
            .collect()
    }
}
