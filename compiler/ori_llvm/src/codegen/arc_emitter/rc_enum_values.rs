//! Inline enum retain/release dispatch for ARC-managed payloads.

use ori_types::{Idx, Tag};

use crate::codegen::value_id::{BlockId, LLVMTypeId, ValueId};

use super::context::is_boxed_enum_field;
use super::drop_enum::{compute_variant_field_offsets, variant_field_offset};
use super::rc_enum_payload::TaggedEnumPayloadOps;
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Whether a variant-payload position holding `field_ty` inside the tagged
    /// union `owner_ty` needs an RC dec/inc: either the payload's inline type
    /// is RC-bearing, OR the position is a boxed recursive back-edge (a heap
    /// RC box that must be dropped regardless of the inline classification).
    pub(super) fn payload_needs_rc(&self, owner_ty: Idx, field_ty: Idx) -> bool {
        self.classifier.has_managed_ownership_obligation(field_ty)
            || is_boxed_enum_field(self.pool, owner_ty, field_ty)
    }

    /// Collect per-variant RC field info for an inline enum.
    ///
    /// Returns a vec-of-vecs: `[variant_idx][field_idx] = (field_position, field_type)`.
    /// Empty inner vec means the variant has no RC fields.
    fn collect_variant_rc_fields(&self, resolved_ty: Idx, pool_tag: Tag) -> Vec<Vec<(u32, Idx)>> {
        match pool_tag {
            Tag::Result => {
                let ok_ty = self.pool.result_ok(resolved_ty);
                let err_ty = self.pool.result_err(resolved_ty);
                // INVARIANT: boxed recursive back-edges own RC boxes even when their inline type is unmanaged.
                let ok_fields = if self.payload_needs_rc(resolved_ty, ok_ty) {
                    vec![(0_u32, ok_ty)]
                } else {
                    vec![]
                };
                let err_fields = if self.payload_needs_rc(resolved_ty, err_ty) {
                    vec![(0_u32, err_ty)]
                } else {
                    vec![]
                };
                vec![ok_fields, err_fields]
            }
            Tag::Option => {
                let inner = self.pool.option_inner(resolved_ty);
                let some_fields = if self.payload_needs_rc(resolved_ty, inner) {
                    vec![(0_u32, inner)]
                } else {
                    vec![]
                };
                vec![some_fields, vec![]]
            }
            Tag::Enum => {
                let variants = self.pool.enum_variants(resolved_ty);
                variants
                    .iter()
                    .map(|(_, field_tys)| {
                        field_tys
                            .iter()
                            .enumerate()
                            .filter(|(_, ty)| self.payload_needs_rc(resolved_ty, **ty))
                            .map(|(i, ty)| {
                                #[expect(
                                    clippy::cast_possible_truncation,
                                    reason = "variant field index fits u32"
                                )]
                                (i as u32, *ty)
                            })
                            .collect()
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    /// Retain the managed payload fields of an inline enum value.
    pub(super) fn emit_inline_enum_inc(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        count: u32,
    ) {
        self.emit_inline_enum_rc_core(
            val,
            resolved_ty,
            pool_tag,
            super::emitter_utils::RcOperation::Retain { count },
        );
    }

    /// Release the managed payload fields of an inline enum value.
    pub(super) fn emit_inline_enum_dec(&mut self, val: ValueId, resolved_ty: Idx, pool_tag: Tag) {
        self.emit_inline_enum_rc_core(
            val,
            resolved_ty,
            pool_tag,
            super::emitter_utils::RcOperation::Release,
        );
    }

    /// Apply one retain or release operation to an inline enum's active payload.
    fn emit_inline_enum_rc_core(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        operation: super::emitter_utils::RcOperation,
    ) {
        let variant_rc_fields = self.collect_variant_rc_fields(resolved_ty, pool_tag);

        if variant_rc_fields.iter().all(Vec::is_empty) {
            return;
        }

        if self.get_tagged_ptr_encoding(resolved_ty).is_some() {
            self.emit_tagged_ptr_enum_rc(val, &variant_rc_fields, operation);
            return;
        }

        if self.is_tagless_enum(resolved_ty) {
            self.emit_inline_tagless_rc(val, resolved_ty, operation);
            return;
        }

        if let Some(encoding) = self.get_niche_encoding(resolved_ty) {
            self.emit_niche_enum_rc(val, resolved_ty, &variant_rc_fields, &encoding, operation);
            return;
        }

        let dir = operation.prefix();

        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let alloca = self.builder.alloca(enum_llvm_ty, &format!("{dir}.enum"));
        self.builder.store(val, alloca);

        let Some(tag_ty) = self.builder.struct_field_type(enum_llvm_ty, 0) else {
            self.builder
                .record_codegen_error_with_msg("enum RC layout is missing its tag field");
            return;
        };
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, alloca, 0, &format!("{dir}.tag.ptr"));
        let tag_val = self.builder.load(tag_ty, tag_ptr, &format!("{dir}.tag"));

        let done_block = self
            .builder
            .append_block(self.current_function, &format!("{dir}.done"));

        let all_variant_fields: Vec<Vec<Idx>> = if pool_tag == Tag::Enum {
            self.pool
                .enum_variants(resolved_ty)
                .into_iter()
                .map(|(_, fields)| fields)
                .collect()
        } else {
            Vec::new()
        };

        let mut cases = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            let block = self
                .builder
                .append_block(self.current_function, &format!("{dir}.v{i}"));
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            cases.push((tag_const, block, fields.as_slice(), i));
        }

        let switch_cases: Vec<_> = cases
            .iter()
            .map(|(tag, block, _, _)| (*tag, *block))
            .collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        let is_option_result = matches!(pool_tag, Tag::Result | Tag::Option);
        for &(_, block, fields, variant_idx) in &cases {
            self.builder.position_at_end(block);

            let offsets = if pool_tag == Tag::Enum {
                // Why: an empty fallback would silently skip payload RC for an invalid upstream variant.
                let variant_fields = all_variant_fields.get(variant_idx).unwrap_or_else(|| {
                    unreachable!("RC walk variant {variant_idx} out of bounds for enum type")
                });
                compute_variant_field_offsets(variant_fields, resolved_ty, self)
            } else {
                Vec::new()
            };

            // INVARIANT: release order is reverse declaration order; retain order is unobservable.
            let ordered: Vec<(u32, Idx)> =
                super::emitter_utils::field_rc_walk_order(fields, operation.field_walk_order());

            if let Some(count) = operation.retain_count() {
                self.inc_enum_payload_fields(
                    &ordered,
                    enum_llvm_ty,
                    alloca,
                    resolved_ty,
                    is_option_result,
                    &offsets,
                    count,
                );
            } else {
                // Why: the field walker releases remaining siblings when a user drop unwinds.
                let ops = TaggedEnumPayloadOps {
                    alloca,
                    enum_llvm_ty,
                    owner_ty: resolved_ty,
                    is_option_result,
                    offsets,
                };
                self.dec_fields_may_unwind(&ops, &ordered, 0);
            }

            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }

    /// Inc the RC children of a tagged-enum variant's payload fields (forward
    /// order; inc has no user `@drop` and no unwind, so order is unobservable).
    #[expect(
        clippy::too_many_arguments,
        reason = "matches TaggedEnumPayloadOps addressing fields plus the retain count"
    )]
    fn inc_enum_payload_fields(
        &mut self,
        ordered: &[(u32, Idx)],
        enum_llvm_ty: LLVMTypeId,
        alloca: ValueId,
        owner_ty: Idx,
        is_option_result: bool,
        offsets: &[u64],
        count: u32,
    ) {
        for &(field_index, field_type) in ordered {
            let boxed = is_boxed_enum_field(self.pool, owner_ty, field_type);
            let field_ptr = if is_option_result {
                self.builder
                    .struct_gep(enum_llvm_ty, alloca, 1 + field_index, "inc.payload.ptr")
            } else {
                let payload_ptr = self
                    .builder
                    .struct_gep(enum_llvm_ty, alloca, 1, "inc.payload");
                let i8_ty = self.builder.i8_type();
                let byte_off = variant_field_offset(offsets, field_index as usize);
                let off = self.builder.const_i64(byte_off as i64);
                self.builder
                    .gep(i8_ty, payload_ptr, &[off], "inc.field.ptr")
            };
            if boxed {
                let ptr_ty = self.builder.ptr_type();
                let rc_ptr = self.builder.load(ptr_ty, field_ptr, "inc.payload.rc");
                self.call_rc_inc_all(&[rc_ptr], count);
            } else {
                let field_llvm_ty = self.resolve_type(field_type);
                let field_val = self.builder.load(field_llvm_ty, field_ptr, "inc.payload");
                self.inc_value_rc(field_val, field_type, count);
            }
        }
    }

    /// Apply retain or release to the data variant of a niche-encoded enum.
    fn emit_niche_enum_rc(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        variant_rc_fields: &[Vec<(u32, Idx)>],
        encoding: &super::tag_access::TagEncoding,
        operation: super::emitter_utils::RcOperation,
    ) {
        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let prefix = operation.prefix();
        let alloca = self
            .builder
            .alloca(enum_llvm_ty, &format!("{prefix}.niche"));
        self.builder.store(val, alloca);

        let Some((niche_idx, niche_value, niche_variant_idx)) = encoding.niche_fields() else {
            self.builder
                .record_codegen_error_with_msg("niche RC emission requires a niche tag encoding");
            return;
        };
        let niche_variant_idx = niche_variant_idx as usize;

        let Some(field_ty) = self.builder.struct_field_type(enum_llvm_ty, niche_idx) else {
            self.builder
                .record_codegen_error_with_msg("niche RC layout is missing its sentinel field");
            return;
        };
        let field_ptr = self.builder.struct_gep(
            enum_llvm_ty,
            alloca,
            niche_idx,
            &format!("{prefix}.niche.ptr"),
        );
        let field_val = self
            .builder
            .load(field_ty, field_ptr, &format!("{prefix}.niche.val"));

        let is_niche =
            self.niche_is_sentinel(field_val, niche_value, &format!("{prefix}.is_niche"));

        let data_block = self
            .builder
            .append_block(self.current_function, &format!("{prefix}.data"));
        let done_block = self
            .builder
            .append_block(self.current_function, &format!("{prefix}.done"));
        self.builder.cond_br(is_niche, done_block, data_block);

        self.builder.position_at_end(data_block);
        let data_variant_idx = usize::from(niche_variant_idx == 0);
        if let Some(data_fields) = variant_rc_fields.get(data_variant_idx) {
            if let Some(count) = operation.retain_count() {
                // INVARIANT: niche payload indices exclude a tag slot.
                for &(field_index, field_type) in data_fields {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let gep = self.builder.struct_gep(
                        enum_llvm_ty,
                        alloca,
                        field_index,
                        &format!("{prefix}.f{field_index}.ptr"),
                    );
                    let fval =
                        self.builder
                            .load(field_llvm_ty, gep, &format!("{prefix}.f{field_index}"));
                    self.inc_value_rc(fval, field_type, count);
                }
            } else {
                // Why: the field walker releases remaining siblings when a user drop unwinds.
                let walk = super::emitter_utils::field_rc_walk_order(
                    data_fields,
                    super::emitter_utils::FieldRcWalkOrder::Teardown,
                );
                let ops = super::drop_enum::NicheEnumPayloadOps {
                    value: alloca,
                    enum_llvm_ty,
                    owner_ty: resolved_ty,
                    value_traversal: true,
                };
                self.dec_fields_may_unwind(&ops, &walk, 0);
            }
        }
        self.builder.br(done_block);

        self.builder.position_at_end(done_block);
    }

    /// Apply retain or release to a tagged-pointer enum's active pointer payload.
    ///
    /// Each variant has zero fields for a unit or one field for its pointer.
    fn emit_tagged_ptr_enum_rc(
        &mut self,
        val: ValueId,
        variant_rc_fields: &[Vec<(u32, Idx)>],
        operation: super::emitter_utils::RcOperation,
    ) {
        let dir = operation.prefix();

        let tag_val = self.tagged_ptr_decode_tag(val, &format!("{dir}.tag"));

        let done_block = self
            .builder
            .append_block(self.current_function, &format!("{dir}.done"));

        let mut cases: Vec<(ValueId, BlockId, Idx)> = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            debug_assert!(
                fields.len() == 1,
                "tagged-pointer variant must have at most one RC field"
            );
            let (_, field_type) = fields[0];
            let block = self
                .builder
                .append_block(self.current_function, &format!("{dir}.tp.v{i}"));
            // Why: three-bit tags bound every variant index below eight.
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            cases.push((tag_const, block, field_type));
        }

        if cases.is_empty() {
            self.builder.br(done_block);
            self.builder.position_at_end(done_block);
            return;
        }

        let switch_cases: Vec<(ValueId, BlockId)> =
            cases.iter().map(|(t, b, _)| (*t, *b)).collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        for &(_, block, field_type) in &cases {
            self.builder.position_at_end(block);
            let ptr_val = self.tagged_ptr_decode_ptr(val, &format!("{dir}.tp.ptr"));
            if let Some(count) = operation.retain_count() {
                self.inc_value_rc(ptr_val, field_type, count);
            } else {
                self.dec_value_rc(ptr_val, field_type);
            }
            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }
}
