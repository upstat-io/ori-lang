//! Field-level operations for derived trait codegen.
//!
//! Contains type-driven field comparison (for Eq), field ordering (for
//! Comparable), and field-to-i64 coercion (for Hashable). All dispatch on
//! `TypeInfo` to handle primitives, strings, and nested structs with
//! recursive method calls.

use ori_types::Idx;
use tracing::trace;

use super::super::function_compiler::FunctionCompiler;
use super::super::type_info::TypeInfo;
use super::super::value_id::{LLVMTypeId, ValueId};
use super::emit_method_call_for_derive;

/// Emit an equality comparison for a single field based on its type.
pub(super) fn emit_field_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int
        | TypeInfo::Char
        | TypeInfo::Byte
        | TypeInfo::Bool
        | TypeInfo::Duration
        | TypeInfo::Size
        | TypeInfo::Ordering => fc.builder_mut().icmp_eq(lhs, rhs, name),

        TypeInfo::Float => fc.builder_mut().fcmp_oeq(lhs, rhs, name),

        TypeInfo::Str => emit_str_eq_call(fc, lhs, rhs, name, str_ty_id),

        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let eq_name = fc.intern("eq");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, eq_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[lhs, rhs], name);
                }
            }
            fc.builder_mut().icmp_eq(lhs, rhs, name)
        }

        _ => {
            trace!(
                ?info,
                "unsupported field type for derive Eq — using icmp eq"
            );
            fc.builder_mut().icmp_eq(lhs, rhs, name)
        }
    }
}

/// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
fn emit_str_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();

    let lhs_alloca = fc.builder_mut().alloca(str_ty_id, "lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.builder_mut().alloca(str_ty_id, "rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let eq_fn = fc
        .builder_mut()
        .get_or_declare_function("ori_str_eq", &[ptr_ty, ptr_ty], bool_ty);
    fc.builder_mut()
        .call(eq_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_bool(false))
}

/// Emit a three-way comparison for a single field, returning Ordering (i8).
///
/// Returns: 0 (Less), 1 (Equal), 2 (Greater). For integer types this uses
/// the same icmp+select chain as `lower_int_method("compare")`. For strings
/// it calls `ori_str_compare`. For nested structs it calls their `compare()`
/// method recursively.
pub(super) fn emit_field_compare<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        // Integer-like: signed comparison via icmp
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
            emit_icmp_ordering(fc, lhs, rhs, name, /* signed */ true)
        }

        TypeInfo::Char | TypeInfo::Byte | TypeInfo::Bool | TypeInfo::Ordering => {
            // All unsigned at native width — icmp works directly without widening
            emit_icmp_ordering(fc, lhs, rhs, name, /* signed */ false)
        }

        TypeInfo::Float => emit_fcmp_ordering(fc, lhs, rhs, name),

        TypeInfo::Str => emit_str_compare_call(fc, lhs, rhs, name, str_ty_id),

        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let compare_name = fc.intern("compare");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, compare_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[lhs, rhs], name);
                }
            }
            // Fallback: treat as Equal if no compare method found
            fc.builder_mut().const_i8(1)
        }

        _ => {
            trace!(
                ?info,
                "unsupported field type for derive Comparable — treating as Equal"
            );
            fc.builder_mut().const_i8(1)
        }
    }
}

/// Emit `icmp slt/sgt → select` chain returning Ordering i8.
///
/// Delegates to `IrBuilder::emit_icmp_ordering`.
fn emit_icmp_ordering<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    signed: bool,
) -> ValueId {
    fc.builder_mut().emit_icmp_ordering(lhs, rhs, name, signed)
}

/// Emit `fcmp olt/ogt → select` chain returning Ordering i8.
///
/// Delegates to `IrBuilder::emit_fcmp_ordering`.
fn emit_fcmp_ordering<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
) -> ValueId {
    fc.builder_mut().emit_fcmp_ordering(lhs, rhs, name)
}

/// Call `ori_str_compare(a: ptr, b: ptr) -> i8` via alloca+store pattern.
fn emit_str_compare_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let i8_ty = fc.builder_mut().i8_type();

    let lhs_alloca = fc.builder_mut().alloca(str_ty_id, "cmp_lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.builder_mut().alloca(str_ty_id, "cmp_rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let cmp_fn =
        fc.builder_mut()
            .get_or_declare_function("ori_str_compare", &[ptr_ty, ptr_ty], i8_ty);
    fc.builder_mut()
        .call(cmp_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_i8(1)) // Equal fallback
}

/// Call `ori_str_hash(s: ptr) -> i64` via alloca+store pattern.
fn emit_str_hash_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    name: &str,
) -> ValueId {
    let str_ty = fc.resolve_type(Idx::STR);
    let str_ty_id = fc.builder_mut().register_type(str_ty);
    let ptr_ty = fc.builder_mut().ptr_type();
    let i64_ty = fc.builder_mut().i64_type();

    let val_alloca = fc.builder_mut().alloca(str_ty_id, &format!("{name}.str"));
    fc.builder_mut().store(val, val_alloca);

    let hash_fn = fc
        .builder_mut()
        .get_or_declare_function("ori_str_hash", &[ptr_ty], i64_ty);
    fc.builder_mut()
        .call(hash_fn, &[val_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_i64(0))
}

/// Coerce a field value to i64 for hashing.
pub(super) fn coerce_to_i64<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    field_type: Idx,
    name: &str,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => val,

        TypeInfo::Byte => {
            // Byte is unsigned (8-bit) — use zext to match evaluator semantics
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().zext(val, i64_ty, name)
        }

        TypeInfo::Char | TypeInfo::Ordering => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().sext(val, i64_ty, name)
        }

        TypeInfo::Bool => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().zext(val, i64_ty, name)
        }

        TypeInfo::Float => {
            // Normalize ±0.0 → +0.0 before bitcast to preserve hash contract:
            // (-0.0).equals(0.0) is true, so their hashes must match.
            let pos_zero = fc.builder_mut().const_f64(0.0);
            let is_zero = fc
                .builder_mut()
                .fcmp_oeq(val, pos_zero, &format!("{name}.is_zero"));
            let normalized =
                fc.builder_mut()
                    .select(is_zero, pos_zero, val, &format!("{name}.normalized"));
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().bitcast(normalized, i64_ty, name)
        }

        TypeInfo::Str => emit_str_hash_call(fc, val, name),

        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let hash_name = fc.intern("hash");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, hash_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[val], name);
                }
            }
            fc.builder_mut().const_i64(0)
        }

        _ => {
            trace!(?info, "unsupported field type for derive Hash — using 0");
            fc.builder_mut().const_i64(0)
        }
    }
}
