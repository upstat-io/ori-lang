//! Field-level operations for derived trait codegen.
//!
//! Provides [`emit_field_operation`], a unified dispatcher that handles
//! equality (Eq), comparison (Comparable), and hash coercion (Hashable)
//! for all field types via a single `TypeInfo` match.
//!
//! Submodules:
//! - [`wrapper_cmp`]: inline Option comparison/hash + the shared `hash_combine`
//! - [`result_cmp`]: inline Result comparison/hash
//! - [`tuple_cmp`]: inline Tuple comparison/hash
//! - [`thunks`]: thunk generator functions for list/map callbacks

mod option_thunk;
mod result_cmp;
mod result_thunk;
mod runtime_calls;
mod thunks;
mod tuple_cmp;
mod wrapper_cmp;

use ori_ir::{DerivedTrait, FieldOp};
use ori_types::Idx;
use tracing::trace;

use super::super::function_compiler::FunctionCompiler;
use super::super::type_info::TypeInfo;
use super::super::value_id::{LLVMTypeId, ValueId};
use super::emit_method_call_for_derive;

/// Emit a field-level operation for the given type.
///
/// Dispatches once on `TypeInfo`, then applies the requested [`FieldOp`].
/// For binary ops (`Equals`, `Compare`), `rhs` must be `Some`. For `Hash`
/// (unary), `rhs` should be `None`.
pub(super) fn emit_field_operation<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        // Integer-like signed: signed compare, sext to i64 for hash.
        // Integer narrowing may produce i8/i16/i32 struct fields for types
        // with bounded ranges. Hash requires canonical i64 width.
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, true)
            }
            FieldOp::Hash => fc.builder_mut().sext_to_i64_if_narrower(lhs, name),
        },

        // Unsigned small: unsigned compare, zext to i64 for hash
        TypeInfo::Byte | TypeInfo::Bool => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, false)
            }
            FieldOp::Hash => {
                let i64_ty = fc.builder_mut().i64_type();
                fc.builder_mut().zext(lhs, i64_ty, name)
            }
        },

        // Char/Ordering: unsigned compare, sext to i64 for hash
        TypeInfo::Char | TypeInfo::Ordering => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, false)
            }
            FieldOp::Hash => {
                let i64_ty = fc.builder_mut().i64_type();
                fc.builder_mut().sext(lhs, i64_ty, name)
            }
        },

        TypeInfo::Float => match op {
            FieldOp::Equals => fc.builder_mut().fcmp_oeq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => fc
                .builder_mut()
                .emit_fcmp_ordering(lhs, expect_rhs(rhs), name),
            FieldOp::Hash => {
                // Float narrowing: widen f32 back to f64 before hashing.
                // This ensures hash(narrowed_struct) == hash(canonical_struct).
                let hash_val = fc.builder_mut().fpext_to_f64_if_narrower(lhs, name);
                // Normalize ±0.0 → +0.0 before bitcast to preserve hash contract:
                // (-0.0).equals(0.0) is true, so their hashes must match.
                let pos_zero = fc.builder_mut().const_f64(0.0);
                let is_zero =
                    fc.builder_mut()
                        .fcmp_oeq(hash_val, pos_zero, &format!("{name}.is_zero"));
                let normalized = fc.builder_mut().select(
                    is_zero,
                    pos_zero,
                    hash_val,
                    &format!("{name}.normalized"),
                );
                let i64_ty = fc.builder_mut().i64_type();
                fc.builder_mut().bitcast(normalized, i64_ty, name)
            }
        },

        TypeInfo::Str => match op {
            FieldOp::Equals => {
                runtime_calls::emit_str_eq_call(fc, lhs, expect_rhs(rhs), name, str_ty_id)
            }
            FieldOp::Compare => {
                runtime_calls::emit_str_compare_call(fc, lhs, expect_rhs(rhs), name, str_ty_id)
            }
            FieldOp::Hash => runtime_calls::emit_str_hash_call(fc, lhs, name, str_ty_id),
        },

        // List/Set/Map: byte-level (scalar) or thunk-driven (deep) equality;
        // Compare/Hash fall back to Equal/0. `field_type` is the enclosing
        // collection idx, threaded so narrowed element strides key on it.
        TypeInfo::List { .. } | TypeInfo::Set { .. } | TypeInfo::Map { .. } => {
            emit_collection_field_op(fc, op, lhs, rhs, field_type, &info, name, str_ty_id)
        }

        // Wrapper types: Option, Result, Tuple — structural comparison
        TypeInfo::Option { .. } | TypeInfo::Result { .. } | TypeInfo::Tuple { .. } => {
            emit_wrapper_field_op(fc, op, lhs, rhs, &info, name, str_ty_id)
        }

        TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
            emit_user_type_field_op(fc, op, lhs, rhs, field_type, name)
        }

        _ => {
            trace!(
                ?info,
                ?op,
                "unsupported field type for derive — using fallback"
            );
            emit_fallback(fc, op, lhs, rhs, name)
        }
    }
}

