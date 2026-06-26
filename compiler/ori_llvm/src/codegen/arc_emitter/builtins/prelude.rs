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
pub(crate) const HANDLED_PRELUDE_NAMES: &[&str] =
    &["byte", "float", "hash_combine", "int", "repeat", "str"];

/// Prelude functions recognized but not yet implementable in AOT codegen.
///
/// These need runtime infrastructure (iterators, error construction, thread
/// locals) that isn't available in the AOT pipeline yet.
#[allow(
    dead_code,
    reason = "documents pending prelude AOT support for sync completeness"
)]
pub(crate) const PENDING_PRELUDE_NAMES: &[&str] = &["Error", "thread_id"];

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
        "repeat" => emit_repeat(emitter, args, arc_func),
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
    let arg_val = emitter.var(args[0]);

    // Prefer Printable (to_str) semantics — str() is the Printable conversion.
    // Fall back to Debug for compound types where to_str codegen is missing.
    emitter
        .emit_element_to_str(arg_val, arg_ty)
        .or_else(|| emitter.emit_element_debug(arg_val, arg_ty))
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
        // Checked conversion — panics on NaN / infinity / out-of-range
        // (parity with eval's `function_val_int`).
        TypeInfo::Float => emitter.emit_checked_float_to_int(arg_val, "int_cast"),
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

/// Emit `byte(value)` — range-checked conversions (parity with the
/// interpreter's `function_val_byte`: int errors outside 0..=255, char
/// errors above codepoint 255; silent truncation diverges on those inputs).
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
        TypeInfo::Int => emitter.emit_checked_int_to_byte(arg_val, "byte_cast"),
        TypeInfo::Char => {
            // Codepoints above 255 do not fit a byte — the interpreter
            // errors via u8::try_from on the codepoint (Latin-1 accepted).
            let byte_limit = emitter.builder.const_i32(256);
            let valid = emitter.builder.icmp_ult(arg_val, byte_limit, "byte.fits");
            emitter.emit_unwrap_branch(valid, "out of byte range (0-255)", "byte.char")?;
            Some(emitter.builder.trunc(arg_val, i8_ty, "byte_cast"))
        }
        _ => None,
    }
}

/// Emit `repeat(value)` — construct an infinite iterator over copies of `value`.
///
/// Calls `ori_iter_repeat(value_ptr, elem_size, elem_dec_fn)`. The runtime
/// memcpy's the value into a master buffer; because that creates a reference
/// the iterator holds for its whole lifetime (it may outlive the borrowed
/// source binding), the value's RC is incremented here before the copy —
/// mirroring `emit_option_iter`. The master's reference is released by the
/// runtime's `Drop for IterState` via `elem_dec_fn`; each yielded element is
/// inc'd by the consumer (e.g. `collect`).
fn emit_repeat<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    args: &[ArcVarId],
    arc_func: &ArcFunction,
) -> Option<ValueId> {
    // The free prelude `repeat(value:) -> Iterator<T>` takes EXACTLY one arg.
    // The string method `str.repeat(count:) -> str` lowers to the same
    // `Apply @repeat(self, count)` callee name with TWO args; decline it here so
    // it falls through to the string-method emitter (name-only prelude dispatch
    // would otherwise shadow the method).
    if args.len() != 1 {
        return None;
    }
    let value_ty = arc_func.var_type(args[0]);
    let value_val = emitter.var(args[0]);

    // RC-retain the value: the runtime memcpy's the value bytes into the
    // master buffer, creating a second reference to any inner RC-managed data.
    // The master outlives the (borrowed) source, so without this retain both
    // the source and the iterator master would dec the same RC → double-free.
    emitter.inc_value_rc(value_val, value_ty, 1);

    // Store the value to an alloca so the runtime can read it through a pointer.
    let value_llvm = emitter.resolve_type(value_ty);
    let alloca = emitter.builder.alloca(value_llvm, "repeat.val.a");
    emitter.builder.store(value_val, alloca);

    let elem_size =
        crate::codegen::abi::abi_size(value_ty, emitter.type_info, emitter.repr_plan) as i64;
    let elem_size_val = emitter.builder.const_i64(elem_size);

    // elem_dec_fn releases the master's reference at iterator drop (null for
    // scalar elements).
    let elem_dec_fn = emitter.get_or_generate_elem_dec_fn(value_ty);

    let func_id = emitter.builder.runtime_fn("ori_iter_repeat");
    emitter.emit_rt_call(
        func_id,
        &[alloca, elem_size_val, elem_dec_fn],
        "repeat.iter",
    )
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
