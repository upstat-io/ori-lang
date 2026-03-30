//! Enum Comparable derive codegen: `Lexicographic` strategy.
//!
//! Compares tags first (as unsigned for declaration order), then per-variant
//! lexicographic payload comparison when tags match.

use ori_ir::FieldOp;
use ori_types::VariantDef;

use super::super::super::function_compiler::FunctionCompiler;
use super::super::super::type_info::TypeLayoutResolver;
use super::super::super::value_id::ValueId;
use super::super::field_ops::emit_field_operation;
use super::super::{emit_derive_return, DeriveSetup};

use super::variant_field_types;

/// Enum Comparable: compare tags first, then per-variant lexicographic ordering.
///
/// Returns `Ordering` (Less=0, Equal=1, Greater=2) based on tag values,
/// then per-variant field ordering if tags match.
pub(super) fn emit_enum_lexicographic<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("Lexicographic has self");
    let other_val = setup.other_val.expect("Lexicographic has other");
    let func_id = setup.func_id;

    let tag_self = fc.builder_mut().extract_value(self_val, 0, "cmp.tag.self");
    let tag_other = fc
        .builder_mut()
        .extract_value(other_val, 0, "cmp.tag.other");

    let (Some(ts), Some(to)) = (tag_self, tag_other) else {
        tracing::warn!("extract_value failed for enum tag in derive Comparable");
        let equal = fc.builder_mut().const_i8(1);
        emit_derive_return(fc, func_id, &setup.abi, Some(equal));
        return;
    };

    // Compare tags as unsigned (variant declaration order)
    let tag_ord = fc
        .builder_mut()
        .emit_icmp_ordering(ts, to, "cmp.tags", false);

    let has_payload = variants.iter().any(|v| !v.fields.is_unit());

    if has_payload {
        // If tags differ, return tag ordering. If same, compare payloads.
        let one = fc.builder_mut().const_i8(1);
        let tags_equal = fc.builder_mut().icmp_eq(tag_ord, one, "cmp.tags.is_eq");

        let equal_bb = fc.builder_mut().append_block(func_id, "cmp.equal");
        let ret_tag_bb = fc.builder_mut().append_block(func_id, "cmp.ret.tag");
        fc.builder_mut().cond_br(tags_equal, equal_bb, ret_tag_bb);

        // Return tag ordering when tags differ
        fc.builder_mut().position_at_end(ret_tag_bb);
        emit_derive_return(fc, func_id, &setup.abi, Some(tag_ord));

        // Tags match — compare payloads
        fc.builder_mut().position_at_end(equal_bb);
        emit_enum_payload_cmp(fc, setup, variants, field_op, ts);
    } else {
        // All unit: tag ordering is the full ordering
        emit_derive_return(fc, func_id, &setup.abi, Some(tag_ord));
    }
}

/// Emit per-variant lexicographic comparison via switch on tag.
#[expect(
    clippy::too_many_lines,
    reason = "enum payload comparison emits per-variant switch + field ops"
)]
fn emit_enum_payload_cmp<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
    tag_self: ValueId,
) {
    let self_val = setup.self_val.expect("Comparable has self");
    let other_val = setup.other_val.expect("Comparable has other");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Comparable needs str_ty_id");

    let equal_result_bb = fc.builder_mut().append_block(func_id, "cmp.result.equal");

    // Alloca for GEP
    let enum_llvm_ty = fc.resolve_type(setup.type_idx);
    let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
    let self_alloca = fc.entry_alloca(enum_ty_id, "cmp.self");
    let other_alloca = fc.entry_alloca(enum_ty_id, "cmp.other");
    fc.builder_mut().store(self_val, self_alloca);
    fc.builder_mut().store(other_val, other_alloca);

    let self_payload = fc
        .builder_mut()
        .struct_gep(enum_ty_id, self_alloca, 1, "cmp.self.payload");
    let other_payload =
        fc.builder_mut()
            .struct_gep(enum_ty_id, other_alloca, 1, "cmp.other.payload");

    // Build switch cases — use const_int_matching for narrowed tag (§07.1).
    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("cmp.v.{variant_name}"));
        let tag_val = fc
            .builder_mut()
            .const_int_matching(tag_self, tag_idx as u64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    fc.builder_mut().switch(tag_self, equal_result_bb, &cases);

    let i64_ty = fc.builder_mut().i64_type();

    for (tag_idx, variant) in variants.iter().enumerate() {
        fc.builder_mut().position_at_end(variant_bbs[tag_idx]);

        let field_types = variant_field_types(&variant.fields);
        if field_types.is_empty() {
            fc.builder_mut().br(equal_result_bb);
            continue;
        }

        let mut i64_offset: u64 = 0;
        for (fi, &field_type) in field_types.iter().enumerate() {
            let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
            let self_slot = fc.builder_mut().gep(
                i64_ty,
                self_payload,
                &[slot_idx],
                &format!("cmp.v{tag_idx}.self.f{fi}"),
            );
            let other_slot = fc.builder_mut().gep(
                i64_ty,
                other_payload,
                &[slot_idx],
                &format!("cmp.v{tag_idx}.other.f{fi}"),
            );

            let field_llvm_ty = fc.resolve_type(field_type);
            let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);

            let self_field = fc.builder_mut().load(
                field_ty_id,
                self_slot,
                &format!("cmp.v{tag_idx}.self.f{fi}.val"),
            );
            let other_field = fc.builder_mut().load(
                field_ty_id,
                other_slot,
                &format!("cmp.v{tag_idx}.other.f{fi}.val"),
            );

            let ord = emit_field_operation(
                fc,
                field_op,
                self_field,
                Some(other_field),
                field_type,
                &format!("cmp.v{tag_idx}.f{fi}"),
                str_ty_id,
            );

            let one = fc.builder_mut().const_i8(1);
            let is_equal =
                fc.builder_mut()
                    .icmp_eq(ord, one, &format!("cmp.v{tag_idx}.f{fi}.is_eq"));

            if fi + 1 < field_types.len() {
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.v{tag_idx}.ret.f{fi}"));
                let next_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.v{tag_idx}.f{}", fi + 1));
                fc.builder_mut().cond_br(is_equal, next_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));

                fc.builder_mut().position_at_end(next_bb);
            } else {
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.v{tag_idx}.ret.f{fi}"));
                fc.builder_mut().cond_br(is_equal, equal_result_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));
            }

            let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
            i64_offset += field_bytes.div_ceil(8).max(1);
        }
    }

    fc.builder_mut().position_at_end(equal_result_bb);
    let equal_val = fc.builder_mut().const_i8(1);
    emit_derive_return(fc, func_id, &setup.abi, Some(equal_val));
}
