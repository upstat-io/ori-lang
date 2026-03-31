//! Thunk generator functions for derived trait callbacks.
//!
//! Generates small `(ptr, ptr) -> bool` or `(ptr) -> i64` thunk functions
//! used as callbacks by runtime list/map comparison routines. Each thunk
//! loads values from pointers and delegates to the appropriate comparison
//! or hash logic.

use ori_types::Idx;

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::{compute_elem_size, needs_deep_comparison};

/// Get a function pointer to an equality thunk for use in map/list comparison.
///
/// Supported types:
/// - `str`: returns `ori_str_eq`
/// - Primitives (`int`, `float`, `bool`, `char`, `byte`, `duration`, `size`):
///   generates `_ori_eq_{type}` thunks using `icmp`/`fcmp`
/// - `List`/`Set`: generates thunks that call `ori_list_eq_scalar` (scalar elements)
///   or `ori_list_eq_deep` (non-scalar elements like str)
/// - `Struct`/`Enum` with derived Eq: generates thunks that call the compiled
///   `eq` method
pub(super) fn get_or_create_derive_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    info: &TypeInfo,
) -> Option<ValueId> {
    if matches!(info, TypeInfo::Str) {
        let func_id = fc.builder_mut().runtime_fn("ori_str_eq");
        return Some(fc.builder_mut().get_function_ptr(func_id));
    }

    // List/Set: generate thunk calling ori_list_eq_scalar or ori_list_eq_deep
    if let TypeInfo::List { element } | TypeInfo::Set { element } = info {
        return get_or_create_list_eq_thunk(fc, ty, *element);
    }

    // Option/Result/Tuple: generate structural equality thunks
    if let TypeInfo::Option { inner } = info {
        return get_or_create_option_eq_thunk(fc, ty, *inner);
    }
    if let TypeInfo::Result { ok, err } = info {
        return get_or_create_result_eq_thunk(fc, ty, *ok, *err);
    }
    if let TypeInfo::Tuple { elements } = info {
        return get_or_create_tuple_eq_thunk(fc, ty, &elements.clone());
    }

    // Struct/Enum: generate thunk calling compiled derived eq method
    if matches!(info, TypeInfo::Struct { .. } | TypeInfo::Enum { .. }) {
        return get_or_create_user_type_eq_thunk(fc, ty);
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

/// Generate a thunk `(ptr, ptr) -> bool` that compares two list/set values.
///
/// For scalar elements, calls `ori_list_eq_scalar`. For non-scalar elements
/// (str, nested collections), calls `ori_list_eq_deep` with an inner eq thunk.
fn get_or_create_list_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    _list_ty: Idx,
    element_ty: Idx,
) -> Option<ValueId> {
    let elem_info = fc.type_info().get(element_ty);
    let use_deep = needs_deep_comparison(&elem_info);
    let elem_size = compute_elem_size(fc, element_ty, &elem_info);

    // For deep comparison, we need an inner thunk for element equality
    let inner_thunk = if use_deep {
        get_or_create_derive_eq_thunk(fc, element_ty, &elem_info)?
    } else {
        // Not used — just a placeholder
        fc.builder_mut().const_null_ptr()
    };

    let func_name = format!(
        "_ori_eq_list_e{elem_size}_{}",
        if use_deep { "deep" } else { "scalar" }
    );
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

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
        let elem_size_val = fc.builder_mut().const_i64(elem_size);

        let result = if use_deep {
            let eq_fn = fc.builder_mut().runtime_fn("ori_list_eq_deep");
            fc.builder_mut()
                .call(eq_fn, &[a_ptr, b_ptr, elem_size_val, inner_thunk], "eq")
                .unwrap_or_else(|| fc.builder_mut().const_bool(false))
        } else {
            let eq_fn = fc.builder_mut().runtime_fn("ori_list_eq_scalar");
            fc.builder_mut()
                .call(eq_fn, &[a_ptr, b_ptr, elem_size_val], "eq")
                .unwrap_or_else(|| fc.builder_mut().const_bool(false))
        };
        fc.builder_mut().ret(result);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}

/// Generate a thunk `(ptr, ptr) -> bool` for Option<T> equality.
///
/// Loads the tag (i64 at offset 0) from each pointer, compares tags,
/// then if both Some, compares payloads at offset 8 using the inner eq thunk.
fn get_or_create_option_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    inner_ty: Idx,
) -> Option<ValueId> {
    let inner_info = fc.type_info().get(inner_ty);
    let inner_eq_thunk = get_or_create_derive_eq_thunk(fc, inner_ty, &inner_info)?;

    let func_name = format!("_ori_eq_option_{}", ty.raw());
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

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

        // Load tags (i64 at offset 0)
        let i64_ty_id = fc.builder_mut().i64_type();
        let a_tag = fc.builder_mut().load(i64_ty_id, a_ptr, "a_tag");
        let b_tag = fc.builder_mut().load(i64_ty_id, b_ptr, "b_tag");
        let tags_eq = fc.builder_mut().icmp_eq(a_tag, b_tag, "tags_eq");

        // Payload pointers at byte offset 8 (after i64 tag)
        let i8_ty_id = fc.builder_mut().i8_type();
        let offset_8 = fc.builder_mut().const_i64(8);
        let a_payload = fc.builder_mut().gep(i8_ty_id, a_ptr, &[offset_8], "a_pay");
        let b_payload = fc.builder_mut().gep(i8_ty_id, b_ptr, &[offset_8], "b_pay");

        // Compare payloads using inner thunk (indirect call through fn ptr)
        let bool_ty_id = fc.builder_mut().bool_type();
        let payload_eq = fc
            .builder_mut()
            .call_indirect(
                bool_ty_id,
                &[ptr_ty, ptr_ty],
                inner_eq_thunk,
                &[a_payload, b_payload],
                "pay_eq",
            )
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));

        // None tag = 1: both None -> true. Some tag = 0: compare payloads.
        let one = fc.builder_mut().const_i64(1);
        let is_none = fc.builder_mut().icmp_eq(a_tag, one, "is_none");
        let true_val = fc.builder_mut().const_bool(true);
        let same_result = fc
            .builder_mut()
            .select(is_none, true_val, payload_eq, "same_eq");
        let false_val = fc.builder_mut().const_bool(false);
        let result = fc
            .builder_mut()
            .select(tags_eq, same_result, false_val, "eq");
        fc.builder_mut().ret(result);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}

