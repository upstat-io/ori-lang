//! Field-level operations for derived trait codegen.
//!
//! Provides [`emit_field_operation`], a unified dispatcher that handles
//! equality (Eq), comparison (Comparable), and hash coercion (Hashable)
//! for all field types via a single `TypeInfo` match.
//!
//! Submodules:
//! - [`wrapper_cmp`]: inline Option/Result/Tuple comparison
//! - [`thunks`]: thunk generator functions for list/map callbacks

mod thunks;
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
        // §04 integer narrowing may produce i8/i16/i32 struct fields for
        // types with bounded ranges. Hash requires canonical i64 width.
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
                // §05 float narrowing: widen f32 back to f64 before hashing.
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
            FieldOp::Equals => emit_str_eq_call(fc, lhs, expect_rhs(rhs), name, str_ty_id),
            FieldOp::Compare => emit_str_compare_call(fc, lhs, expect_rhs(rhs), name, str_ty_id),
            FieldOp::Hash => emit_str_hash_call(fc, lhs, name, str_ty_id),
        },

        // List/Set: use ori_list_eq_scalar for equality (byte-level comparison)
        TypeInfo::List { element } | TypeInfo::Set { element } => {
            let elem = *element;
            match op {
                FieldOp::Equals => {
                    emit_list_eq_call(fc, lhs, expect_rhs(rhs), elem, name, str_ty_id)
                }
                FieldOp::Compare => fc.builder_mut().const_i8(1), // Equal fallback
                FieldOp::Hash => fc.builder_mut().const_i64(0),   // Hash fallback
            }
        }

        // Map: use ori_map_eq with key/value comparison callbacks
        TypeInfo::Map { key, value } => {
            let key = *key;
            let value = *value;
            match op {
                FieldOp::Equals => {
                    emit_map_eq_call(fc, lhs, expect_rhs(rhs), key, value, name, str_ty_id)
                }
                FieldOp::Compare => fc.builder_mut().const_i8(1), // Equal fallback
                FieldOp::Hash => fc.builder_mut().const_i64(0),   // Hash fallback
            }
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
    let nested_name = fc.type_idx_to_name(field_type);
    let method = fc.intern(trait_kind.method_name());
    if let Some(type_name) = nested_name {
        if let Some((fid, abi)) = fc.get_method_function(type_name, method) {
            return match op {
                FieldOp::Hash => emit_method_call_for_derive(fc, fid, &abi, &[lhs], name),
                _ => emit_method_call_for_derive(fc, fid, &abi, &[lhs, expect_rhs(rhs)], name),
            };
        }
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
                wrapper_cmp::emit_result_eq(fc, lhs, expect_rhs(rhs), *ok, *err, name, str_ty_id)
            }
            FieldOp::Compare => fc.builder_mut().const_i8(1),
            FieldOp::Hash => fc.builder_mut().const_i64(0),
        },
        TypeInfo::Tuple { elements } => {
            let elems = elements.clone();
            match op {
                FieldOp::Equals => {
                    wrapper_cmp::emit_tuple_eq(fc, lhs, expect_rhs(rhs), &elems, name, str_ty_id)
                }
                FieldOp::Compare => fc.builder_mut().const_i8(1),
                FieldOp::Hash => fc.builder_mut().const_i64(0),
            }
        }
        _ => emit_fallback(fc, op, lhs, rhs, name),
    }
}

// String runtime helpers (alloca+store+call pattern)

/// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
fn emit_str_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let lhs_alloca = fc.entry_alloca(str_ty_id, "lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, "rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let eq_fn = fc.builder_mut().runtime_fn("ori_str_eq");
    fc.builder_mut()
        .call(eq_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_bool(false))
}

/// Call `ori_str_compare(a: ptr, b: ptr) -> i8` via alloca+store pattern.
fn emit_str_compare_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let lhs_alloca = fc.entry_alloca(str_ty_id, "cmp_lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, "cmp_rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let cmp_fn = fc.builder_mut().runtime_fn("ori_str_compare");
    fc.builder_mut()
        .call(cmp_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_i8(1)) // Equal fallback
}

/// Call `ori_str_hash(s: ptr) -> i64` via alloca+store pattern.
fn emit_str_hash_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let val_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.str"));
    fc.builder_mut().store(val, val_alloca);

    let hash_fn = fc.builder_mut().runtime_fn("ori_str_hash");
    fc.builder_mut()
        .call(hash_fn, &[val_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_i64(0))
}

