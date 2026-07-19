//! Runtime-function call helpers for derived collection/string field ops.
//!
//! `alloca+store+call` wrappers for `ori_str_*` / `ori_list_eq_*` / `ori_map_eq`,
//! plus element-size + deep-comparison helpers. Called by [`super`]'s field-op
//! dispatcher.

use ori_types::Idx;
use tracing::trace;

use super::super::super::arc_emitter::narrowed_collection_element_width;
use super::super::super::function_compiler::FunctionCompiler;
use super::super::super::type_info::TypeInfo;
use super::super::super::value_id::{LLVMTypeId, ValueId};
use super::thunks;

// String runtime helpers (alloca+store+call pattern)

/// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
pub(super) fn emit_str_eq_call<'a>(
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
pub(super) fn emit_str_compare_call<'a>(
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
pub(super) fn emit_str_hash_call<'a>(
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

/// Emit list equality via the appropriate runtime function: `ori_list_eq_scalar`
/// (byte-level memcmp) for scalar elements; `ori_list_eq_deep` with a per-element
/// equality callback for non-scalar elements (memcmp fails when equal heap values
/// hold different data pointers).
pub(super) fn emit_list_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    collection_type: Idx,
    element_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(element_type);
    let elem_size = compute_elem_size(fc, collection_type, element_type, &info);

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
pub(super) fn emit_map_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    collection_type: Idx,
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

    let key_size = compute_elem_size(fc, collection_type, key_type, &key_info);
    let val_size = compute_elem_size(fc, collection_type, val_type, &val_info);
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
pub(super) fn needs_deep_comparison(info: &TypeInfo) -> bool {
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
///
/// `collection_idx` is the enclosing collection type whose backing buffer
/// stores elements of `ty` (e.g. the `[int]` / `{K: V}` type). For narrowed
/// int elements the stride is keyed on THAT collection's `ReprPlan` entry via
/// the [`narrowed_collection_element_width`] SSOT — never a `ReprPlan`-wide
/// scan, which conflates widths across distinct narrowed collections.
pub(super) fn compute_elem_size<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    collection_idx: Idx,
    ty: Idx,
    info: &TypeInfo,
) -> i64 {
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
            // Why: phase-C integer narrowing — an int element narrowed within THIS
            // collection's backing buffer has stride 1/2/4, not canonical 8; the
            // width is read from the specific `collection_idx` entry so two narrowed
            // int collections of different widths each get their own stride.
            let Some(plan) = fc.repr_plan() else { return 8 };
            if fc.pool().tag(fc.pool().resolve_fully(ty)) != ori_types::Tag::Int {
                return 8;
            }
            narrowed_collection_element_width(plan, fc.pool(), collection_idx)
                .map_or(8, |width| i64::from(width.size_bytes()))
        }
    }
}
