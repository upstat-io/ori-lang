//! Inline Option/Result/Tuple comparison for derived trait codegen.
//!
//! These functions generate branchless LLVM IR for structural equality,
//! comparison, and hashing of wrapper types. Each recursively dispatches
//! to [`super::emit_field_operation`] for payload comparisons.

use ori_ir::{FieldOp, OPTION_TAG_NONE, RESULT_TAG_OK};
use ori_types::Idx;

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::emit_field_operation;

/// Compute the same-tag payload result for an `Option` whose Some payload is a
/// boxed recursive back-edge. Branches on `is_none` so the box pointer is
/// dereferenced ONLY on the Some path (the None slot holds a null/invalid box
/// pointer). Returns `none_result` for the None tag, else the recursive
/// `op` result on the loaded-through payload values. Used by eq / compare /
/// hash; `op == Hash` ignores `rhs`.
#[expect(
    clippy::too_many_arguments,
    reason = "shared Option-boxed-payload branch threads lhs/rhs/op/labels"
)]
fn compare_boxed_some_payload<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    inner_ty: Idx,
    is_none: ValueId,
    none_result: ValueId,
    result_ty: LLVMTypeId,
    op: FieldOp,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let func_id = fc
        .builder_mut()
        .current_function()
        .expect("emit within a function");
    let entry_bb = fc
        .builder_mut()
        .current_block()
        .expect("emit within a block");
    let some_bb = fc
        .builder_mut()
        .append_block(func_id, &format!("{name}.some"));
    let join_bb = fc
        .builder_mut()
        .append_block(func_id, &format!("{name}.join"));
    fc.builder_mut().cond_br(is_none, join_bb, some_bb);

    // Some path: load box pointers through the payload slot, recurse.
    fc.builder_mut().position_at_end(some_bb);
    let lhs_box = fc
        .builder_mut()
        .extract_value_any(lhs, 1, &format!("{name}.lhs_box"));
    let rhs_box = fc
        .builder_mut()
        .extract_value_any(rhs, 1, &format!("{name}.rhs_box"));
    let inner_llvm = fc.resolve_type(inner_ty);
    let inner_ty_id = fc.builder_mut().register_type(inner_llvm);
    let lhs_val = fc
        .builder_mut()
        .load(inner_ty_id, lhs_box, &format!("{name}.lhs.boxed"));
    let rhs_opt = if matches!(op, FieldOp::Hash) {
        None
    } else {
        Some(
            fc.builder_mut()
                .load(inner_ty_id, rhs_box, &format!("{name}.rhs.boxed")),
        )
    };
    let payload_result = emit_field_operation(
        fc,
        op,
        lhs_val,
        rhs_opt,
        inner_ty,
        &format!("{name}.payload"),
        str_ty_id,
    );
    let some_end_bb = fc.builder_mut().current_block().unwrap_or(some_bb);
    fc.builder_mut().br(join_bb);

    // Join: none_result on the None edge, payload_result on the Some edge.
    fc.builder_mut().position_at_end(join_bb);
    fc.builder_mut()
        .phi_from_incoming(
            result_ty,
            &[(none_result, entry_bb), (payload_result, some_end_bb)],
            &format!("{name}.same"),
        )
        .unwrap_or(none_result)
}

/// Option<T> equality: tags must match; both None -> true; both Some -> payload eq.
///
/// Uses `extract_value` on already-loaded values, then recursively dispatches
/// to `emit_field_operation` for the payload comparison.
pub(super) fn emit_option_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    inner_ty: Idx,
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

    let none_tag = fc.builder_mut().const_i64(OPTION_TAG_NONE);
    let is_none = fc
        .builder_mut()
        .icmp_eq(lhs_tag, none_tag, &format!("{name}.is_none"));
    let true_val = fc.builder_mut().const_bool(true);

    // Both None → true. Both Some → compare payloads. The payload comparison is
    // computed under a Some-guarded branch ONLY when the payload is boxed: a
    // boxed recursive Some payload slot holds an RC pointer that is null/invalid
    // for None, so an unconditional load-through would dereference null.
    let payload_boxed = crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
        fc.type_info().pool(),
        inner_ty,
    );
    let same_tag_result = if payload_boxed {
        let bool_ty = fc.builder_mut().bool_type();
        compare_boxed_some_payload(
            fc,
            lhs,
            rhs,
            inner_ty,
            is_none,
            true_val,
            bool_ty,
            FieldOp::Equals,
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
        let payload_eq = emit_field_operation(
            fc,
            FieldOp::Equals,
            lhs_val,
            Some(rhs_val),
            inner_ty,
            &format!("{name}.payload"),
            str_ty_id,
        );
        fc.builder_mut()
            .select(is_none, true_val, payload_eq, &format!("{name}.same_eq"))
    };

    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut()
        .select(tags_eq, same_tag_result, false_val, &format!("{name}.eq"))
}