/// Emit list equality comparison via the appropriate runtime function.
///
/// For scalar element types (int, float, bool, byte, char), uses
/// `ori_list_eq_scalar` (byte-level memcmp — correct because byte
/// representation matches semantic equality for scalars).
///
/// For non-scalar element types (str, nested collections, structs),
/// uses `ori_list_eq_deep` with a per-element equality callback,
/// because byte-level comparison fails (e.g., two independently
/// allocated heap strings have different data pointers but equal content).
fn emit_list_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    element_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(element_type);
    let elem_size = compute_elem_size(fc, element_type, &info);

    let lhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.lhs_list"));
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.rhs_list"));
    fc.builder_mut().store(rhs, rhs_alloca);

    let elem_size_val = fc.builder_mut().const_i64(elem_size);

    // Try deep comparison for non-scalar element types
    let elem_eq_thunk = if needs_deep_comparison(&info) {
        thunks::get_or_create_derive_eq_thunk(fc, element_type, &info)
    } else {
        None
    };

    if let Some(thunk) = elem_eq_thunk {
        let eq_fn = fc.builder_mut().runtime_fn("ori_list_eq_deep");
        fc.builder_mut()
            .call(eq_fn, &[lhs_alloca, rhs_alloca, elem_size_val, thunk], name)
            .unwrap_or_else(|| fc.builder_mut().const_bool(false))
    } else {
        let eq_fn = fc.builder_mut().runtime_fn("ori_list_eq_scalar");
        fc.builder_mut()
            .call(eq_fn, &[lhs_alloca, rhs_alloca, elem_size_val], name)
            .unwrap_or_else(|| fc.builder_mut().const_bool(false))
    }
}

/// Call `ori_map_eq(a, b, key_size, val_size, key_eq, key_hash, val_eq) -> bool`
/// via alloca+store pattern.
///
/// Generates or references thunks for key equality, key hashing, and value
/// equality based on the key and value types.
fn emit_map_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    key_type: Idx,
    val_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let lhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.lhs_map"));
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.rhs_map"));
    fc.builder_mut().store(rhs, rhs_alloca);

    let key_info = fc.type_info().get(key_type);
    let val_info = fc.type_info().get(val_type);

    let key_size = compute_elem_size(fc, key_type, &key_info);
    let val_size = compute_elem_size(fc, val_type, &val_info);
    let key_size_val = fc.builder_mut().const_i64(key_size);
    let val_size_val = fc.builder_mut().const_i64(val_size);

    // Get or create thunk function pointers for key_eq, key_hash, val_eq
    let key_eq = thunks::get_or_create_derive_eq_thunk(fc, key_type, &key_info);
    let key_hash = thunks::get_or_create_derive_hash_thunk(fc, key_type, &key_info);
    let val_eq = thunks::get_or_create_derive_eq_thunk(fc, val_type, &val_info);

    let (Some(key_eq), Some(key_hash), Some(val_eq)) = (key_eq, key_hash, val_eq) else {
        trace!(
            ?key_info,
            ?val_info,
            "map eq: unsupported key/val type for thunks"
        );
        return fc.builder_mut().const_bool(false);
    };

    let eq_fn = fc.builder_mut().runtime_fn("ori_map_eq");
    fc.builder_mut()
        .call(
            eq_fn,
            &[
                lhs_alloca,
                rhs_alloca,
                key_size_val,
                val_size_val,
                key_eq,
                key_hash,
                val_eq,
            ],
            name,
        )
        .unwrap_or_else(|| fc.builder_mut().const_bool(false))
}

/// Check if an element type requires deep (callback-based) comparison
/// rather than byte-level memcmp.
fn needs_deep_comparison(info: &TypeInfo) -> bool {
    matches!(
        info,
        TypeInfo::Str
            | TypeInfo::List { .. }
            | TypeInfo::Set { .. }
            | TypeInfo::Map { .. }
            | TypeInfo::Struct { .. }
            | TypeInfo::Enum { .. }
            | TypeInfo::Option { .. }
            | TypeInfo::Result { .. }
            | TypeInfo::Tuple { .. }
    )
}

/// Compute element size in bytes for a given `TypeInfo`.
///
/// For compound types (Struct, Option, Result, Tuple), resolves the LLVM
/// type to compute the actual store size. Structs with fat-pointer fields
/// (e.g., `str` = 24 bytes) cannot use `fields.len() * 8`.
fn compute_elem_size<'a>(fc: &FunctionCompiler<'_, 'a, 'a, '_>, ty: Idx, info: &TypeInfo) -> i64 {
    match info {
        TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Ordering => 1,
        TypeInfo::Char => 4,
        TypeInfo::Str | TypeInfo::List { .. } | TypeInfo::Set { .. } | TypeInfo::Map { .. } => 24,
        TypeInfo::Struct { .. }
        | TypeInfo::Option { .. }
        | TypeInfo::Result { .. }
        | TypeInfo::Tuple { .. } => {
            let llvm_ty = fc.resolve_type(ty);
            crate::codegen::TypeLayoutResolver::type_store_size(llvm_ty) as i64
        }
        _ => {
            // §04.4 Phase C: check if this element type is narrowed as a
            // collection element. For `int` elements in narrowed collections,
            // the element size is 1/2/4 instead of canonical 8.
            if let Some(plan) = fc.repr_plan() {
                let resolved = fc.pool().resolve_fully(ty);
                if fc.pool().tag(resolved) == ori_types::Tag::Int {
                    for idx in plan.decision_indices() {
                        if let Some(ori_repr::MachineRepr::FatPointer(
                            ori_repr::FatRepr::Collection { ref element_repr },
                        )) = plan.get_repr(idx)
                        {
                            if let ori_repr::MachineRepr::Int { width, .. } = element_repr.as_ref()
                            {
                                if *width != ori_repr::IntWidth::I64 {
                                    return i64::from(width.size_bytes());
                                }
                            }
                        }
                    }
                }
            }
            8
        }
    }
}
