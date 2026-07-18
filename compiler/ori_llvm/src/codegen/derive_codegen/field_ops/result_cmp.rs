//! Inline Result comparison/hash for derived trait codegen.
//!
//! Generates branchless LLVM IR for structural equality, comparison, and
//! hashing of `Result` fields. Recurses to [`super::emit_field_operation`] for
//! payloads; folds the discriminant via [`super::wrapper_cmp::emit_hash_combine`].

use ori_ir::{FieldOp, RESULT_TAG_OK};
use ori_types::Idx;

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::ir_builder::IntegerSignedness;
use crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::emit_field_operation;
use super::wrapper_cmp::emit_hash_combine;

fn compare_result_boxed_payload<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: ValueId,
    ok_ty: Idx,
    err_ty: Idx,
    ok_boxed: bool,
    err_boxed: bool,
    is_ok: ValueId,
    result_ty: LLVMTypeId,
    fallback: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let func_id = fc
        .builder_mut()
        .current_function()
        .expect("emit within a function");
    // Why: a boxed arm's field-1 slot is a `ptr`, a scalar arm packs its value
    // into that slot — store to allocas + GEP field 1 so each arm loads through
    // the slot with its own LLVM type (the Result-clone alloca+GEP discipline).
    let result_ty_id = fc.builder_mut().register_value_type(lhs);
    let lhs_alloca =
        fc.builder_mut()
            .create_entry_alloca(func_id, &format!("{name}.lhs_tmp"), result_ty_id);
    let rhs_alloca =
        fc.builder_mut()
            .create_entry_alloca(func_id, &format!("{name}.rhs_tmp"), result_ty_id);
    fc.builder_mut().store(lhs, lhs_alloca);
    fc.builder_mut().store(rhs, rhs_alloca);
    let lhs_slot =
        fc.builder_mut()
            .struct_gep(result_ty_id, lhs_alloca, 1, &format!("{name}.lhs_slot"));
    let rhs_slot =
        fc.builder_mut()
            .struct_gep(result_ty_id, rhs_alloca, 1, &format!("{name}.rhs_slot"));

    let ok_bb = fc
        .builder_mut()
        .append_block(func_id, &format!("{name}.ok"));
    let err_bb = fc
        .builder_mut()
        .append_block(func_id, &format!("{name}.err"));
    let join_bb = fc
        .builder_mut()
        .append_block(func_id, &format!("{name}.join"));
    fc.builder_mut().cond_br(is_ok, ok_bb, err_bb);

    // Ok path: apply op to the Ok payload, loading through the box when boxed.
    fc.builder_mut().position_at_end(ok_bb);
    let ok_res = compare_result_arm_payload(
        fc,
        op,
        lhs_slot,
        rhs_slot,
        ok_ty,
        ok_boxed,
        &format!("{name}.ok"),
        str_ty_id,
    );
    let ok_end_bb = fc.builder_mut().current_block().unwrap_or(ok_bb);
    fc.builder_mut().br(join_bb);

    // Err path: apply op to the Err payload, loading through the box when boxed.
    fc.builder_mut().position_at_end(err_bb);
    let err_res = compare_result_arm_payload(
        fc,
        op,
        lhs_slot,
        rhs_slot,
        err_ty,
        err_boxed,
        &format!("{name}.err"),
        str_ty_id,
    );
    let err_end_bb = fc.builder_mut().current_block().unwrap_or(err_bb);
    fc.builder_mut().br(join_bb);

    fc.builder_mut().position_at_end(join_bb);
    fc.builder_mut()
        .phi_from_incoming(
            result_ty,
            &[(ok_res, ok_end_bb), (err_res, err_end_bb)],
            &format!("{name}.same"),
        )
        .unwrap_or(fallback)
}