/// Option<T> comparison: None < Some; same tag -> compare payloads.
pub(super) fn emit_option_compare<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    inner_ty: Idx,
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

    // Tags differ: reversed order (None(1) < Some(0) semantically).
    let tag_cmp =
        fc.builder_mut()
            .emit_icmp_ordering(rhs_tag, lhs_tag, &format!("{name}.tag_cmp"), false);

    let none_tag = fc.builder_mut().const_i64(OPTION_TAG_NONE);
    let is_none = fc
        .builder_mut()
        .icmp_eq(lhs_tag, none_tag, &format!("{name}.is_none"));
    let equal_ord = fc.builder_mut().const_i8(1);

    let payload_boxed = crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
        fc.type_info().pool(),
        inner_ty,
    );
    let same_tag_cmp = if payload_boxed {
        let ord_ty = fc.builder_mut().i8_type();
        compare_boxed_some_payload(
            fc,
            lhs,
            rhs,
            inner_ty,
            is_none,
            equal_ord,
            ord_ty,
            FieldOp::Compare,
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
        let payload_cmp = emit_field_operation(
            fc,
            FieldOp::Compare,
            lhs_val,
            Some(rhs_val),
            inner_ty,
            &format!("{name}.payload"),
            str_ty_id,
        );
        fc.builder_mut()
            .select(is_none, equal_ord, payload_cmp, &format!("{name}.same_cmp"))
    };

    fc.builder_mut()
        .select(tags_eq, same_tag_cmp, tag_cmp, &format!("{name}.cmp"))
}

/// `Option<T>` hash: None -> 0, `Some(x)` -> `hash_combine(1, x.hash())`.
pub(super) fn emit_option_hash<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    inner_ty: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let fallback_i64 = fc.builder_mut().const_i64(0);
    let tag = fc
        .builder_mut()
        .extract_value(val, 0, &format!("{name}.tag"))
        .unwrap_or(fallback_i64);
    let none_tag = fc.builder_mut().const_i64(OPTION_TAG_NONE);
    let is_none = fc
        .builder_mut()
        .icmp_eq(tag, none_tag, &format!("{name}.is_none"));

    let payload_boxed = crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
        fc.type_info().pool(),
        inner_ty,
    );
    if payload_boxed {
        // None → none_tag. Some → hash the box contents (guarded so the box
        // pointer is dereferenced only on the Some path). The helper returns
        // none_tag for None and the recursive payload hash for Some.
        let i64_ty = fc.builder_mut().i64_type();
        let combined = compare_boxed_some_payload(
            fc,
            val,
            val,
            inner_ty,
            is_none,
            none_tag,
            i64_ty,
            FieldOp::Hash,
            name,
            str_ty_id,
        );
        // On the Some edge, combine with the tag (XOR) to fold the discriminant
        // into the hash; on the None edge `combined == none_tag` already.
        let some_combined = fc.builder_mut().xor(tag, combined, &format!("{name}.hash"));
        return fc
            .builder_mut()
            .select(is_none, none_tag, some_combined, &format!("{name}.h"));
    }

    let payload = fc
        .builder_mut()
        .extract_value_any(val, 1, &format!("{name}.payload"));
    let payload_hash = emit_field_operation(
        fc,
        FieldOp::Hash,
        payload,
        None,
        inner_ty,
        &format!("{name}.inner"),
        str_ty_id,
    );

    // hash_combine(tag, payload_hash) — simple XOR+shift for now
    let combined = fc
        .builder_mut()
        .xor(tag, payload_hash, &format!("{name}.hash"));
    // None: use tag as hash; Some: use combined
    fc.builder_mut()
        .select(is_none, none_tag, combined, &format!("{name}.h"))
}

