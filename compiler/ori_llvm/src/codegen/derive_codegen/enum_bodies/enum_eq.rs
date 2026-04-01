//! Enum Eq derive codegen: `AllTrue` strategy.
//!
//! Compares tags first, then per-variant payload comparison for payload enums.

use ori_ir::FieldOp;
use ori_types::VariantDef;

use super::super::super::function_compiler::FunctionCompiler;
use super::super::super::type_info::TypeLayoutResolver;
use super::super::super::value_id::{BlockId, ValueId};
use super::super::field_ops::emit_field_operation;
use super::super::DeriveSetup;

use super::{variant_field_types, variant_non_void_field_types};

/// Enum Eq: compare tags first, then per-variant payload comparison.
///
/// For unit-only enums, just compares tags. For payload enums, switches on
/// the tag value and compares each variant's fields individually.
pub(super) fn emit_enum_all_true<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("AllTrue has self");
    let other_val = setup.other_val.expect("AllTrue has other");
    let func_id = setup.func_id;

    let true_bb = fc.builder_mut().append_block(func_id, "eq.true");
    let false_bb = fc.builder_mut().append_block(func_id, "eq.false");

    // Extract tags
    let tag_self = fc.builder_mut().extract_value(self_val, 0, "eq.tag.self");
    let tag_other = fc.builder_mut().extract_value(other_val, 0, "eq.tag.other");

    let (Some(ts), Some(to)) = (tag_self, tag_other) else {
        tracing::warn!("extract_value failed for enum tag in derive Eq");
        fc.builder_mut().br(false_bb);
        emit_enum_true_false_returns(fc, true_bb, false_bb);
        return;
    };

    let tags_eq = fc.builder_mut().icmp_eq(ts, to, "eq.tags");
    // BUG-04-008 / TPR-07-006: check for non-void payload fields, not just
    // non-unit variants. A variant like `A(u: void)` has VariantFields::Tuple
    // but zero payload slots in LLVM.
    let has_payload = variants
        .iter()
        .any(|v| !variant_non_void_field_types(&v.fields, fc.pool()).is_empty());

    if has_payload {
        // Tags must match first, then per-variant payload comparison
        let tags_match_bb = fc.builder_mut().append_block(func_id, "eq.tags.match");
        fc.builder_mut().cond_br(tags_eq, tags_match_bb, false_bb);
        fc.builder_mut().position_at_end(tags_match_bb);

        emit_enum_payload_eq(fc, setup, variants, field_op, ts, true_bb, false_bb);
    } else {
        // All unit: tags match is sufficient
        fc.builder_mut().cond_br(tags_eq, true_bb, false_bb);
    }

    emit_enum_true_false_returns(fc, true_bb, false_bb);
}

