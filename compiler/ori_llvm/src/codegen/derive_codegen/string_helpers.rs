//! String emission helpers for derived format codegen (Printable & Debug).
//!
//! Provides LLVM IR generation for string literals, concatenation, and
//! field-to-string conversion. Used by `compile_format_fields()` to build
//! formatted representations like `"TypeName(val1, val2)"` (Printable) or
//! `"TypeName { f1: val1, f2: val2 }"` (Debug).

use ori_ir::DerivedTrait;
use ori_types::Idx;

use super::super::function_compiler::FunctionCompiler;
use super::super::type_info::TypeInfo;
use super::super::value_id::{LLVMTypeId, ValueId};
use super::{emit_boxed_self_method_call, emit_method_call_for_derive};

/// Emit a string literal as an Ori str value via `ori_str_from_raw`.
///
/// Uses the runtime to create a proper SSO string (≤ 23 bytes inline)
/// or RC-managed heap string (> 23 bytes). This avoids creating
/// `{len, cap, global_ptr}` structs whose data pointer lacks an RC header,
/// which would be UB if passed to COW operations like `ori_str_concat`.
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
    let from_raw = fc.builder_mut().runtime_fn("ori_str_from_raw");
    fc.builder_mut()
        .call_with_sret(from_raw, &[data_ptr, len], str_ty_id, name)
        .unwrap_or_else(|| {
            // Fallback for JIT path where sret may be unavailable
            let cap = fc.builder_mut().const_i64(s.len() as i64);
            fc.builder_mut()
                .build_struct(str_ty_id, &[len, cap, data_ptr], name)
        })
}

/// Emit `ori_str_rc_dec(data, cap, null)` on a string value.
///
/// Handles SSO strings (no-op — cap field encodes SSO flag), heap strings
/// (decrements RC on the data buffer), and seamless slices (finds original
/// buffer). The `drop_fn` is null because strings contain no nested references.
///
/// Used by `compile_format_fields` to clean up intermediate concatenation results
/// that are overwritten and become unreachable. Without these calls, every
/// intermediate string in the format loop leaks.
pub(super) fn emit_str_rc_dec<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    str_val: ValueId,
    name: &str,
) {
    let data = fc
        .builder_mut()
        .extract_value(str_val, 2, &format!("{name}.data"));
    let cap = fc
        .builder_mut()
        .extract_value(str_val, 1, &format!("{name}.cap"));
    if let (Some(dp), Some(cp)) = (data, cap) {
        let drop_fn_id = fc.builder_mut().runtime_fn("ori_str_drop_buffer");
        let drop_fn_ptr = fc.builder_mut().get_function_ptr(drop_fn_id);
        let dec_fn = fc.builder_mut().runtime_fn("ori_str_rc_dec");
        fc.builder_mut().call(dec_fn, &[dp, cp, drop_fn_ptr], "");
    }
}

/// Call `ori_str_concat(a: ptr, b: ptr) -> str` (alloca+store pattern).
pub(super) fn emit_str_concat<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let lhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.lhs"));
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.rhs"));
    fc.builder_mut().store(rhs, rhs_alloca);

    let concat_fn = fc.builder_mut().runtime_fn("ori_str_concat");
    fc.builder_mut()
        .call_with_sret(concat_fn, &[lhs_alloca, rhs_alloca], str_ty_id, name)
        .unwrap_or_else(|| emit_str_literal(fc, "", "empty", str_ty_id))
}