/// Apply field-op `op` (Equals, Compare, or Hash) to one Result arm's payload
/// given pointers to each side's field-1 slot. When `boxed`, the slot holds an
/// RC box pointer: load the box pointer, then load the inner value through it
/// before applying `op`. Otherwise load the slot directly as the arm's own LLVM
/// type (reinterpreting the shared slot bytes). `Hash` is unary — only the lhs
/// slot is loaded; the rhs slot is left untouched.
fn compare_result_arm_payload<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs_slot: ValueId,
    rhs_slot: ValueId,
    payload_ty: Idx,
    boxed: bool,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let inner_llvm = fc.resolve_type(payload_ty);
    let inner_ty_id = fc.builder_mut().register_type(inner_llvm);
    let needs_rhs = !matches!(op, FieldOp::Hash);
    let (lhs_val, rhs_val) = if boxed {
        let ptr_ty = fc.builder_mut().ptr_type();
        let lhs_box = fc
            .builder_mut()
            .load(ptr_ty, lhs_slot, &format!("{name}.lhs_box"));
        let lhs_val = fc
            .builder_mut()
            .load(inner_ty_id, lhs_box, &format!("{name}.lhs.boxed"));
        let rhs_val = if needs_rhs {
            let rhs_box = fc
                .builder_mut()
                .load(ptr_ty, rhs_slot, &format!("{name}.rhs_box"));
            Some(
                fc.builder_mut()
                    .load(inner_ty_id, rhs_box, &format!("{name}.rhs.boxed")),
            )
        } else {
            None
        };
        (lhs_val, rhs_val)
    } else {
        let lhs_val = fc
            .builder_mut()
            .load(inner_ty_id, lhs_slot, &format!("{name}.lhs_val"));
        let rhs_val = needs_rhs.then(|| {
            fc.builder_mut()
                .load(inner_ty_id, rhs_slot, &format!("{name}.rhs_val"))
        });
        (lhs_val, rhs_val)
    };
    emit_field_operation(
        fc,
        op,
        lhs_val,
        rhs_val,
        payload_ty,
        &format!("{name}.payload"),
        str_ty_id,
    )
}

/// Result<T, E> equality: tags must match; same variant -> compare payloads.
///
/// When neither payload is a boxed recursive back-edge, uses branchless select:
/// both payload comparisons are evaluated and the relevant one selected (safe —
/// equality thunks have no side effects). When EITHER payload is boxed, the
/// payload slot holds an RC box pointer that is valid only on its own variant's
/// path, so the comparison is computed under a tag-guarded branch that loads
/// through the box only on the matching arm.
pub(super) fn emit_result_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    ok_ty: Idx,
    err_ty: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let fallback_i64 = fc.builder_mut().const_i64(0);
    let lhs_tag = fc
        .builder_mut()
        .extract_value(lhs, 0, &format!("{name}.lhs_tag"))
        .unwrap_or(fallback_i64);
    let rhs_tag = fc
        .builder_mut()
        .extract_value(rhs, 0, &format!("{name}.rhs_tag"))
        .unwrap_or(fallback_i64);
    let tags_eq = fc
        .builder_mut()
        .icmp_eq(lhs_tag, rhs_tag, &format!("{name}.tags_eq"));

    let ok_boxed = payload_type_is_rc_boxed(fc.type_info().pool(), ok_ty);
    let err_boxed = payload_type_is_rc_boxed(fc.type_info().pool(), err_ty);

    let ok_tag = fc.builder_mut().const_i64(RESULT_TAG_OK);
    let is_ok = fc
        .builder_mut()
        .icmp_eq(lhs_tag, ok_tag, &format!("{name}.is_ok"));

    let same_tag_eq = if ok_boxed || err_boxed {
        let bool_ty = fc.builder_mut().bool_type();
        let false_val = fc.builder_mut().const_bool(false);
        compare_result_boxed_payload(
            fc,
            FieldOp::Equals,
            lhs,
            rhs,
            ok_ty,
            err_ty,
            ok_boxed,
            err_boxed,
            is_ok,
            bool_ty,
            false_val,
            name,
            str_ty_id,
        )
    } else {
        let lhs_val = fc
            .builder_mut()
            .extract_value_any(lhs, 1, &format!("{name}.lhs_val"));
        let rhs_val = fc
            .builder_mut()
            .extract_value_any(rhs, 1, &format!("{name}.rhs_val"));
        let ok_eq = emit_field_operation(
            fc,
            FieldOp::Equals,
            lhs_val,
            Some(rhs_val),
            ok_ty,
            &format!("{name}.ok"),
            str_ty_id,
        );
        let err_eq = emit_field_operation(
            fc,
            FieldOp::Equals,
            lhs_val,
            Some(rhs_val),
            err_ty,
            &format!("{name}.err"),
            str_ty_id,
        );
        fc.builder_mut()
            .select(is_ok, ok_eq, err_eq, &format!("{name}.same_eq"))
    };

    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut()
        .select(tags_eq, same_tag_eq, false_val, &format!("{name}.eq"))
}

