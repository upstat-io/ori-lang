//! Method dispatch for scalar variant types (bool, char, byte, newtype).

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalError, EvalResult, Value};

use super::super::compare::ordering_to_value;
use super::super::helpers::{
    escape_debug_char, require_args, require_bool_arg, require_byte_arg, require_char_arg,
    require_scalar_int_arg,
};
use super::super::DispatchCtx;

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
    // Printable trait
    } else if method == n.to_str {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(format!("0x{b:02x}")))
    // Debug trait
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(b.to_string()))
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
    } else {
        dispatch_byte_tail(b, method, &args, ctx)
    }
}

fn dispatch_byte_tail(byte: u8, method: Name, args: &[Value], ctx: &DispatchCtx<'_>) -> EvalResult {
    let n = ctx.names;
    if method == n.bit_and {
        require_args("bit_and", 1, args.len())?;
        Ok(Value::Byte(byte & require_byte_arg("bit_and", args, 0)?))
    } else if method == n.bit_or {
        require_args("bit_or", 1, args.len())?;
        Ok(Value::Byte(byte | require_byte_arg("bit_or", args, 0)?))
    } else if method == n.bit_xor {
        require_args("bit_xor", 1, args.len())?;
        Ok(Value::Byte(byte ^ require_byte_arg("bit_xor", args, 0)?))
    } else if method == n.bit_not {
        require_args("bit_not", 0, args.len())?;
        Ok(Value::Byte(!byte))
    } else if method == n.shl || method == n.shr {
        let name = if method == n.shl { "shl" } else { "shr" };
        require_args(name, 1, args.len())?;
        let shift = require_scalar_int_arg(name, args, 0)?.raw();
        if method == n.shl {
            byte_shift_left(byte, shift)
        } else {
            byte_shift_right(byte, shift)
        }
    } else if method == n.is_ascii {
        require_args("is_ascii", 0, args.len())?;
        Ok(Value::Bool(true))
    } else if method == n.is_ascii_alpha {
        require_args("is_ascii_alpha", 0, args.len())?;
        Ok(Value::Bool(byte.is_ascii_alphabetic()))
    } else if method == n.is_ascii_digit {
        require_args("is_ascii_digit", 0, args.len())?;
        Ok(Value::Bool(byte.is_ascii_digit()))
    } else if method == n.is_ascii_whitespace {
        require_args("is_ascii_whitespace", 0, args.len())?;
        Ok(Value::Bool(byte.is_ascii_whitespace()))
    } else if method == n.to_char {
        require_args("to_char", 0, args.len())?;
        Ok(Value::Char(char::from(byte)))
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        Ok(Value::int(i64::from(byte)))
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
