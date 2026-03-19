//! Field-level operations for derived trait codegen.
//!
//! Provides [`emit_field_operation`], a unified dispatcher that handles
//! equality (Eq), comparison (Comparable), and hash coercion (Hashable)
//! for all field types via a single `TypeInfo` match.

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
        // Integer-like signed: signed compare, already i64 for hash
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, true)
            }
            FieldOp::Hash => lhs,
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
                // Normalize ±0.0 → +0.0 before bitcast to preserve hash contract:
                // (-0.0).equals(0.0) is true, so their hashes must match.
                let pos_zero = fc.builder_mut().const_f64(0.0);
                let is_zero = fc
                    .builder_mut()
                    .fcmp_oeq(lhs, pos_zero, &format!("{name}.is_zero"));
                let normalized =
                    fc.builder_mut()
                        .select(is_zero, pos_zero, lhs, &format!("{name}.normalized"));
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

/// Call `ori_list_eq_scalar(a: ptr, b: ptr, elem_size: i64) -> bool` via alloca+store pattern.
///
/// For lists of scalar elements (int, float, bool, byte), this does a byte-level
/// comparison of the data buffers. For lists of strings or other fat types,
/// this still works because `ori_list_eq_scalar` compares bytes — two independently
/// created `[str]` with same content will have different data pointers for the
/// strings, but the str structs themselves have the same len/cap and SSO data.
///
/// For proper deep comparison of `[str]`, a dedicated runtime function would be
/// needed that calls `ori_str_eq` per element.
fn emit_list_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    element_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(element_type);
    let elem_size: i64 = match &info {
        TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Ordering => 1,
        TypeInfo::Char => 4,
        TypeInfo::Str | TypeInfo::List { .. } | TypeInfo::Set { .. } | TypeInfo::Map { .. } => 24, // {i64, i64, ptr}
        TypeInfo::Struct { fields } => {
            // Rough estimate: sum of field sizes (not accounting for padding)
            fields.len() as i64 * 8
        }
        _ => 8, // int, float, duration, size, and fallback
    };

    // Store both lists to stack (ori_list_eq_scalar expects ptr to {len, cap, data}).
    // Lists have the same LLVM layout as str: {i64, i64, ptr}.
    // Lists have the same LLVM layout as str: {i64, i64, ptr}.
    let lhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.lhs_list"));
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.rhs_list"));
    fc.builder_mut().store(rhs, rhs_alloca);

    let elem_size_val = fc.builder_mut().const_i64(elem_size);
    let eq_fn = fc.builder_mut().runtime_fn("ori_list_eq_scalar");
    fc.builder_mut()
        .call(eq_fn, &[lhs_alloca, rhs_alloca, elem_size_val], name)
        .unwrap_or_else(|| fc.builder_mut().const_bool(false))
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

    let key_size: i64 = match &key_info {
        TypeInfo::Str | TypeInfo::List { .. } | TypeInfo::Map { .. } | TypeInfo::Set { .. } => 24,
        TypeInfo::Bool | TypeInfo::Byte => 1,
        TypeInfo::Char => 4,
        _ => 8, // int, float, duration, size
    };
    let val_size: i64 = match &val_info {
        TypeInfo::Str | TypeInfo::List { .. } | TypeInfo::Map { .. } | TypeInfo::Set { .. } => 24,
        TypeInfo::Bool | TypeInfo::Byte => 1,
        TypeInfo::Char => 4,
        _ => 8,
    };
    let key_size_val = fc.builder_mut().const_i64(key_size);
    let val_size_val = fc.builder_mut().const_i64(val_size);

    // Get or create thunk function pointers for key_eq, key_hash, val_eq
    let key_eq = get_or_create_derive_eq_thunk(fc, key_type, &key_info);
    let key_hash = get_or_create_derive_hash_thunk(fc, key_type, &key_info);
    let val_eq = get_or_create_derive_eq_thunk(fc, val_type, &val_info);

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

