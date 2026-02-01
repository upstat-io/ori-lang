//! Method dispatch for numeric types (int, float).

use super::compare::ordering_to_value;
use super::helpers::{require_args, require_float_arg, require_scalar_int_arg};
use ori_ir::StringInterner;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, EvalError, EvalResult,
    Value,
};

/// Dispatch operator methods on integer values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_int_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Int(a) = receiver else {
        unreachable!("dispatch_int_method called with non-int receiver")
    };

    match method {
        // Binary arithmetic operators
        "add" => {
            require_args("add", 1, args.len())?;
            let b = require_scalar_int_arg("add", &args, 0)?;
            a.checked_add(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("addition"))
        }
        "sub" => {
            require_args("sub", 1, args.len())?;
            let b = require_scalar_int_arg("sub", &args, 0)?;
            a.checked_sub(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("subtraction"))
        }
        "mul" => {
            require_args("mul", 1, args.len())?;
            let b = require_scalar_int_arg("mul", &args, 0)?;
            a.checked_mul(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("multiplication"))
        }
        "div" => {
            require_args("div", 1, args.len())?;
            let b = require_scalar_int_arg("div", &args, 0)?;
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_div(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("division"))
            }
        }
        "floor_div" => {
            require_args("floor_div", 1, args.len())?;
            let b = require_scalar_int_arg("floor_div", &args, 0)?;
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_floor_div(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("floor division"))
            }
        }
        "rem" => {
            require_args("rem", 1, args.len())?;
            let b = require_scalar_int_arg("rem", &args, 0)?;
            if b.is_zero() {
                Err(modulo_by_zero())
            } else {
                a.checked_rem(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("remainder"))
            }
        }
        // Unary operators
        "neg" => {
            require_args("neg", 0, args.len())?;
            a.checked_neg()
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("negation"))
        }
        // Bitwise operators
        "bit_and" => {
            require_args("bit_and", 1, args.len())?;
            let b = require_scalar_int_arg("bit_and", &args, 0)?;
            Ok(Value::Int(a & b))
        }
        "bit_or" => {
            require_args("bit_or", 1, args.len())?;
            let b = require_scalar_int_arg("bit_or", &args, 0)?;
            Ok(Value::Int(a | b))
        }
        "bit_xor" => {
            require_args("bit_xor", 1, args.len())?;
            let b = require_scalar_int_arg("bit_xor", &args, 0)?;
            Ok(Value::Int(a ^ b))
        }
        "bit_not" => {
            require_args("bit_not", 0, args.len())?;
            Ok(Value::Int(!a))
        }
        "shl" => {
            require_args("shl", 1, args.len())?;
            let b = require_scalar_int_arg("shl", &args, 0)?;
            a.checked_shl(b).map(Value::Int).ok_or_else(|| {
                EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))
            })
        }
        "shr" => {
            require_args("shr", 1, args.len())?;
            let b = require_scalar_int_arg("shr", &args, 0)?;
            a.checked_shr(b).map(Value::Int).ok_or_else(|| {
                EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))
            })
        }
        // Comparable trait
        "compare" => {
            require_args("compare", 1, args.len())?;
            let b = require_scalar_int_arg("compare", &args, 0)?;
            Ok(ordering_to_value(a.cmp(&b), interner))
        }
        _ => Err(no_such_method(method, "int")),
    }
}

/// Dispatch operator methods on float values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_float_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Float(a) = receiver else {
        unreachable!("dispatch_float_method called with non-float receiver")
    };

    match method {
        "add" => {
            require_args("add", 1, args.len())?;
            let b = require_float_arg("add", &args, 0)?;
            Ok(Value::Float(a + b))
        }
        "sub" => {
            require_args("sub", 1, args.len())?;
            let b = require_float_arg("sub", &args, 0)?;
            Ok(Value::Float(a - b))
        }
        "mul" => {
            require_args("mul", 1, args.len())?;
            let b = require_float_arg("mul", &args, 0)?;
            Ok(Value::Float(a * b))
        }
        "div" => {
            require_args("div", 1, args.len())?;
            let b = require_float_arg("div", &args, 0)?;
            Ok(Value::Float(a / b))
        }
        "neg" => {
            require_args("neg", 0, args.len())?;
            Ok(Value::Float(-a))
        }
        // Comparable trait - IEEE 754 total ordering
        "compare" => {
            require_args("compare", 1, args.len())?;
            let b = require_float_arg("compare", &args, 0)?;
            // Use total_cmp for IEEE 754 total ordering (handles NaN consistently)
            Ok(ordering_to_value(a.total_cmp(&b), interner))
        }
        _ => Err(no_such_method(method, "float")),
    }
}