/// Generate a thunk `(ptr, ptr) -> bool` for Result<T, E> equality.
///
/// Loads the tag (i64 at offset 0) from each pointer, compares tags,
/// then if same variant, compares payloads using Ok or Err eq thunk.
fn get_or_create_result_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    ok_ty: Idx,
    err_ty: Idx,
) -> Option<ValueId> {
    let ok_info = fc.type_info().get(ok_ty);
    let err_info = fc.type_info().get(err_ty);
    let ok_eq = get_or_create_derive_eq_thunk(fc, ok_ty, &ok_info)?;
    let err_eq = get_or_create_derive_eq_thunk(fc, err_ty, &err_info)?;

    let func_name = format!("_ori_eq_result_{}", ty.raw());
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

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

        // Load tags
        let i64_ty_id = fc.builder_mut().i64_type();
        let a_tag = fc.builder_mut().load(i64_ty_id, a_ptr, "a_tag");
        let b_tag = fc.builder_mut().load(i64_ty_id, b_ptr, "b_tag");
        let tags_eq = fc.builder_mut().icmp_eq(a_tag, b_tag, "tags_eq");

        // Payload pointers at byte offset 8
        let i8_ty_id = fc.builder_mut().i8_type();
        let offset_8 = fc.builder_mut().const_i64(8);
        let a_payload = fc.builder_mut().gep(i8_ty_id, a_ptr, &[offset_8], "a_pay");
        let b_payload = fc.builder_mut().gep(i8_ty_id, b_ptr, &[offset_8], "b_pay");

        // Compare as Ok (always evaluate both; branchless)
        let bool_ty_id = fc.builder_mut().bool_type();
        let ok_result = fc
            .builder_mut()
            .call_indirect(
                bool_ty_id,
                &[ptr_ty, ptr_ty],
                ok_eq,
                &[a_payload, b_payload],
                "ok_eq",
            )
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));
        // Compare as Err
        let err_result = fc
            .builder_mut()
            .call_indirect(
                bool_ty_id,
                &[ptr_ty, ptr_ty],
                err_eq,
                &[a_payload, b_payload],
                "err_eq",
            )
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));

        // Ok tag = 0, Err tag = 1
        let zero = fc.builder_mut().const_i64(0);
        let is_ok = fc.builder_mut().icmp_eq(a_tag, zero, "is_ok");
        let same_eq = fc
            .builder_mut()
            .select(is_ok, ok_result, err_result, "same_eq");
        let false_val = fc.builder_mut().const_bool(false);
        let result = fc.builder_mut().select(tags_eq, same_eq, false_val, "eq");
        fc.builder_mut().ret(result);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}

