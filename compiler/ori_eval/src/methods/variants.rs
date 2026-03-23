//! Method dispatch for variant types (Option, Result, bool, char, byte, newtype).

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalError, EvalResult, Value};

use super::compare::{
    compare_option_values, compare_result_values, equals_values, hash_value, ordering_to_value,
};
use super::helpers::{
    debug_value, escape_debug_char, require_args, require_bool_arg, require_byte_arg,
    require_char_arg, require_scalar_int_arg,
};
use super::DispatchCtx;

/// Dispatch operator methods on bool values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_bool_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Bool(a) = receiver else {
        unreachable!("dispatch_bool_method called with non-bool receiver")
    };

    let n = ctx.names;

    if method == n.not {
        require_args("not", 0, args.len())?;
        Ok(Value::Bool(!a))
    // Comparable trait - false < true
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let b = require_bool_arg("compare", &args, 0)?;
        Ok(ordering_to_value(a.cmp(&b)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let b = require_bool_arg("equals", &args, 0)?;
        Ok(Value::Bool(a == b))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable and Debug traits
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(if a { "true" } else { "false" }))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(i64::from(a)))
    // Conversion: to_int
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        Ok(Value::int(i64::from(a)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "bool").into())
    }
}

/// Dispatch methods on char values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_char_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Char(c) = receiver else {
        unreachable!("dispatch_char_method called with non-char receiver")
    };

    let n = ctx.names;

    // Comparable trait - Unicode codepoint order
    if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_char_arg("compare", &args, 0)?;
        Ok(ordering_to_value(c.cmp(&other)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_char_arg("equals", &args, 0)?;
        Ok(Value::Bool(c == other))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable trait
    } else if method == n.to_str {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(c.to_string()))
    // Debug trait - shows escaped char with quotes
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(format!("'{}'", escape_debug_char(c))))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(i64::from(c as u32)))
    // Predicates
    } else if method == n.is_alpha {
        require_args("is_alpha", 0, args.len())?;
        Ok(Value::Bool(c.is_alphabetic()))
    } else if method == n.is_ascii {
        require_args("is_ascii", 0, args.len())?;
        Ok(Value::Bool(c.is_ascii()))
    } else if method == n.is_digit {
        require_args("is_digit", 0, args.len())?;
        Ok(Value::Bool(c.is_ascii_digit()))
    } else if method == n.is_lowercase {
        require_args("is_lowercase", 0, args.len())?;
        Ok(Value::Bool(c.is_lowercase()))
    } else if method == n.is_uppercase {
        require_args("is_uppercase", 0, args.len())?;
        Ok(Value::Bool(c.is_uppercase()))
    } else if method == n.is_whitespace {
        require_args("is_whitespace", 0, args.len())?;
        Ok(Value::Bool(c.is_whitespace()))
    // Conversions
    } else if method == n.to_byte {
        require_args("to_byte", 0, args.len())?;
        let code = c as u32;
        u8::try_from(code).map(Value::Byte).map_err(|_| {
            EvalError::new(format!(
                "char '{c}' (U+{code:04X}) cannot be converted to byte (> 127)"
            ))
            .into()
        })
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        Ok(Value::int(i64::from(c as u32)))
    } else if method == n.to_lowercase {
        require_args("to_lowercase", 0, args.len())?;
        let lower: String = c.to_lowercase().collect();
        let first = lower.chars().next().unwrap_or(c);
        Ok(Value::Char(first))
    } else if method == n.to_uppercase {
        require_args("to_uppercase", 0, args.len())?;
        let upper: String = c.to_uppercase().collect();
        let first = upper.chars().next().unwrap_or(c);
        Ok(Value::Char(first))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "char").into())
    }
}