/// Unwrap `rhs` for binary operations (Eq, Compare).
fn expect_rhs(rhs: Option<ValueId>) -> ValueId {
    rhs.expect("binary field op (Equals/Compare) requires rhs")
}

/// Fallback values when a type doesn't support the operation.
fn emit_fallback<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    name: &str,
) -> ValueId {
    match op {
        FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
        FieldOp::Compare => fc.builder_mut().const_i8(1), // Equal
        FieldOp::Hash => fc.builder_mut().const_i64(0),
    }
}

/// Dispatch a field op for a collection type (List, Set, Map).
///
/// `Equals` routes to `ori_list_eq_*` / `ori_map_eq` (scalar or deep);
/// `Compare`/`Hash` fall back to `Equal` / `0`. `collection_type` is the
/// enclosing collection idx, threaded into element-size computation so a
/// narrowed element stride keys on that specific collection.
fn emit_collection_field_op<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    collection_type: Idx,
    info: &TypeInfo,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    match op {
        FieldOp::Equals => match info {
            TypeInfo::List { element } | TypeInfo::Set { element } => {
                runtime_calls::emit_list_eq_call(
                    fc,
                    lhs,
                    expect_rhs(rhs),
                    collection_type,
                    *element,
                    name,
                    str_ty_id,
                )
            }
            TypeInfo::Map { key, value } => runtime_calls::emit_map_eq_call(
                fc,
                lhs,
                expect_rhs(rhs),
                collection_type,
                *key,
                *value,
                name,
                str_ty_id,
            ),
            _ => emit_fallback(fc, op, lhs, rhs, name),
        },
        FieldOp::Compare => fc.builder_mut().const_i8(1), // Equal fallback
        FieldOp::Hash => fc.builder_mut().const_i64(0),   // Hash fallback
    }
}

/// Emit a field op for a user-defined type (struct or enum) by calling its
/// derived trait method (e.g., `equals`, `compare`, `hash`).
fn emit_user_type_field_op<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    field_type: Idx,
    name: &str,
) -> ValueId {
    let trait_kind = match op {
        FieldOp::Equals => DerivedTrait::Eq,
        FieldOp::Compare => DerivedTrait::Comparable,
        FieldOp::Hash => DerivedTrait::Hashable,
    };
    let method = fc.intern(trait_kind.method_name());
    let resolved = fc.get_derived_method_for_type(field_type, method);
    trace!(
        target: "ori_llvm::codegen::derive_codegen",
        method = %trait_kind.method_name(),
        field_type = ?field_type,
        resolved = resolved.is_some(),
        "derive field-op dispatch lookup"
    );
    if let Some((fid, abi)) = resolved {
        return match op {
            FieldOp::Hash => emit_method_call_for_derive(fc, fid, &abi, &[lhs], name),
            _ => emit_method_call_for_derive(fc, fid, &abi, &[lhs, expect_rhs(rhs)], name),
        };
    }
    emit_fallback(fc, op, lhs, rhs, name)
}

/// Dispatch a field operation for wrapper types (Option, Result, Tuple).
fn emit_wrapper_field_op<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    info: &TypeInfo,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    match info {
        TypeInfo::Option { inner } => match op {
            FieldOp::Equals => {
                wrapper_cmp::emit_option_eq(fc, lhs, expect_rhs(rhs), *inner, name, str_ty_id)
            }
            FieldOp::Compare => {
                wrapper_cmp::emit_option_compare(fc, lhs, expect_rhs(rhs), *inner, name, str_ty_id)
            }
            FieldOp::Hash => wrapper_cmp::emit_option_hash(fc, lhs, *inner, name, str_ty_id),
        },
        TypeInfo::Result { ok, err } => match op {
            FieldOp::Equals => {
                result_cmp::emit_result_eq(fc, lhs, expect_rhs(rhs), *ok, *err, name, str_ty_id)
            }
            FieldOp::Compare => result_cmp::emit_result_compare(
                fc,
                lhs,
                expect_rhs(rhs),
                *ok,
                *err,
                name,
                str_ty_id,
            ),
            FieldOp::Hash => result_cmp::emit_result_hash(fc, lhs, *ok, *err, name, str_ty_id),
        },
        TypeInfo::Tuple { elements } => {
            let elems = elements.clone();
            match op {
                FieldOp::Equals => {
                    tuple_cmp::emit_tuple_eq(fc, lhs, expect_rhs(rhs), &elems, name, str_ty_id)
                }
                FieldOp::Compare => {
                    tuple_cmp::emit_tuple_compare(fc, lhs, expect_rhs(rhs), &elems, name, str_ty_id)
                }
                FieldOp::Hash => tuple_cmp::emit_tuple_hash(fc, lhs, &elems, name, str_ty_id),
            }
        }
        _ => emit_fallback(fc, op, lhs, rhs, name),
    }
}