/// Emit per-variant payload comparison for Eq via switch on tag.
#[expect(
    clippy::too_many_lines,
    reason = "enum Eq emits per-variant switch with field comparisons"
)]
fn emit_enum_payload_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
    tag_self: ValueId,
    true_bb: BlockId,
    false_bb: BlockId,
) {
    let self_val = setup.self_val.expect("Eq has self");
    let other_val = setup.other_val.expect("Eq has other");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Eq needs str_ty_id");

    // Check if all variant fields are single-slot scalars. If so, we can
    // use extractvalue chains (no alloca+store+GEP needed).
    let all_single_slot = fc.builder_mut().is_struct_value(self_val)
        && fc.builder_mut().is_struct_value(other_val)
        && variants.iter().all(|v| {
            variant_field_types(&v.fields).iter().all(|&ft| {
                let llvm_ty = fc.resolve_type(ft);
                let ty_id = fc.builder_mut().register_type(llvm_ty);
                fc.builder_mut().is_single_slot_type(ty_id)
            })
        });

    // Build switch cases — one block per variant.
    // Use const_int_matching to match the narrowed tag type (§07.1).
    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("eq.v.{variant_name}"));
        let tag_val = fc
            .builder_mut()
            .const_int_matching(tag_self, tag_idx as u64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    if all_single_slot {
        // Fast path: extractvalue chains — no alloca+store+GEP.
        let self_payload = fc
            .builder_mut()
            .extract_value(self_val, 1, "eq.self.payload")
            .expect("self should be struct");
        let other_payload = fc
            .builder_mut()
            .extract_value(other_val, 1, "eq.other.payload")
            .expect("other should be struct");

        fc.builder_mut().switch(tag_self, false_bb, &cases);

        for (tag_idx, variant) in variants.iter().enumerate() {
            fc.builder_mut().position_at_end(variant_bbs[tag_idx]);
            // TPR-07-006: filter zero-sized fields to match LLVM layout.
            let field_types = variant_non_void_field_types(&variant.fields, fc.pool());
            if field_types.is_empty() {
                fc.builder_mut().br(true_bb);
                continue;
            }

            let mut i64_offset: u32 = 0;
            for (fi, &field_type) in field_types.iter().enumerate() {
                let self_raw = fc.builder_mut().extract_value_any(
                    self_payload,
                    i64_offset,
                    &format!("eq.v{tag_idx}.self.f{fi}"),
                );
                let other_raw = fc.builder_mut().extract_value_any(
                    other_payload,
                    i64_offset,
                    &format!("eq.v{tag_idx}.other.f{fi}"),
                );

                let field_llvm_ty = fc.resolve_type(field_type);
                let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);
                let self_field = fc.builder_mut().reinterpret_from_i64(
                    self_raw,
                    field_ty_id,
                    &format!("eq.v{tag_idx}.self.f{fi}.val"),
                );
                let other_field = fc.builder_mut().reinterpret_from_i64(
                    other_raw,
                    field_ty_id,
                    &format!("eq.v{tag_idx}.other.f{fi}.val"),
                );

                let cmp = emit_field_operation(
                    fc,
                    field_op,
                    self_field,
                    Some(other_field),
                    field_type,
                    &format!("eq.v{tag_idx}.f{fi}"),
                    str_ty_id,
                );

                if fi + 1 < field_types.len() {
                    let next_bb = fc
                        .builder_mut()
                        .append_block(func_id, &format!("eq.v{tag_idx}.f{}", fi + 1));
                    fc.builder_mut().cond_br(cmp, next_bb, false_bb);
                    fc.builder_mut().position_at_end(next_bb);
                } else {
                    fc.builder_mut().cond_br(cmp, true_bb, false_bb);
                }

                let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
                i64_offset += u32::try_from(field_bytes.div_ceil(8).max(1)).unwrap_or(1);
            }
        }
    } else {
        // Slow path: alloca+store+GEP for multi-word fields.
        let enum_llvm_ty = fc.resolve_type(setup.type_idx);
        let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
        let self_alloca = fc.entry_alloca(enum_ty_id, "eq.self");
        let other_alloca = fc.entry_alloca(enum_ty_id, "eq.other");
        fc.builder_mut().store(self_val, self_alloca);
        fc.builder_mut().store(other_val, other_alloca);

        let self_payload =
            fc.builder_mut()
                .struct_gep(enum_ty_id, self_alloca, 1, "eq.self.payload");
        let other_payload =
            fc.builder_mut()
                .struct_gep(enum_ty_id, other_alloca, 1, "eq.other.payload");

        fc.builder_mut().switch(tag_self, false_bb, &cases);

        let i64_ty = fc.builder_mut().i64_type();
        for (tag_idx, variant) in variants.iter().enumerate() {
            fc.builder_mut().position_at_end(variant_bbs[tag_idx]);
            // TPR-07-006: filter zero-sized fields to match LLVM layout.
            let field_types = variant_non_void_field_types(&variant.fields, fc.pool());
            if field_types.is_empty() {
                fc.builder_mut().br(true_bb);
                continue;
            }

            let mut i64_offset: u64 = 0;
            for (fi, &field_type) in field_types.iter().enumerate() {
                let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
                let self_slot = fc.builder_mut().gep(
                    i64_ty,
                    self_payload,
                    &[slot_idx],
                    &format!("eq.v{tag_idx}.self.f{fi}"),
                );
                let other_slot = fc.builder_mut().gep(
                    i64_ty,
                    other_payload,
                    &[slot_idx],
                    &format!("eq.v{tag_idx}.other.f{fi}"),
                );

                let field_llvm_ty = fc.resolve_type(field_type);
                let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);
                let self_field = fc.builder_mut().load(
                    field_ty_id,
                    self_slot,
                    &format!("eq.v{tag_idx}.self.f{fi}.val"),
                );
                let other_field = fc.builder_mut().load(
                    field_ty_id,
                    other_slot,
                    &format!("eq.v{tag_idx}.other.f{fi}.val"),
                );

                let cmp = emit_field_operation(
                    fc,
                    field_op,
                    self_field,
                    Some(other_field),
                    field_type,
                    &format!("eq.v{tag_idx}.f{fi}"),
                    str_ty_id,
                );

                if fi + 1 < field_types.len() {
                    let next_bb = fc
                        .builder_mut()
                        .append_block(func_id, &format!("eq.v{tag_idx}.f{}", fi + 1));
                    fc.builder_mut().cond_br(cmp, next_bb, false_bb);
                    fc.builder_mut().position_at_end(next_bb);
                } else {
                    fc.builder_mut().cond_br(cmp, true_bb, false_bb);
                }

                let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
                i64_offset += field_bytes.div_ceil(8).max(1);
            }
        }
    }
}

/// Emit the true/false return blocks for enum Eq.
fn emit_enum_true_false_returns<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    true_bb: BlockId,
    false_bb: BlockId,
) {
    fc.builder_mut().position_at_end(true_bb);
    let true_val = fc.builder_mut().const_bool(true);
    fc.builder_mut().ret(true_val);

    fc.builder_mut().position_at_end(false_bb);
    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut().ret(false_val);
}
