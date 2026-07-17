//! Value construction emission for [`ArcIrEmitter`].
//!
//! Handles `Construct` instructions: building structs/tuples, enum variants
//! (with recursive field boxing), list literals, map literals, and set literals.

use ori_arc::ir::{ArcVarId, CtorKind};
use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

struct EffectiveVariantArgs {
    values: Vec<ValueId>,
    arc_vars: Vec<ArcVarId>,
    field_types: Vec<Idx>,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a `Construct` instruction.
    pub(super) fn emit_construct(
        &mut self,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ArcVarId],
    ) -> ValueId {
        let arg_vals = args.iter().map(|arg| self.var(*arg)).collect::<Vec<_>>();
        let llvm_ty = self.resolve_type(ty);

        match ctor {
            CtorKind::Struct(_) | CtorKind::Tuple => {
                self.emit_struct_or_tuple_construct(ty, llvm_ty, &arg_vals, args)
            }
            CtorKind::EnumVariant { variant, .. } => {
                self.emit_enum_variant_construct(ty, llvm_ty, *variant, &arg_vals, args)
            }
            CtorKind::ListLiteral => self.emit_list_literal_construct(ty, llvm_ty, &arg_vals),
            CtorKind::MapLiteral => self.emit_map_literal_construct(ty, llvm_ty, &arg_vals),
            CtorKind::SetLiteral => self.emit_set_literal_construct(ty, llvm_ty, &arg_vals),
            CtorKind::Closure { .. } => {
                unreachable!("closures use PartialApply, not Construct")
            }
        }
    }

    fn emit_struct_or_tuple_construct(
        &mut self,
        ty: Idx,
        llvm_ty: LLVMTypeId,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
    ) -> ValueId {
        let resolved_ty = self.pool.resolve_fully(ty);
        if matches!(self.pool.tag(resolved_ty), Tag::Unit | Tag::Never) {
            return self.builder.const_zero_ty(llvm_ty);
        }

        let field_types = self.struct_field_types(resolved_ty);
        let has_boxed_fields = field_types
            .iter()
            .any(|&field_ty| is_boxed_enum_field(self.pool, ty, field_ty));
        let boxed_args = if has_boxed_fields {
            arg_vals
                .iter()
                .enumerate()
                .map(|(index, &value)| match field_types.get(index).copied() {
                    Some(field_ty) if is_boxed_enum_field(self.pool, ty, field_ty) => {
                        self.box_recursive_field(value, field_ty, arc_args.get(index).copied())
                    }
                    _ => value,
                })
                .collect::<Vec<_>>()
        } else {
            arg_vals.to_vec()
        };

        let memory_args = self.reorder_args_to_memory_order(&boxed_args, ty);
        let narrowed_args = self.trunc_for_narrowed_struct(llvm_ty, &memory_args, ty);
        self.builder.build_struct(llvm_ty, &narrowed_args, "ctor")
    }

    fn emit_enum_variant_construct(
        &mut self,
        ty: Idx,
        llvm_ty: LLVMTypeId,
        variant: u32,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
    ) -> ValueId {
        if self.get_tagged_ptr_encoding(ty).is_some() {
            let payload = match arg_vals {
                [] => self.builder.const_i64(0),
                [payload] => *payload,
                _ => unreachable!("tagged-pointer variant must have at most one field"),
            };
            return self.tagged_ptr_encode(payload, variant, "tagged.ctor");
        }
        if self.is_tagless_enum(ty) {
            return self.emit_tagless_variant_construct(llvm_ty, ty, arg_vals, arc_args);
        }
        if self.builder.is_scalar_int_type(llvm_ty) {
            return self.builder.const_int_of_type(llvm_ty, u64::from(variant));
        }
        if let Some(encoding) = self.get_niche_encoding(ty) {
            return self.emit_niche_variant_construct(llvm_ty, arg_vals, &encoding, variant);
        }

        let tag = self
            .builder
            .const_int_for_struct_field(llvm_ty, 0, u64::from(variant));
        let field_types = self.variant_field_types(ty, variant);
        let effective = self.effective_variant_args(arg_vals, arc_args, field_types);
        let has_boxed_fields = effective
            .field_types
            .iter()
            .any(|&field_ty| is_boxed_enum_field(self.pool, ty, field_ty));

        if has_boxed_fields {
            self.emit_variant_via_alloca(
                llvm_ty,
                ty,
                tag,
                &effective.values,
                &effective.arc_vars,
                &effective.field_types,
            )
        } else {
            self.emit_variant_via_insertvalue(
                llvm_ty,
                ty,
                tag,
                &effective.values,
                &effective.arc_vars,
                &effective.field_types,
            )
        }
    }

    fn variant_field_types(&self, ty: Idx, variant: u32) -> Vec<Idx> {
        let resolved = self.pool.resolve_fully(ty);
        match self.pool.tag(resolved) {
            Tag::Enum => {
                let variants = self.pool.enum_variants(resolved);
                let Some((_, fields)) = variants.get(variant as usize) else {
                    unreachable!("Construct variant {variant} out of bounds for enum type")
                };
                fields.clone()
            }
            Tag::Option if variant == 0 => vec![self.pool.option_inner(resolved)],
            Tag::Option if variant == 1 => Vec::new(),
            Tag::Result if variant == 0 => vec![self.pool.result_ok(resolved)],
            Tag::Result if variant == 1 => vec![self.pool.result_err(resolved)],
            Tag::Option | Tag::Result => {
                unreachable!("Construct variant {variant} out of bounds for {resolved:?}")
            }
            unexpected => {
                unreachable!(
                    "explicit-tag Construct requires enum/Option/Result, got {unexpected:?}"
                )
            }
        }
    }

    fn effective_variant_args(
        &self,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        field_types: Vec<Idx>,
    ) -> EffectiveVariantArgs {
        let is_void = |field_ty| {
            let resolved = self.pool.resolve_fully(field_ty);
            matches!(self.pool.tag(resolved), Tag::Unit | Tag::Never)
        };
        if !field_types.iter().copied().any(is_void) {
            return EffectiveVariantArgs {
                values: arg_vals.to_vec(),
                arc_vars: arc_args.to_vec(),
                field_types,
            };
        }

        let non_void = field_types
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, field_ty)| !is_void(*field_ty))
            .collect::<Vec<_>>();
        EffectiveVariantArgs {
            values: non_void.iter().map(|(index, _)| arg_vals[*index]).collect(),
            arc_vars: non_void.iter().map(|(index, _)| arc_args[*index]).collect(),
            field_types: non_void.into_iter().map(|(_, field_ty)| field_ty).collect(),
        }
    }

    fn emit_list_literal_construct(
        &mut self,
        ty: Idx,
        llvm_ty: LLVMTypeId,
        arg_vals: &[ValueId],
    ) -> ValueId {
        let count = arg_vals.len();
        let type_info = self.type_info.get(ty);
        let super::super::type_info::TypeInfo::List { element } = &type_info else {
            unreachable!("ListLiteral TypeInfo mismatch: {type_info:?}")
        };
        let elem_idx = *element;
        let collection_idx = self.pool.resolve_fully(ty);
        let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_idx);
        let elem_size = self.collection_elem_size(collection_idx, elem_idx);
        let cap = self.builder.const_i64(count as i64);
        let elem_size_value = self.builder.const_i64(elem_size as i64);
        let alloc = self.builder.runtime_fn("ori_list_alloc_data");
        let Some(data_ptr) = self
            .builder
            .call(alloc, &[cap, elem_size_value], "list.data")
        else {
            panic!("ori_list_alloc_data must return a data pointer");
        };

        for (index, &value) in arg_vals.iter().enumerate() {
            let index = self.builder.const_i64(index as i64);
            let elem_ptr = self
                .builder
                .gep(elem_llvm_ty, data_ptr, &[index], "list.elem_ptr");
            let stored = self.trunc_for_narrowed_collection_element(
                value,
                collection_idx,
                "list.elem.trunc",
            );
            self.builder.store(stored, elem_ptr);
        }

        let elem_dec = self.get_or_generate_elem_dec_fn(elem_idx);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder.call(store_dec, &[data_ptr, elem_dec], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder.call(store_count, &[data_ptr, cap], "");
        self.builder
            .build_struct(llvm_ty, &[cap, cap, data_ptr], "list")
    }

    fn emit_map_literal_construct(
        &mut self,
        ty: Idx,
        llvm_ty: LLVMTypeId,
        arg_vals: &[ValueId],
    ) -> ValueId {
        assert!(
            arg_vals.len().is_multiple_of(2),
            "MapLiteral requires alternating key/value arguments"
        );
        let count = arg_vals.len() / 2;
        let type_info = self.type_info.get(ty);
        let super::super::type_info::TypeInfo::Map { key, value } = &type_info else {
            unreachable!("MapLiteral TypeInfo mismatch: {type_info:?}")
        };
        let (key_idx, value_idx) = (*key, *value);
        let key_llvm_ty = self.resolve_type(key_idx);
        let value_llvm_ty = self.resolve_type(value_idx);
        let collection_idx = self.pool.resolve_fully(ty);
        let key_size = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, key_idx) as i64);
        let value_size = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, value_idx) as i64);
        let count_value = self.builder.const_i64(count as i64);

        let i64_ty = self.builder.i64_type();
        let out_cap = self.builder.alloca(i64_ty, "map.out_cap");
        let alloc = self.builder.runtime_fn("ori_map_literal_alloc");
        let Some(data_ptr) = self.builder.call(
            alloc,
            &[count_value, key_size, value_size, out_cap],
            "map.data",
        ) else {
            panic!("ori_map_literal_alloc must return a data pointer");
        };
        let cap = self.builder.load(i64_ty, out_cap, "map.cap");
        let Some(eq) = self.get_or_create_eq_thunk(key_idx) else {
            panic!("type-checked map key must have an equality implementation");
        };
        let Some(hash) = self.get_or_create_hash_thunk(key_idx) else {
            panic!("type-checked map key must have a hash implementation");
        };
        let key_dec = self.get_or_generate_elem_dec_fn(key_idx);
        let value_dec = self.get_or_generate_elem_dec_fn(value_idx);
        let key_tmp = self.builder.alloca(key_llvm_ty, "map.key_tmp");
        let value_tmp = self.builder.alloca(value_llvm_ty, "map.val_tmp");
        let put = self.builder.runtime_fn("ori_map_literal_put");
        let mut actual_count = self.builder.const_i64(0);

        for pair in arg_vals.chunks_exact(2) {
            self.builder.store(pair[0], key_tmp);
            self.builder.store(pair[1], value_tmp);
            let Some(inserted) = self.emit_rt_call(
                put,
                &[
                    data_ptr, cap, key_tmp, value_tmp, key_size, value_size, eq, hash, key_dec,
                    value_dec,
                ],
                "map.put",
            ) else {
                panic!("ori_map_literal_put must return an insertion flag");
            };
            actual_count = self.builder.add(actual_count, inserted, "map.len");
        }
        self.builder
            .build_struct(llvm_ty, &[actual_count, cap, data_ptr], "map")
    }

    fn emit_set_literal_construct(
        &mut self,
        ty: Idx,
        llvm_ty: LLVMTypeId,
        arg_vals: &[ValueId],
    ) -> ValueId {
        let count = arg_vals.len();
        let type_info = self.type_info.get(ty);
        let super::super::type_info::TypeInfo::Set { element } = &type_info else {
            unreachable!("SetLiteral TypeInfo mismatch: {type_info:?}")
        };
        let elem_idx = *element;
        let elem_llvm_ty = self.resolve_type(elem_idx);
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_idx) as i64);
        let count_value = self.builder.const_i64(count as i64);
        let i64_ty = self.builder.i64_type();
        let out_cap = self.builder.alloca(i64_ty, "set.out_cap");
        let alloc = self.builder.runtime_fn("ori_set_literal_alloc");
        let Some(data_ptr) =
            self.builder
                .call(alloc, &[count_value, elem_size, out_cap], "set.data")
        else {
            panic!("ori_set_literal_alloc must return a data pointer");
        };
        let cap = self.builder.load(i64_ty, out_cap, "set.cap");
        let Some(hash) = self.get_or_create_hash_thunk(elem_idx) else {
            panic!("type-checked set element must have a hash implementation");
        };
        let elem_tmp = self.builder.alloca(elem_llvm_ty, "set.elem_tmp");
        let put = self.builder.runtime_fn("ori_set_literal_put");
        for &value in arg_vals {
            self.builder.store(value, elem_tmp);
            self.emit_rt_call(put, &[data_ptr, cap, elem_tmp, elem_size, hash], "set.put");
        }

        let elem_dec = self.get_or_generate_elem_dec_fn(elem_idx);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder.call(store_dec, &[data_ptr, elem_dec], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder.call(store_count, &[data_ptr, count_value], "");
        self.builder
            .build_struct(llvm_ty, &[count_value, cap, data_ptr], "set")
    }

    /// Construct a niche-encoded enum variant.
    ///
    /// Niche layout has no tag field — payload fields start at struct index 0.
    /// For the niche variant (e.g., None): create a zeroinit struct; `SetTag`
    /// will write the niche value afterward.
    /// For the data variant (e.g., Some(val)): insert payload at index 0.
    fn emit_niche_variant_construct(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        arg_vals: &[ValueId],
        encoding: &super::tag_access::TagEncoding,
        variant: u32,
    ) -> ValueId {
        let mut result = self.builder.const_zero_ty(llvm_ty);
        if encoding.needs_tag_store(variant) {
            // Niche variant (no payload): insert niche_value directly
            // so that `SetTag` is not needed (avoids GEP-on-register issues).
            let Some(niche_idx) = encoding.niche_field_index() else {
                panic!("niche variant requiring a tag store must name its niche field");
            };
            let niche_value = encoding.variant_to_tag_value(variant);
            let niche_const =
                self.builder
                    .const_int_for_struct_field(llvm_ty, niche_idx, niche_value);
            return self
                .builder
                .insert_value(result, niche_const, niche_idx, "niche.tag");
        }
        // Data variant: insert payload fields starting at index 0.
        for (i, &val) in arg_vals.iter().enumerate() {
            let Ok(idx) = u32::try_from(i) else {
                panic!("niche payload field index must fit u32");
            };
            result = self.builder.insert_value(result, val, idx, "niche.val");
        }
        result
    }

    /// Declaration-order field/element types for a struct or tuple `Idx`.
    ///
    /// Returns an empty vec for non-aggregate tags.
    pub(super) fn struct_field_types(&self, resolved_ty: Idx) -> Vec<Idx> {
        match self.pool.tag(resolved_ty) {
            Tag::Struct => self
                .pool
                .struct_fields(resolved_ty)
                .into_iter()
                .map(|(_, ft)| ft)
                .collect(),
            Tag::Tuple => self.pool.tuple_elems(resolved_ty),
            _ => Vec::new(),
        }
    }

    /// Box an inline value for a boxed recursive field/element slot.
    ///
    /// Allocates an RC box sized to `field_type`, stores the inline `val` into
    /// it, and returns the box pointer. When the source variable is rooted at a
    /// borrowed parameter (the caller retains a live reference), the inline
    /// value's heap sub-pointers gain a second owner (the box) and are
    /// incremented; for consumed (moved) values this is a move with no inc.
    /// Mirrors the enum boxing in `emit_variant_via_alloca`.
    pub(super) fn box_recursive_field(
        &mut self,
        val: ValueId,
        field_type: Idx,
        arc_var: Option<ArcVarId>,
    ) -> ValueId {
        let size = self.element_store_size(field_type);
        let rc_ptr = self.rc_alloc(size, 8);
        self.builder.store(val, rc_ptr);
        if let Some(var) = arc_var {
            if self.is_var_borrowed_rooted(var) {
                self.inc_value_rc(val, field_type, 1);
            }
        }
        rc_ptr
    }
}
