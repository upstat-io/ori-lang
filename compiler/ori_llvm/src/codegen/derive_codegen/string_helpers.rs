//! String emission helpers for derived Printable codegen.
//!
//! Provides LLVM IR generation for string literals, concatenation, and
//! field-to-string conversion. Used by `compile_derive_printable()` to build
//! `"TypeName(val1, val2)"` representations.

use ori_types::Idx;

use super::super::function_compiler::FunctionCompiler;
use super::super::type_info::TypeInfo;
use super::super::value_id::{LLVMTypeId, ValueId};
use super::emit_method_call_for_derive;

/// Emit a string literal as an Ori str value `{i64 len, ptr data}`.
pub(super) fn emit_str_literal<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    s: &str,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let data_ptr = fc
        .builder_mut()
        .build_global_string_ptr(s, &format!("{name}.data"));
    let len = fc.builder_mut().const_i64(s.len() as i64);
    fc.builder_mut()
        .build_struct(str_ty_id, &[len, data_ptr], name)
}

/// Call `ori_str_concat(a: ptr, b: ptr) -> str` (alloca+store pattern).
pub(super) fn emit_str_concat<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();

    let lhs_alloca = fc.builder_mut().alloca(str_ty_id, &format!("{name}.lhs"));
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.builder_mut().alloca(str_ty_id, &format!("{name}.rhs"));
    fc.builder_mut().store(rhs, rhs_alloca);

    let concat_fn =
        fc.builder_mut()
            .get_or_declare_function("ori_str_concat", &[ptr_ty, ptr_ty], str_ty_id);
    fc.builder_mut()
        .call(concat_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| emit_str_literal(fc, "", "empty", str_ty_id))
}

/// Convert a field value to its string representation.
pub(super) fn emit_field_to_string<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
            let i64_ty = fc.builder_mut().i64_type();
            let f =
                fc.builder_mut()
                    .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty_id);
            fc.builder_mut()
                .call(f, &[val], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<int>", name, str_ty_id))
        }
        TypeInfo::Float => {
            let f64_ty = fc.builder_mut().f64_type();
            let f = fc.builder_mut().get_or_declare_function(
                "ori_str_from_float",
                &[f64_ty],
                str_ty_id,
            );
            fc.builder_mut()
                .call(f, &[val], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<float>", name, str_ty_id))
        }
        TypeInfo::Bool => {
            let bool_ty = fc.builder_mut().bool_type();
            let f = fc.builder_mut().get_or_declare_function(
                "ori_str_from_bool",
                &[bool_ty],
                str_ty_id,
            );
            fc.builder_mut()
                .call(f, &[val], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<bool>", name, str_ty_id))
        }
        TypeInfo::Str => val,
        TypeInfo::Char => {
            let i64_ty = fc.builder_mut().i64_type();
            let char_as_i64 = fc.builder_mut().sext(val, i64_ty, &format!("{name}.sext"));
            let f =
                fc.builder_mut()
                    .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty_id);
            fc.builder_mut()
                .call(f, &[char_as_i64], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<char>", name, str_ty_id))
        }
        TypeInfo::Byte | TypeInfo::Ordering => {
            let i64_ty = fc.builder_mut().i64_type();
            let as_i64 = fc.builder_mut().sext(val, i64_ty, &format!("{name}.sext"));
            let f =
                fc.builder_mut()
                    .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty_id);
            fc.builder_mut()
                .call(f, &[as_i64], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<byte>", name, str_ty_id))
        }
        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let ts_name = fc.intern("to_str");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, ts_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[val], name);
                }
            }
            emit_str_literal(fc, "<struct>", name, str_ty_id)
        }
        _ => emit_str_literal(fc, "<?>", name, str_ty_id),
    }
}