/// Generate a thunk `(ptr, ptr) -> bool` for Tuple<A, B, ...> equality.
///
/// Loads each field from each pointer using struct GEP, compares
/// field-by-field using per-field eq thunks.
fn get_or_create_tuple_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    elements: &[Idx],
) -> Option<ValueId> {
    // Pre-generate all element eq thunks
    let mut elem_thunks = Vec::with_capacity(elements.len());
    for &elem_ty in elements {
        let elem_info = fc.type_info().get(elem_ty);
        elem_thunks.push(get_or_create_derive_eq_thunk(fc, elem_ty, &elem_info)?);
    }

    let func_name = format!("_ori_eq_tuple_{}", ty.raw());
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

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

        // Compare field-by-field using struct_gep + indirect calls
        let llvm_ty = fc.resolve_type(ty);
        let ty_id = fc.builder_mut().register_type(llvm_ty);
        let bool_ty_id = fc.builder_mut().bool_type();

        let mut result = fc.builder_mut().const_bool(true);
        for (i, &thunk) in elem_thunks.iter().enumerate() {
            // §06: remap declaration-order index to memory order.
            let mem_i = fc
                .repr_plan()
                .and_then(|plan| {
                    let resolved = fc.pool().resolve_fully(ty);
                    let repr = plan.get_repr(resolved)?;
                    let fields = match repr {
                        ori_repr::MachineRepr::Tuple(t) => &t.elements[..],
                        ori_repr::MachineRepr::Struct(s) => &s.fields[..],
                        _ => return None,
                    };
                    #[expect(clippy::cast_possible_truncation, reason = "field index fits u32")]
                    fields
                        .iter()
                        .position(|f| f.original_index == i as u32)
                        .map(|p| p as u32)
                })
                .unwrap_or(i as u32);
            // Get field pointers using struct_gep
            let a_field_ptr = fc
                .builder_mut()
                .struct_gep(ty_id, a_ptr, mem_i, &format!("a_f{i}"));
            let b_field_ptr = fc
                .builder_mut()
                .struct_gep(ty_id, b_ptr, mem_i, &format!("b_f{i}"));
            // Call eq thunk on field pointers (indirect call through fn ptr)
            let field_eq = fc
                .builder_mut()
                .call_indirect(
                    bool_ty_id,
                    &[ptr_ty, ptr_ty],
                    thunk,
                    &[a_field_ptr, b_field_ptr],
                    &format!("f{i}_eq"),
                )
                .unwrap_or_else(|| fc.builder_mut().const_bool(false));
            result = fc.builder_mut().and(result, field_eq, &format!("acc_{i}"));
        }

        fc.builder_mut().ret(result);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}

/// Generate a thunk `(ptr, ptr) -> bool` that calls a compiled derived eq method.
fn get_or_create_user_type_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
) -> Option<ValueId> {
    let type_name = fc.type_idx_to_name(ty)?;
    let method = fc.intern("eq");
    let (method_fid, _abi) = fc.get_method_function(type_name, method)?;

    let func_name = format!("_ori_eq_struct_{}", fc.lookup_name(type_name));
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

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

        // Call the compiled eq method: (ptr, ptr) -> bool
        let result = fc
            .builder_mut()
            .call(method_fid, &[a_ptr, b_ptr], "eq")
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));
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
/// For composite types (List, Set, Struct, Enum), generates a constant-0 hash
/// thunk — correct (a == b implies hash(a) == hash(b)) but causes O(n) lookup.
pub(super) fn get_or_create_derive_hash_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    info: &TypeInfo,
) -> Option<ValueId> {
    if matches!(info, TypeInfo::Str) {
        let func_id = fc.builder_mut().runtime_fn("ori_str_hash");
        return Some(fc.builder_mut().get_function_ptr(func_id));
    }

    // For composite types used as map keys, generate a constant-0 hash thunk.
    // This is correct (all equal values hash the same) but degrades to O(n) lookup.
    // Composite map keys are rare; this avoids recursive hash thunk generation.
    if matches!(
        info,
        TypeInfo::List { .. }
            | TypeInfo::Set { .. }
            | TypeInfo::Map { .. }
            | TypeInfo::Struct { .. }
            | TypeInfo::Enum { .. }
            | TypeInfo::Option { .. }
            | TypeInfo::Result { .. }
            | TypeInfo::Tuple { .. }
    ) {
        return get_or_create_constant_hash_thunk(fc);
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

/// Generate a constant-0 hash thunk `(ptr) -> i64` for composite map keys.
///
/// Always returns 0 — correct (equal values get equal hashes) but degrades
/// to O(n) bucket scan. Acceptable since composite map keys are rare.
fn get_or_create_constant_hash_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
) -> Option<ValueId> {
    let func_name = "_ori_hash_const_zero";
    let ptr_ty = fc.builder_mut().ptr_type();
    let i64_ty = fc.builder_mut().i64_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(func_name, &[ptr_ty], i64_ty);

    if !fc.builder_mut().function_has_body(func_id) {
        let saved_pos = fc.builder_mut().save_position();
        let saved_func = fc.builder_mut().current_function();

        fc.builder_mut().set_ccc(func_id);
        fc.builder_mut().add_nounwind_attribute(func_id);
        let entry = fc.builder_mut().append_block(func_id, "entry");
        fc.builder_mut().position_at_end(entry);
        fc.builder_mut().set_current_function(func_id);

        let zero = fc.builder_mut().const_i64(0);
        fc.builder_mut().ret(zero);

        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}