/// Dispatch methods on byte values.
#[expect(clippy::too_many_lines, reason = "exhaustive byte method dispatch")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_byte_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Byte(b) = receiver else {
        unreachable!("dispatch_byte_method called with non-byte receiver")
    };

    let n = ctx.names;

    // Comparable trait - numeric order
    if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_byte_arg("compare", &args, 0)?;
        Ok(ordering_to_value(b.cmp(&other)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_byte_arg("equals", &args, 0)?;
        Ok(Value::Bool(b == other))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable and Debug traits
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(format!("0x{b:02x}")))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(i64::from(b)))
    // Arithmetic operators
    } else if method == n.add {
        require_args("add", 1, args.len())?;
        let other = require_byte_arg("add", &args, 0)?;
        b.checked_add(other)
            .map(Value::Byte)
            .ok_or_else(|| EvalError::new("byte addition overflow").into())
    } else if method == n.sub {
        require_args("sub", 1, args.len())?;
        let other = require_byte_arg("sub", &args, 0)?;
        b.checked_sub(other)
            .map(Value::Byte)
            .ok_or_else(|| EvalError::new("byte subtraction overflow").into())
    } else if method == n.mul {
        require_args("mul", 1, args.len())?;
        let other = require_byte_arg("mul", &args, 0)?;
        b.checked_mul(other)
            .map(Value::Byte)
            .ok_or_else(|| EvalError::new("byte multiplication overflow").into())
    } else if method == n.div {
        require_args("div", 1, args.len())?;
        let other = require_byte_arg("div", &args, 0)?;
        if other == 0 {
            Err(EvalError::new("division by zero").into())
        } else {
            // SAFETY(arithmetic): other != 0 verified above — u8 division cannot overflow
            #[expect(clippy::arithmetic_side_effects, reason = "divisor != 0 checked above")]
            Ok(Value::Byte(b / other))
        }
    } else if method == n.rem {
        require_args("rem", 1, args.len())?;
        let other = require_byte_arg("rem", &args, 0)?;
        if other == 0 {
            Err(EvalError::new("modulo by zero").into())
        } else {
            // SAFETY(arithmetic): other != 0 verified above — u8 remainder cannot overflow
            #[expect(clippy::arithmetic_side_effects, reason = "divisor != 0 checked above")]
            Ok(Value::Byte(b % other))
        }
    // Bitwise operators
    } else if method == n.bit_and {
        require_args("bit_and", 1, args.len())?;
        let other = require_byte_arg("bit_and", &args, 0)?;
        Ok(Value::Byte(b & other))
    } else if method == n.bit_or {
        require_args("bit_or", 1, args.len())?;
        let other = require_byte_arg("bit_or", &args, 0)?;
        Ok(Value::Byte(b | other))
    } else if method == n.bit_xor {
        require_args("bit_xor", 1, args.len())?;
        let other = require_byte_arg("bit_xor", &args, 0)?;
        Ok(Value::Byte(b ^ other))
    } else if method == n.bit_not {
        require_args("bit_not", 0, args.len())?;
        Ok(Value::Byte(!b))
    } else if method == n.shl {
        require_args("shl", 1, args.len())?;
        let shift = require_scalar_int_arg("shl", &args, 0)?;
        byte_shift_left(b, shift.raw())
    } else if method == n.shr {
        require_args("shr", 1, args.len())?;
        let shift = require_scalar_int_arg("shr", &args, 0)?;
        byte_shift_right(b, shift.raw())
    // Predicates
    } else if method == n.is_ascii {
        require_args("is_ascii", 0, args.len())?;
        Ok(Value::Bool(true))
    } else if method == n.is_ascii_alpha {
        require_args("is_ascii_alpha", 0, args.len())?;
        Ok(Value::Bool(b.is_ascii_alphabetic()))
    } else if method == n.is_ascii_digit {
        require_args("is_ascii_digit", 0, args.len())?;
        Ok(Value::Bool(b.is_ascii_digit()))
    } else if method == n.is_ascii_whitespace {
        require_args("is_ascii_whitespace", 0, args.len())?;
        Ok(Value::Bool(b.is_ascii_whitespace()))
    // Conversions
    } else if method == n.to_char {
        require_args("to_char", 0, args.len())?;
        Ok(Value::Char(char::from(b)))
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        Ok(Value::int(i64::from(b)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "byte").into())
    }
}

/// Dispatch methods on newtype values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_newtype_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Newtype { inner, .. } = receiver else {
        unreachable!("dispatch_newtype_method called with non-newtype value");
    };

    let n = ctx.names;

    if method == n.unwrap {
        require_args("unwrap", 0, args.len())?;
        Ok((*inner).clone())
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "newtype").into())
    }
}

