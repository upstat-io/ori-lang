//! Field-level operations for derived trait codegen.
//!
//! Contains type-driven field comparison (for Eq) and field-to-i64 coercion
//! (for Hashable). Both dispatch on `TypeInfo` to handle primitives, strings,
//! and nested structs with recursive method calls.

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

        TypeInfo::Char | TypeInfo::Byte | TypeInfo::Ordering => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().sext(val, i64_ty, name)
        }

        TypeInfo::Bool => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().zext(val, i64_ty, name)
        }

        TypeInfo::Float => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().bitcast(val, i64_ty, name)
        }

        TypeInfo::Str => fc
            .builder_mut()
            .extract_value(val, 0, &format!("{name}.len"))
            .unwrap_or_else(|| fc.builder_mut().const_i64(0)),

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
