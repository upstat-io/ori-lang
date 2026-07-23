//! Niche and tagged-pointer enum drop emission.

use ori_types::Idx;

use crate::codegen::value_id::{FunctionId, ValueId};

use super::drop_enum::{resolve_type_through_aliases, NicheEnumPayloadOps};
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Drop the data variant of a two-variant niche enum.
    pub(super) fn emit_drop_enum_niche(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        variants: &[Vec<(u32, Idx)>],
        encoding: &super::tag_access::TagEncoding,
    ) {
        let enum_llvm_ty = self.resolve_type(ty);
        let Some((niche_idx, niche_value, niche_variant_idx)) = encoding.niche_fields() else {
            self.builder
                .record_codegen_error_with_msg("niche drop requires a niche tag encoding");
            return;
        };
        let niche_variant_idx = niche_variant_idx as usize;

        let Some(field_ty) = self.builder.struct_field_type(enum_llvm_ty, niche_idx) else {
            self.builder
                .record_codegen_error_with_msg("niche drop layout is missing its sentinel field");
            return;
        };
        let field_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, data_ptr, niche_idx, "niche.ptr");
        let field_val = self.builder.load(field_ty, field_ptr, "niche.val");

        let is_niche = self.niche_is_sentinel(field_val, niche_value, "is.niche");

        let drop_data = self.builder.append_block(func_id, "drop.data");
        let drop_done = self.builder.append_block(func_id, "drop.done");

        self.builder.cond_br(is_niche, drop_done, drop_data);

        // Why: the field walker releases remaining siblings when a user drop unwinds.
        self.builder.position_at_end(drop_data);
        let data_variant_idx = usize::from(niche_variant_idx == 0);
        if let Some(data_fields) = variants.get(data_variant_idx) {
            let (resolved_ty, _) = resolve_type_through_aliases(ty, self.pool);
            let walk = super::emitter_utils::field_rc_walk_order(
                data_fields,
                super::emitter_utils::FieldRcWalkOrder::Teardown,
            );
            let ops = NicheEnumPayloadOps {
                value: data_ptr,
                enum_llvm_ty,
                owner_ty: resolved_ty,
                value_traversal: false,
            };
            self.dec_fields_may_unwind(&ops, &walk, 0);
        }
        self.builder.br(drop_done);

        self.builder.position_at_end(drop_done);
    }

    /// Drop the active pointer payload of a tagged-pointer enum.
    ///
    /// Each variant has zero fields for a unit or one field for its pointer.
    pub(super) fn emit_drop_enum_tagged_ptr(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        variants: &[Vec<(u32, Idx)>],
    ) {
        let i64_ty = self.builder.i64_type();
        let encoded = self.builder.load(i64_ty, data_ptr, "tagged.encoded");
        let tag_val = self.tagged_ptr_decode_tag(encoded, "tagged.tag");

        let drop_done = self.builder.append_block(func_id, "drop.done");

        let mut cases: Vec<(ValueId, crate::codegen::value_id::BlockId, Idx)> = Vec::new();
        for (i, variant_fields) in variants.iter().enumerate() {
            if variant_fields.is_empty() {
                continue;
            }
            debug_assert!(
                variant_fields.len() == 1,
                "tagged-pointer variant must have at most one RC field"
            );
            let (_, field_type) = variant_fields[0];
            let block = self
                .builder
                .append_block(func_id, &format!("tagged.v{i}.drop"));
            // Why: three-bit tags bound every variant index below eight.
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            cases.push((tag_const, block, field_type));
        }

        if cases.is_empty() {
            self.builder.br(drop_done);
        } else {
            let switch_cases: Vec<(ValueId, crate::codegen::value_id::BlockId)> =
                cases.iter().map(|(t, b, _)| (*t, *b)).collect();
            self.builder.switch(tag_val, drop_done, &switch_cases);

            for &(_, block, field_type) in &cases {
                self.builder.position_at_end(block);
                let ptr = self.tagged_ptr_decode_ptr(encoded, "tagged.ptr");
                self.emit_drop_rc_dec(ptr, field_type);
                self.builder.br(drop_done);
            }
        }

        self.builder.position_at_end(drop_done);
    }
}