/// Dispatch methods on Option values.
pub fn dispatch_option_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.unwrap || method == n.unwrap_or {
        // Both unwrap and unwrap_or return inner value for Some
        if let Value::Some(v) = &receiver {
            return Ok((**v).clone());
        }
        // None: unwrap errors, unwrap_or returns default
        if method == n.unwrap {
            return Err(EvalError::new("called unwrap on None").into());
        }
        require_args("unwrap_or", 1, args.len())?;
        match args.into_iter().next() {
            Some(default) => Ok(default),
            None => unreachable!("require_args verified length is 1"),
        }
    } else if method == n.is_some {
        Ok(Value::Bool(matches!(&receiver, Value::Some(_))))
    } else if method == n.is_none {
        Ok(Value::Bool(matches!(&receiver, Value::None)))
    // ok_or: Convert Option to Result
    } else if method == n.ok_or {
        require_args("ok_or", 1, args.len())?;
        match &receiver {
            Value::Some(v) => Ok(Value::ok((**v).clone())),
            _ => match args.into_iter().next() {
                Some(error) => Ok(Value::err(error)),
                None => unreachable!("require_args verified length is 1"),
            },
        }
    // Comparable trait - None < Some(_)
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let ord = compare_option_values(&receiver, &args[0], ctx.interner)?;
        Ok(ordering_to_value(ord))
    // Eq trait - deep equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - recursive hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&receiver, ctx.interner)?))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Iterable: Some(x) → 1-element list iterator, None → empty iterator
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        // from_value handles Some → 1-element and None → empty iterator
        match ori_patterns::IteratorValue::from_value(&receiver) {
            Some(iter) => Ok(Value::iterator(iter)),
            None => unreachable!("Option values are always iterable"),
        }
    // Debug trait - structural representation
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver)))
    // Higher-order methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_option_method_str(receiver, method_str, args)
    }
}

/// String-based dispatch for Option higher-order methods.
///
/// These methods require closures that can only be evaluated with a full
/// interpreter context. The dispatch handler validates argument count so
/// the method is recognized (not `UndefinedMethod`).
fn dispatch_option_method_str(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match method {
        "map" | "and_then" | "flat_map" | "filter" | "or_else" => {
            require_args(method, 1, args.len())?;
            // These require a closure — wrong_arg_type when called without interpreter
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        "expect" => {
            require_args("expect", 1, args.len())?;
            if let Value::Some(v) = &receiver {
                Ok((**v).clone())
            } else {
                let msg = match &args[0] {
                    Value::Str(s) => s.to_string(),
                    _ => "expect failed on None".to_string(),
                };
                Err(EvalError::new(msg).into())
            }
        }
        "or" => {
            require_args("or", 1, args.len())?;
            match &receiver {
                Value::Some(_) => Ok(receiver),
                _ => Ok(args.into_iter().next().unwrap_or(Value::None)),
            }
        }
        _ => Err(no_such_method(method, "Option").into()),
    }
}

/// Dispatch methods on Result values.
pub fn dispatch_result_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.unwrap || method == n.unwrap_or {
        match &receiver {
            Value::Ok(v) => Ok((**v).clone()),
            Value::Err(e) => {
                if method == n.unwrap {
                    Err(EvalError::new(format!("called unwrap on Err: {e:?}")).into())
                } else {
                    require_args("unwrap_or", 1, args.len())?;
                    match args.into_iter().next() {
                        Some(default) => Ok(default),
                        None => unreachable!("require_args verified length is 1"),
                    }
                }
            }
            _ => unreachable!(),
        }
    } else if method == n.unwrap_err {
        match &receiver {
            Value::Err(e) => Ok((**e).clone()),
            Value::Ok(v) => Err(EvalError::new(format!("called unwrap_err on Ok: {v:?}")).into()),
            _ => unreachable!(),
        }
    } else if method == n.is_ok {
        Ok(Value::Bool(matches!(&receiver, Value::Ok(_))))
    } else if method == n.is_err {
        Ok(Value::Bool(matches!(&receiver, Value::Err(_))))
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = &args[0];
        let ord = compare_result_values(&receiver, other, ctx.interner)?;
        Ok(ordering_to_value(ord))
    // Eq trait - deep equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - recursive hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&receiver, ctx.interner)?))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Debug trait - structural representation
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver)))
    // Traceable delegation: forward to inner Error if present
    } else if method == n.trace {
        require_args("trace", 0, args.len())?;
        Ok(Value::string(result_error_trace(&receiver)))
    } else if method == n.trace_entries {
        require_args("trace_entries", 0, args.len())?;
        match result_inner_error(&receiver) {
            Some(ev) => {
                let entries: Vec<Value> = ev
                    .trace()
                    .iter()
                    .map(|entry| super::error::trace_entry_to_struct(entry, ctx))
                    .collect();
                Ok(Value::list(entries))
            }
            None => Ok(Value::list(vec![])),
        }
    } else if method == n.has_trace {
        require_args("has_trace", 0, args.len())?;
        let has = result_inner_error(&receiver).is_some_and(ori_patterns::ErrorValue::has_trace);
        Ok(Value::Bool(has))
    // Higher-order and projection methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_result_method_str(&receiver, method_str, &args)
    }
}