/// Compute the same-variant payload-equality for a `Result` whose Ok and/or Err
/// payload is a boxed recursive back-edge. Branches on `is_ok` so each box
/// pointer is dereferenced ONLY on its own variant's path — the inactive
/// variant's payload slot holds a different (possibly scalar) bit pattern, so an
/// unconditional load-through would dereference garbage. Mirrors the
/// Option-boxed branch in [`compare_boxed_some_payload`], extended to two arms.
/// `ok_boxed`/`err_boxed` say whether each arm loads through its box pointer
/// before comparing; non-boxed arms compare the extracted payload inline.
#[expect(
    clippy::too_many_arguments,
    reason = "shared Result-boxed-payload branch threads lhs/rhs/payloads/types"
)]
fn compare_result_boxed_payload<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    ok_ty: Idx,
    err_ty: Idx,
    ok_boxed: bool,
    err_boxed: bool,
    is_ok: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let func_id = fc
        .builder_mut()
        .current_function()
        .expect("emit within a function");
    // When a payload is boxed the Result struct's field-1 slot is a `ptr`; a
    // scalar arm packs its value into that same slot. Store lhs/rhs to allocas
    // and GEP field 1, then load each arm through that slot pointer with the
    // arm's own LLVM type — mirrors the alloca+GEP+load discipline of the
    // Result clone path (`emit_clone_result_rc_inc`).
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

    // Ok path: compare the Ok payload, loading through the box when boxed.
    fc.builder_mut().position_at_end(ok_bb);
    let ok_eq = compare_result_arm_payload(
        fc,
        lhs_slot,
        rhs_slot,
        ok_ty,
        ok_boxed,
        &format!("{name}.ok"),
        str_ty_id,
    );
    let ok_end_bb = fc.builder_mut().current_block().unwrap_or(ok_bb);
    fc.builder_mut().br(join_bb);

    // Err path: compare the Err payload, loading through the box when boxed.
    fc.builder_mut().position_at_end(err_bb);
    let err_eq = compare_result_arm_payload(
        fc,
        lhs_slot,
        rhs_slot,
        err_ty,
        err_boxed,
        &format!("{name}.err"),
        str_ty_id,
    );
    let err_end_bb = fc.builder_mut().current_block().unwrap_or(err_bb);
    fc.builder_mut().br(join_bb);

    let bool_ty = fc.builder_mut().bool_type();
    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut().position_at_end(join_bb);
    fc.builder_mut()
        .phi_from_incoming(
            bool_ty,
            &[(ok_eq, ok_end_bb), (err_eq, err_end_bb)],
            &format!("{name}.same"),
        )
        .unwrap_or(false_val)
}

/// Compare one Result arm's payload given pointers to each side's field-1 slot.
/// When `boxed`, the slot holds an RC box pointer: load the box pointer, then
/// load the inner value through it before comparing. Otherwise load the slot
/// directly as the arm's own LLVM type (reinterpreting the shared slot bytes).
fn compare_result_arm_payload<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs_slot: ValueId,
    rhs_slot: ValueId,
    payload_ty: Idx,
    boxed: bool,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let inner_llvm = fc.resolve_type(payload_ty);
    let inner_ty_id = fc.builder_mut().register_type(inner_llvm);
    let (lhs_val, rhs_val) = if boxed {
        let ptr_ty = fc.builder_mut().ptr_type();
        let lhs_box = fc
            .builder_mut()
            .load(ptr_ty, lhs_slot, &format!("{name}.lhs_box"));
        let rhs_box = fc
            .builder_mut()
            .load(ptr_ty, rhs_slot, &format!("{name}.rhs_box"));
        let lhs_val = fc
            .builder_mut()
            .load(inner_ty_id, lhs_box, &format!("{name}.lhs.boxed"));
        let rhs_val = fc
            .builder_mut()
            .load(inner_ty_id, rhs_box, &format!("{name}.rhs.boxed"));
        (lhs_val, rhs_val)
    } else {
        let lhs_val = fc
            .builder_mut()
            .load(inner_ty_id, lhs_slot, &format!("{name}.lhs_val"));
        let rhs_val = fc
            .builder_mut()
            .load(inner_ty_id, rhs_slot, &format!("{name}.rhs_val"));
        (lhs_val, rhs_val)
    };
    emit_field_operation(
        fc,
        FieldOp::Equals,
        lhs_val,
        Some(rhs_val),
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
        compare_result_boxed_payload(
            fc, lhs, rhs, ok_ty, err_ty, ok_boxed, err_boxed, is_ok, name, str_ty_id,
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

/// Tuple equality: field-by-field comparison, all must match.
pub(super) fn emit_tuple_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    elements: &[Idx],
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    if elements.is_empty() {
        return fc.builder_mut().const_bool(true);
    }

    let mut result = fc.builder_mut().const_bool(true);
    for (i, &elem_ty) in elements.iter().enumerate() {
        let lhs_field =
            fc.builder_mut()
                .extract_value_any(lhs, i as u32, &format!("{name}.lhs_{i}"));
        let rhs_field =
            fc.builder_mut()
                .extract_value_any(rhs, i as u32, &format!("{name}.rhs_{i}"));
        let field_eq = emit_field_operation(
            fc,
            FieldOp::Equals,
            lhs_field,
            Some(rhs_field),
            elem_ty,
            &format!("{name}.f{i}"),
            str_ty_id,
        );
        result = fc
            .builder_mut()
            .and(result, field_eq, &format!("{name}.acc_{i}"));
    }
    result
}
