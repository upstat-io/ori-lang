//! Inline Tuple comparison/hash for derived trait codegen.
//!
//! Generates branchless LLVM IR for structural equality, lexicographic
//! comparison, and hashing of tuple fields. Recurses to
//! [`super::emit_field_operation`]; folds via [`super::wrapper_cmp::emit_hash_combine`].

use ori_ir::FieldOp;
use ori_types::Idx;

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::emit_field_operation;
use super::wrapper_cmp::emit_hash_combine;

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

/// Tuple compare: lexicographic short-circuit. Reverse fold so element 0
/// decides unless Equal, then element 1, etc.
pub(super) fn emit_tuple_compare<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    elements: &[Idx],
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let equal = fc.builder_mut().const_i8(1);
    if elements.is_empty() {
        return equal;
    }
    let mut acc = equal;
    for (i, &elem_ty) in elements.iter().enumerate().rev() {
        let lhs_field =
            fc.builder_mut()
                .extract_value_any(lhs, i as u32, &format!("{name}.lhs_{i}"));
        let rhs_field =
            fc.builder_mut()
                .extract_value_any(rhs, i as u32, &format!("{name}.rhs_{i}"));
        let elem_cmp = emit_field_operation(
            fc,
            FieldOp::Compare,
            lhs_field,
            Some(rhs_field),
            elem_ty,
            &format!("{name}.f{i}"),
            str_ty_id,
        );
        let is_eq = fc
            .builder_mut()
            .icmp_eq(elem_cmp, equal, &format!("{name}.f{i}_eq"));
        acc = fc
            .builder_mut()
            .select(is_eq, acc, elem_cmp, &format!("{name}.acc_{i}"));
    }
    acc
}

/// Tuple hash: `h = 0; h = hash_combine(h, hash(elem))` per element — mirrors
/// the interpreter's `hash_value` tuple fold (seed 0).
pub(super) fn emit_tuple_hash<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    elements: &[Idx],
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let mut acc = fc.builder_mut().const_i64(0);
    for (i, &elem_ty) in elements.iter().enumerate() {
        let field = fc
            .builder_mut()
            .extract_value_any(val, i as u32, &format!("{name}.f{i}"));
        let elem_hash = emit_field_operation(
            fc,
            FieldOp::Hash,
            field,
            None,
            elem_ty,
            &format!("{name}.h{i}"),
            str_ty_id,
        );
        acc = emit_hash_combine(fc, acc, elem_hash, &format!("{name}.c{i}"));
    }
    acc
}
