//! Inline Option/Result/Tuple comparison for derived trait codegen.
//!
//! These functions generate branchless LLVM IR for structural equality,
//! comparison, and hashing of wrapper types. Each recursively dispatches
//! to [`super::emit_field_operation`] for payload comparisons.

use ori_ir::{FieldOp, OPTION_TAG_NONE, RESULT_TAG_OK};
use ori_types::Idx;

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::emit_field_operation;

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

    // Both None → true. Both Some → compare payloads.
    let none_tag = fc.builder_mut().const_i64(OPTION_TAG_NONE);
    let is_none = fc
        .builder_mut()
        .icmp_eq(lhs_tag, none_tag, &format!("{name}.is_none"));
    let true_val = fc.builder_mut().const_bool(true);
    let same_tag_result =
        fc.builder_mut()
            .select(is_none, true_val, payload_eq, &format!("{name}.same_eq"));

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

    let none_tag = fc.builder_mut().const_i64(OPTION_TAG_NONE);
    let is_none = fc
        .builder_mut()
        .icmp_eq(lhs_tag, none_tag, &format!("{name}.is_none"));
    let equal_ord = fc.builder_mut().const_i8(1);
    let same_tag_cmp =
        fc.builder_mut()
            .select(is_none, equal_ord, payload_cmp, &format!("{name}.same_cmp"));

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
    let none_tag = fc.builder_mut().const_i64(OPTION_TAG_NONE);
    let is_none = fc
        .builder_mut()
        .icmp_eq(tag, none_tag, &format!("{name}.is_none"));
    fc.builder_mut()
        .select(is_none, none_tag, combined, &format!("{name}.h"))
}

/// Result<T, E> equality: tags must match; same variant -> compare payloads.
///
/// Uses branchless select. For Ok/Err with different types, always evaluates
/// both payload comparisons but only uses the relevant one. This is safe
/// because equality thunks have no observable side effects.
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

    let lhs_val = fc
        .builder_mut()
        .extract_value_any(lhs, 1, &format!("{name}.lhs_val"));
    let rhs_val = fc
        .builder_mut()
        .extract_value_any(rhs, 1, &format!("{name}.rhs_val"));

    // Compare as Ok type
    let ok_eq = emit_field_operation(
        fc,
        FieldOp::Equals,
        lhs_val,
        Some(rhs_val),
        ok_ty,
        &format!("{name}.ok"),
        str_ty_id,
    );
    // Compare as Err type
    let err_eq = emit_field_operation(
        fc,
        FieldOp::Equals,
        lhs_val,
        Some(rhs_val),
        err_ty,
        &format!("{name}.err"),
        str_ty_id,
    );

    // Distinguish Ok vs Err for payload type selection
    let ok_tag = fc.builder_mut().const_i64(RESULT_TAG_OK);
    let is_ok = fc
        .builder_mut()
        .icmp_eq(lhs_tag, ok_tag, &format!("{name}.is_ok"));
    let same_tag_eq = fc
        .builder_mut()
        .select(is_ok, ok_eq, err_eq, &format!("{name}.same_eq"));

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
