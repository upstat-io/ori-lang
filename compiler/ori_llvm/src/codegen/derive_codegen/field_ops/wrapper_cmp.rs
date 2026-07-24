//! Inline Option comparison and hashing for derived trait codegen.
//!
//! These functions generate branchless LLVM IR for structural equality,
//! comparison, and hashing of wrapper types. Each recursively dispatches
//! to [`super::emit_field_operation`] for payload comparisons.

use ori_ir::{FieldOp, OPTION_TAG_NONE};
use ori_types::Idx;

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::ir_builder::IntegerSignedness;
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

    // Why: a boxed Some payload slot holds an RC pointer that is null for None,
    // so the boxed comparison runs under a Some-guarded branch (unconditional
    // load-through would dereference null). Non-boxed payloads stay branchless.
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
    let tag_cmp = fc.builder_mut().emit_icmp_ordering(
        rhs_tag,
        lhs_tag,
        &format!("{name}.tag_cmp"),
        IntegerSignedness::Unsigned,
    );

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
        let payload_hash = compare_boxed_some_payload(
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
        // Match the interpreter oracle: Some(x) -> hash_combine(1, h(x)), None -> 0.
        let one = fc.builder_mut().const_i64(1);
        let some_combined = emit_hash_combine(fc, one, payload_hash, name);
        let zero = fc.builder_mut().const_i64(0);
        return fc
            .builder_mut()
            .select(is_none, zero, some_combined, &format!("{name}.h"));
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

    // Match the interpreter oracle: Some(x) -> hash_combine(1, h(x)), None -> 0.
    let one = fc.builder_mut().const_i64(1);
    let some_combined = emit_hash_combine(fc, one, payload_hash, name);
    let zero = fc.builder_mut().const_i64(0);
    fc.builder_mut()
        .select(is_none, zero, some_combined, &format!("{name}.h"))
}

/// boost `hash_combine`: `seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))`.
/// Mirrors the interpreter's `function_val_hash_combine` exactly (all wrapping
/// i64; `>> 2` is arithmetic per the signed interpreter shift) so derived
/// Result/Tuple hashes match the parity oracle.
pub(in crate::codegen::derive_codegen) fn emit_hash_combine<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    seed: ValueId,
    value: ValueId,
    name: &str,
) -> ValueId {
    let magic = fc.builder_mut().const_i64(0x9e37_79b9);
    let six = fc.builder_mut().const_i64(6);
    let two = fc.builder_mut().const_i64(2);
    let s6 = fc.builder_mut().shl(seed, six, &format!("{name}.hc_shl"));
    let s2 = fc.builder_mut().ashr(seed, two, &format!("{name}.hc_ashr"));
    let t1 = fc.builder_mut().add(value, magic, &format!("{name}.hc_t1"));
    let t2 = fc.builder_mut().add(t1, s6, &format!("{name}.hc_t2"));
    let t3 = fc.builder_mut().add(t2, s2, &format!("{name}.hc_t3"));
    fc.builder_mut().xor(seed, t3, &format!("{name}.hc"))
}