/// Convert a field value to its string representation.
///
/// The `trait_kind` parameter determines which method to call on nested struct
/// types (e.g., `to_str` for Printable, `debug` for Debug) and whether to
/// quote string values (Debug quotes, Printable doesn't).
///
/// `boxed` is true when the field position is a boxed recursive back-edge per
/// `repr_box_oracle` — `val` is then an RC `ptr` to the heap value, passed
/// directly as the nested method's self argument; otherwise `val` is the inline
/// field value.
pub(super) fn emit_field_to_string<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    val: ValueId,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
    boxed: bool,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
            // Integer narrowing may produce i8/i16/i32 struct fields —
            // sext back to canonical i64 for the runtime formatting call.
            // Duration/Size carry units; route to their unit-aware formatters.
            let widened = fc
                .builder_mut()
                .sext_to_i64_if_narrower(val, &format!("{name}.sext"));
            let (rt_name, fallback) = match &info {
                TypeInfo::Duration => ("ori_str_from_duration", "<duration>"),
                TypeInfo::Size => ("ori_str_from_size", "<size>"),
                _ => ("ori_str_from_int", "<int>"),
            };
            let f = fc.builder_mut().runtime_fn(rt_name);
            fc.builder_mut()
                .call_with_sret(f, &[widened], str_ty_id, name)
                .unwrap_or_else(|| emit_str_literal(fc, fallback, name, str_ty_id))
        }
        TypeInfo::Float => {
            // Float narrowing may produce f32 struct fields —
            // fpext back to canonical f64 for the runtime formatting call.
            let widened = fc
                .builder_mut()
                .fpext_to_f64_if_narrower(val, &format!("{name}.fpext"));
            let f = fc.builder_mut().runtime_fn("ori_str_from_float");
            fc.builder_mut()
                .call_with_sret(f, &[widened], str_ty_id, name)
                .unwrap_or_else(|| emit_str_literal(fc, "<float>", name, str_ty_id))
        }
        TypeInfo::Bool => {
            let f = fc.builder_mut().runtime_fn("ori_str_from_bool");
            fc.builder_mut()
                .call_with_sret(f, &[val], str_ty_id, name)
                .unwrap_or_else(|| emit_str_literal(fc, "<bool>", name, str_ty_id))
        }
        TypeInfo::Str => {
            if trait_kind == DerivedTrait::Debug {
                // Debug quotes string values: "hello" → "\"hello\""
                let open = emit_str_literal(fc, "\"", &format!("{name}.q1"), str_ty_id);
                let quoted = emit_str_concat(fc, open, val, &format!("{name}.qcat"), str_ty_id);
                // open is SSO (1 byte) — dec is a no-op but balances ownership.
                emit_str_rc_dec(fc, open, &format!("{name}.dec.q1"));
                let close = emit_str_literal(fc, "\"", &format!("{name}.q2"), str_ty_id);
                let result =
                    emit_str_concat(fc, quoted, close, &format!("{name}.quoted"), str_ty_id);
                // quoted is heap-backed for long strings — must dec to avoid leak.
                emit_str_rc_dec(fc, quoted, &format!("{name}.dec.qcat"));
                // close is SSO (1 byte) — dec is a no-op but balances ownership.
                emit_str_rc_dec(fc, close, &format!("{name}.dec.q2"));
                result
            } else {
                val
            }
        }
        // Why: a char field renders as its character (Printable) or single-
        // quoted+escaped (Debug) to match the evaluator and the element path —
        // NOT the raw i32 codepoint. The runtime takes the i32 char directly.
        TypeInfo::Char => {
            let rt_name = if trait_kind == DerivedTrait::Debug {
                "ori_char_debug_format"
            } else {
                "ori_str_from_char"
            };
            let f = fc.builder_mut().runtime_fn(rt_name);
            fc.builder_mut()
                .call_with_sret(f, &[val], str_ty_id, name)
                .unwrap_or_else(|| emit_str_literal(fc, "<char>", name, str_ty_id))
        }
        // Why: a byte field renders as hex (`0x41`) for BOTH Printable and Debug
        // to match the evaluator — NOT the raw decimal. `ori_byte_debug_format`
        // masks to the u8 range, so the sext is harmless.
        TypeInfo::Byte => {
            let i64_ty = fc.builder_mut().i64_type();
            let as_i64 = fc.builder_mut().sext(val, i64_ty, &format!("{name}.sext"));
            let f = fc.builder_mut().runtime_fn("ori_byte_debug_format");
            fc.builder_mut()
                .call_with_sret(f, &[as_i64], str_ty_id, name)
                .unwrap_or_else(|| emit_str_literal(fc, "<byte>", name, str_ty_id))
        }
        // An `Ordering`-typed field renders its variant name (Less/Equal/Greater),
        // matching the interpreter — NOT the raw i8 tag.
        TypeInfo::Ordering => {
            let i64_ty = fc.builder_mut().i64_type();
            let tag_i64 = fc.builder_mut().sext(val, i64_ty, &format!("{name}.sext"));
            let f = fc.builder_mut().runtime_fn("ori_str_from_ordering");
            fc.builder_mut()
                .call_with_sret(f, &[tag_i64], str_ty_id, name)
                .unwrap_or_else(|| emit_str_literal(fc, "<ordering>", name, str_ty_id))
        }
        // A nested struct/enum field renders via its own derived `to_str`/`debug`
        // method. A boxed recursive back-edge (`Tree` inside `Node(left, right)`)
        // arrives as a `ptr` to the heap value and is passed directly; an inline
        // field value is stored to an alloca by the call helper.
        TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => {
            let method = fc.intern(trait_kind.method_name());
            if let Some((fid, abi)) = fc.get_derived_method_for_type(field_type, method) {
                if boxed {
                    return emit_boxed_self_method_call(fc, fid, &abi, val, name);
                }
                return emit_method_call_for_derive(fc, fid, &abi, &[val], name);
            }
            emit_str_literal(fc, "<composite>", name, str_ty_id)
        }
        _ => emit_str_literal(fc, "<?>", name, str_ty_id),
    }
}
