//! Enum Hashable derive codegen: `HashCombine` strategy.
//!
//! Zero-seeded `hash_combine` of the declaration ordinal and payload fields.

use ori_ir::FieldOp;
use ori_types::VariantDef;

use super::super::super::function_compiler::FunctionCompiler;
use super::super::super::type_info::TypeLayoutResolver;
use super::super::super::value_id::{BlockId, ValueId};
use super::super::field_ops::emit_field_operation;
use super::super::field_ops::emit_hash_combine;
use super::super::{emit_derive_return, DeriveSetup};

use super::variant_non_void_field_types;

/// Enum Hashable: combine the ordinal, then the selected payload fields.
///
/// Hashes the tag first, then switches on tag to hash variant-specific
/// payload fields.
pub(super) fn emit_enum_hash_combine<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("HashCombine has self");
    let func_id = setup.func_id;

    let tag = fc
        .builder_mut()
        .extract_value(self_val, 0, "hash.tag")
        .expect("tag extraction for derived Hashable");
    // The physical tag is the zero-based declaration ordinal. It may be
    // narrowed, so extend it to the semantic `int` width before combining.
    let i64_ty = fc.builder_mut().i64_type();
    let tag_i64 = fc.builder_mut().zext(tag, i64_ty, "hash.tag.ext");
    let zero = fc.builder_mut().const_i64(0);
    let hash = emit_hash_combine(fc, zero, tag_i64, "hash.tag");

    // check for non-void payload fields, not just non-unit variants.
    let has_payload = variants
        .iter()
        .any(|v| !variant_non_void_field_types(&v.fields, fc.pool()).is_empty());

    if has_payload {
        emit_enum_payload_hash(fc, setup, variants, field_op, hash);
    } else {
        // All unit: tag hash is sufficient
        emit_derive_return(fc, func_id, &setup.abi, Some(hash));
    }
}

/// Emit per-variant payload hashing via switch on tag.
fn emit_enum_payload_hash<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
    tag_hash: ValueId,
) {
    let self_val = setup.self_val.expect("Hashable has self");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Hashable needs str_ty_id");

    let merge_bb = fc.builder_mut().append_block(func_id, "hash.merge");

    // Alloca for GEP
    let enum_llvm_ty = fc.resolve_type(setup.type_idx);
    let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
    let self_alloca = fc.entry_alloca(enum_ty_id, "hash.self");
    fc.builder_mut().store(self_val, self_alloca);

    let self_payload = fc
        .builder_mut()
        .struct_gep(enum_ty_id, self_alloca, 1, "hash.self.payload");

    // Extract tag for switch
    let tag = fc
        .builder_mut()
        .extract_value(self_val, 0, "hash.switch.tag")
        .expect("tag extraction for hash switch");

    // Default block for switch: unreachable (all tags are covered by cases).
    // Cannot use merge_bb as default — its PHI has no incoming from this edge.
    let default_bb = fc.builder_mut().append_block(func_id, "hash.default");

    // Build switch cases — use const_int_matching for narrowed tag.
    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("hash.v.{variant_name}"));
        let tag_val = fc.builder_mut().const_int_matching(tag, tag_idx as u64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    fc.builder_mut().switch(tag, default_bb, &cases);
    fc.builder_mut().position_at_end(default_bb);
    fc.builder_mut().unreachable();

    let i64_ty = fc.builder_mut().i64_type();

    // Collect (variant_bb_end, hash_result) for phi node
    let mut phi_incoming: Vec<(ValueId, BlockId)> = Vec::new();

    for (tag_idx, variant) in variants.iter().enumerate() {
        fc.builder_mut().position_at_end(variant_bbs[tag_idx]);

        // filter zero-sized fields to match LLVM layout.
        let field_types = variant_non_void_field_types(&variant.fields, fc.pool());
        if field_types.is_empty() {
            // Unit or all-void: just use tag hash
            phi_incoming.push((tag_hash, variant_bbs[tag_idx]));
            fc.builder_mut().br(merge_bb);
            continue;
        }

        let mut hash = tag_hash;
        let mut i64_offset: u64 = 0;

        for (fi, &field_type) in field_types.iter().enumerate() {
            let (field_val, field_llvm_ty) = super::load_payload_slot_field(
                fc,
                i64_ty,
                self_payload,
                i64_offset,
                field_type,
                &format!("hash.v{tag_idx}.f{fi}"),
                &format!("hash.v{tag_idx}.f{fi}.val"),
            );

            let field_as_i64 = emit_field_operation(
                fc,
                field_op,
                field_val,
                None,
                field_type,
                &format!("hash.v{tag_idx}.f{fi}"),
                str_ty_id,
            );

            hash = emit_hash_combine(fc, hash, field_as_i64, &format!("hash.v{tag_idx}.f{fi}"));

            let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
            i64_offset += field_bytes.div_ceil(8).max(1);
        }

        // Record the current block (may differ from variant_bbs[tag_idx] due to
        // field operation blocks emitted by emit_field_operation)
        let current_bb = fc
            .builder_mut()
            .current_block()
            .expect("current block in hash");
        phi_incoming.push((hash, current_bb));
        fc.builder_mut().br(merge_bb);
    }

    // Merge: phi from all variant arms
    fc.builder_mut().position_at_end(merge_bb);
    let phi_result = fc.builder_mut().phi(i64_ty, "hash.result");
    fc.builder_mut().add_phi_incoming(phi_result, &phi_incoming);
    emit_derive_return(fc, func_id, &setup.abi, Some(phi_result));
}
