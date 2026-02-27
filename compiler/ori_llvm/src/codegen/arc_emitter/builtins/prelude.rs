//! Prelude builtin function codegen.
//!
//! Handles free functions registered in the evaluator's `register_prelude()`
//! that have no canonical IR body. Each function dispatches on argument types
//! to emit inline LLVM IR or runtime calls.
//!
//! This module is the unified replacement for scattered special-case
//! interceptions (e.g., `hash_combine`) in `emit_invoke()` and `emit_apply()`.

use ori_arc::ir::{ArcFunction, ArcVarId};

use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

/// Prelude functions with complete AOT codegen (sorted).
///
/// Must stay in sync with `try_emit_prelude_function()` match arms and
/// `ori_eval/src/interpreter/mod.rs::register_prelude()`.
#[allow(
    dead_code,
    reason = "used by sync tests for prelude coverage verification"
)]
pub(crate) const HANDLED_PRELUDE_NAMES: &[&str] = &["byte", "float", "hash_combine", "int", "str"];

/// Prelude functions recognized but not yet implementable in AOT codegen.
///
/// These need runtime infrastructure (iterators, error construction, thread
/// locals) that isn't available in the AOT pipeline yet.
#[allow(
    dead_code,
    reason = "documents pending prelude AOT support for sync completeness"
)]
pub(crate) const PENDING_PRELUDE_NAMES: &[&str] = &["Error", "repeat", "thread_id"];

/// Try to emit a prelude builtin function call.
///
/// Returns `Some(result)` if the function was handled, `None` if the caller
/// should fall through to the normal dispatch chain.
pub(in crate::codegen::arc_emitter) fn try_emit_prelude_function<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    callee_name: &str,
    args: &[ArcVarId],
    arc_func: &ArcFunction,
) -> Option<ValueId> {
    match callee_name {
        "str" => emit_str(emitter, args, arc_func),
        "int" => emit_int(emitter, args, arc_func),
        "float" => emit_float(emitter, args, arc_func),
        "byte" => emit_byte(emitter, args, arc_func),
        "hash_combine" => emit_hash_combine(emitter, args),
        // Not yet implementable — need runtime infrastructure.
        _ => None,
    }
}

/// Emit `str(value)` — dispatch on argument type to `ori_str_from_*` runtime.
fn emit_str<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    args: &[ArcVarId],
    arc_func: &ArcFunction,
) -> Option<ValueId> {
    if args.is_empty() {
        return None;
    }
    let arg_ty = arc_func.var_type(args[0]);
    let type_info = emitter.type_info.get(arg_ty);
    let arg_val = emitter.var(args[0]);

    match &type_info {
        // str(str_val) → identity
        TypeInfo::Str => Some(arg_val),
        // Delegate to emit_to_str for primitive types
        TypeInfo::Int | TypeInfo::Float | TypeInfo::Bool | TypeInfo::Duration | TypeInfo::Size => {
            emitter.emit_to_str(arg_val, &type_info)
        }
        // Other types (struct, enum, etc.) need derived Printable::to_str — deferred.
        _ => None,
    }
}

/// Emit `int(value)` — inline LLVM casts.
fn emit_int<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    args: &[ArcVarId],
    arc_func: &ArcFunction,
) -> Option<ValueId> {
    if args.is_empty() {
        return None;
    }
    let arg_ty = arc_func.var_type(args[0]);
    let type_info = emitter.type_info.get(arg_ty);
    let arg_val = emitter.var(args[0]);
    let i64_ty = emitter.builder.i64_type();

    match &type_info {
        TypeInfo::Int => Some(arg_val),
        TypeInfo::Float => Some(emitter.builder.fp_to_si(arg_val, i64_ty, "int_cast")),
        TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Char => {
            Some(emitter.builder.zext(arg_val, i64_ty, "int_cast"))
        }
        // int(str) needs runtime ori_int_from_str — deferred.
        _ => None,
    }
}

/// Emit `float(value)` — inline LLVM casts.
fn emit_float<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    args: &[ArcVarId],
    arc_func: &ArcFunction,
) -> Option<ValueId> {
    if args.is_empty() {
        return None;
    }
    let arg_ty = arc_func.var_type(args[0]);
    let type_info = emitter.type_info.get(arg_ty);
    let arg_val = emitter.var(args[0]);
    let f64_ty = emitter.builder.f64_type();

    match &type_info {
        TypeInfo::Float => Some(arg_val),
        TypeInfo::Int => Some(emitter.builder.si_to_fp(arg_val, f64_ty, "float_cast")),
        // float(str) needs runtime ori_float_from_str — deferred.
        _ => None,
    }
}

/// Emit `byte(value)` — inline LLVM casts.
fn emit_byte<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    args: &[ArcVarId],
    arc_func: &ArcFunction,
) -> Option<ValueId> {
    if args.is_empty() {
        return None;
    }
    let arg_ty = arc_func.var_type(args[0]);
    let type_info = emitter.type_info.get(arg_ty);
    let arg_val = emitter.var(args[0]);
    let i8_ty = emitter.builder.i8_type();

    match &type_info {
        TypeInfo::Byte => Some(arg_val),
        TypeInfo::Int | TypeInfo::Char => Some(emitter.builder.trunc(arg_val, i8_ty, "byte_cast")),
        _ => None,
    }
}

/// Emit `hash_combine(a, b)` — delegates to the existing inline emitter.
fn emit_hash_combine<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    args: &[ArcVarId],
) -> Option<ValueId> {
    if args.len() < 2 {
        return None;
    }
    let a = emitter.var(args[0]);
    let b = emitter.var(args[1]);
    Some(emitter.emit_hash_combine(a, b))
}