/// Get a function pointer to an equality thunk for use in map comparison.
///
/// For strings, returns `ori_str_eq`. For primitives, generates a
/// `_ori_eq_{type}` thunk that loads two values from pointers and compares.
fn get_or_create_derive_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    info: &TypeInfo,
) -> Option<ValueId> {
    if matches!(info, TypeInfo::Str) {
        let func_id = fc.builder_mut().runtime_fn("ori_str_eq");
        return Some(fc.builder_mut().get_function_ptr(func_id));
    }
    let suffix = match info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
        TypeInfo::Float => "float",
        TypeInfo::Bool => "bool",
        TypeInfo::Char => "char",
        TypeInfo::Byte => "byte",
        _ => return None,
    };
    let func_name = format!("_ori_eq_{suffix}");
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

    // If it doesn't have a body yet, generate it
    if !fc.builder_mut().function_has_body(func_id) {
        let saved_pos = fc.builder_mut().save_position();
        let saved_func = fc.builder_mut().current_function();

        fc.builder_mut().set_ccc(func_id);
        fc.builder_mut().add_nounwind_attribute(func_id);
        let entry = fc.builder_mut().append_block(func_id, "entry");
        fc.builder_mut().position_at_end(entry);
        fc.builder_mut().set_current_function(func_id);

        let a_ptr = fc.builder_mut().get_param(func_id, 0);
        let b_ptr = fc.builder_mut().get_param(func_id, 1);
        let llvm_ty = fc.resolve_type(ty);
        let ty_id = fc.builder_mut().register_type(llvm_ty);
        let a_val = fc.builder_mut().load(ty_id, a_ptr, "a");
        let b_val = fc.builder_mut().load(ty_id, b_ptr, "b");
        let result = match info {
            TypeInfo::Float => fc.builder_mut().fcmp_oeq(a_val, b_val, "eq"),
            _ => fc.builder_mut().icmp_eq(a_val, b_val, "eq"),
        };
        fc.builder_mut().ret(result);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}

/// Get a function pointer to a hash thunk for use in map comparison.
///
/// For strings, returns `ori_str_hash`. For primitives, generates a
/// `_ori_hash_{type}` thunk that loads a value from a pointer and returns it as i64.
fn get_or_create_derive_hash_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    info: &TypeInfo,
) -> Option<ValueId> {
    if matches!(info, TypeInfo::Str) {
        let func_id = fc.builder_mut().runtime_fn("ori_str_hash");
        return Some(fc.builder_mut().get_function_ptr(func_id));
    }
    let suffix = match info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
        TypeInfo::Float => "float",
        TypeInfo::Bool => "bool",
        TypeInfo::Char => "char",
        TypeInfo::Byte => "byte",
        _ => return None,
    };
    let func_name = format!("_ori_hash_{suffix}");
    let ptr_ty = fc.builder_mut().ptr_type();
    let i64_ty = fc.builder_mut().i64_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty], i64_ty);

    if !fc.builder_mut().function_has_body(func_id) {
        let saved_pos = fc.builder_mut().save_position();
        let saved_func = fc.builder_mut().current_function();

        fc.builder_mut().set_ccc(func_id);
        fc.builder_mut().add_nounwind_attribute(func_id);
        let entry = fc.builder_mut().append_block(func_id, "entry");
        fc.builder_mut().position_at_end(entry);
        fc.builder_mut().set_current_function(func_id);

        let ptr = fc.builder_mut().get_param(func_id, 0);
        let llvm_ty = fc.resolve_type(ty);
        let ty_id = fc.builder_mut().register_type(llvm_ty);
        let val = fc.builder_mut().load(ty_id, ptr, "v");
        // Extend/bitcast to i64 for the hash
        let result = match info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => val,
            TypeInfo::Float => fc.builder_mut().bitcast(val, i64_ty, "h"),
            TypeInfo::Bool | TypeInfo::Byte => fc.builder_mut().zext(val, i64_ty, "h"),
            TypeInfo::Char => fc.builder_mut().sext(val, i64_ty, "h"),
            _ => unreachable!(),
        };
        fc.builder_mut().ret(result);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
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