/// Result<T, E> compare: tags differ → tag ordering (Ok < Err per spec, natural
/// since `RESULT_TAG_OK=0` < `RESULT_TAG_ERR=1`); same tag → compare the payload.
/// Boxed payloads route through the shared `compare_result_boxed_payload`.
pub(super) fn emit_result_compare<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    ok_ty: Idx,
    err_ty: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let fallback_i64 = fc.builder_mut().const_i64(0);
    let lhs_tag = fc
        .builder_mut()
        .extract_value(lhs, 0, &format!("{name}.lhs_tag"))
        .unwrap_or(fallback_i64);
    let rhs_tag = fc
        .builder_mut()
        .extract_value(rhs, 0, &format!("{name}.rhs_tag"))
        .unwrap_or(fallback_i64);
    let tags_eq = fc
        .builder_mut()
        .icmp_eq(lhs_tag, rhs_tag, &format!("{name}.tags_eq"));

    let ok_boxed = payload_type_is_rc_boxed(fc.type_info().pool(), ok_ty);
    let err_boxed = payload_type_is_rc_boxed(fc.type_info().pool(), err_ty);

    let ok_tag = fc.builder_mut().const_i64(RESULT_TAG_OK);
    let is_ok = fc
        .builder_mut()
        .icmp_eq(lhs_tag, ok_tag, &format!("{name}.is_ok"));

    let same_tag_cmp = if ok_boxed || err_boxed {
        let i8_ty = fc.builder_mut().i8_type();
        let equal_ord = fc.builder_mut().const_i8(1);
        compare_result_boxed_payload(
            fc,
            FieldOp::Compare,
            lhs,
            rhs,
            ok_ty,
            err_ty,
            ok_boxed,
            err_boxed,
            is_ok,
            i8_ty,
            equal_ord,
            name,
            str_ty_id,
        )
    } else {
        let lhs_val = fc
            .builder_mut()
            .extract_value_any(lhs, 1, &format!("{name}.lhs_val"));
        let rhs_val = fc
            .builder_mut()
            .extract_value_any(rhs, 1, &format!("{name}.rhs_val"));
        let ok_cmp = emit_field_operation(
            fc,
            FieldOp::Compare,
            lhs_val,
            Some(rhs_val),
            ok_ty,
            &format!("{name}.ok"),
            str_ty_id,
        );
        let err_cmp = emit_field_operation(
            fc,
            FieldOp::Compare,
            lhs_val,
            Some(rhs_val),
            err_ty,
            &format!("{name}.err"),
            str_ty_id,
        );
        fc.builder_mut()
            .select(is_ok, ok_cmp, err_cmp, &format!("{name}.same_cmp"))
    };

    // Tags differ: Ok(0) < Err(1) is the natural unsigned tag ordering.
    let tag_cmp = fc.builder_mut().emit_icmp_ordering(
        lhs_tag,
        rhs_tag,
        &format!("{name}.tag"),
        IntegerSignedness::Unsigned,
    );
    fc.builder_mut()
        .select(tags_eq, same_tag_cmp, tag_cmp, &format!("{name}.cmp"))
}

/// Result<T, E> hash: `Ok(x) → hash_combine(2, hash(x))`, `Err(x) → hash_combine(3, hash(x))`.
/// Discriminants 2/3 + the boost fold mirror the interpreter's `hash_value`.
pub(super) fn emit_result_hash<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    ok_ty: Idx,
    err_ty: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let fallback_i64 = fc.builder_mut().const_i64(0);
    let tag = fc
        .builder_mut()
        .extract_value(val, 0, &format!("{name}.tag"))
        .unwrap_or(fallback_i64);
    let ok_tag = fc.builder_mut().const_i64(RESULT_TAG_OK);
    let is_ok = fc
        .builder_mut()
        .icmp_eq(tag, ok_tag, &format!("{name}.is_ok"));

    let ok_boxed = payload_type_is_rc_boxed(fc.type_info().pool(), ok_ty);
    let err_boxed = payload_type_is_rc_boxed(fc.type_info().pool(), err_ty);

    let payload_hash = if ok_boxed || err_boxed {
        // Reuse the binary boxed helper for the unary hash by passing val twice;
        // emit_field_operation(Hash, ...) reads only the lhs payload.
        let i64_ty = fc.builder_mut().i64_type();
        compare_result_boxed_payload(
            fc,
            FieldOp::Hash,
            val,
            val,
            ok_ty,
            err_ty,
            ok_boxed,
            err_boxed,
            is_ok,
            i64_ty,
            fallback_i64,
            name,
            str_ty_id,
        )
    } else {
        let payload = fc
            .builder_mut()
            .extract_value_any(val, 1, &format!("{name}.payload"));
        let ok_h = emit_field_operation(
            fc,
            FieldOp::Hash,
            payload,
            None,
            ok_ty,
            &format!("{name}.ok"),
            str_ty_id,
        );
        let err_h = emit_field_operation(
            fc,
            FieldOp::Hash,
            payload,
            None,
            err_ty,
            &format!("{name}.err"),
            str_ty_id,
        );
        fc.builder_mut()
            .select(is_ok, ok_h, err_h, &format!("{name}.payload_hash"))
    };

    let two = fc.builder_mut().const_i64(2);
    let three = fc.builder_mut().const_i64(3);
    let disc = fc
        .builder_mut()
        .select(is_ok, two, three, &format!("{name}.disc"));
    emit_hash_combine(fc, disc, payload_hash, name)
}