/// String-based dispatch for Result methods not covered by Name-based dispatch.
fn dispatch_result_method_str(receiver: &Value, method: &str, args: &[Value]) -> EvalResult {
    match method {
        "map" | "map_err" | "and_then" | "or_else" => {
            require_args(method, 1, args.len())?;
            // These require a closure — wrong_arg_type when called without interpreter
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        "ok" => {
            require_args("ok", 0, args.len())?;
            if let Value::Ok(v) = receiver {
                Ok(Value::some((**v).clone()))
            } else {
                Ok(Value::None)
            }
        }
        "err" => {
            require_args("err", 0, args.len())?;
            if let Value::Err(e) = receiver {
                Ok(Value::some((**e).clone()))
            } else {
                Ok(Value::None)
            }
        }
        "expect" => {
            require_args("expect", 1, args.len())?;
            if let Value::Ok(v) = receiver {
                Ok((**v).clone())
            } else {
                let msg = match &args[0] {
                    Value::Str(s) => s.to_string(),
                    _ => "expect failed on Err".to_string(),
                };
                Err(EvalError::new(msg).into())
            }
        }
        "expect_err" => {
            require_args("expect_err", 1, args.len())?;
            if let Value::Err(e) = receiver {
                Ok((**e).clone())
            } else {
                let msg = match &args[0] {
                    Value::Str(s) => s.to_string(),
                    _ => "expect_err failed on Ok".to_string(),
                };
                Err(EvalError::new(msg).into())
            }
        }
        _ => Err(no_such_method(method, "Result").into()),
    }
}

/// Extract the inner `ErrorValue` from a Result's Err variant, if present.
fn result_inner_error(value: &Value) -> Option<&ori_patterns::ErrorValue> {
    match value {
        Value::Err(inner) => inner.as_error(),
        _ => None,
    }
}

/// Get the trace string from a Result's inner Error, or empty string.
fn result_error_trace(value: &Value) -> String {
    result_inner_error(value).map_or_else(String::new, ori_patterns::ErrorValue::format_trace)
}

/// Byte left shift with range validation.
fn byte_shift_left(b: u8, shift_val: i64) -> EvalResult {
    if let Ok(shift_u32) = u32::try_from(shift_val) {
        if shift_u32 < 8 {
            return Ok(Value::Byte(b << shift_u32));
        }
    }
    Err(EvalError::new(format!("shift amount {shift_val} out of range (0-7)")).into())
}

/// Byte right shift with range validation.
fn byte_shift_right(b: u8, shift_val: i64) -> EvalResult {
    if let Ok(shift_u32) = u32::try_from(shift_val) {
        if shift_u32 < 8 {
            return Ok(Value::Byte(b >> shift_u32));
        }
    }
    Err(EvalError::new(format!("shift amount {shift_val} out of range (0-7)")).into())
}
