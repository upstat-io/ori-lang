//! Method dispatch for numeric types (int, float).

use ori_ir::Name;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, EvalError, EvalResult,
    Value,
};

use super::compare::ordering_to_value;
use super::helpers::{require_args, require_float_arg, require_scalar_int_arg};
use super::DispatchCtx;

/// Dispatch operator methods on integer values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_int_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Int(a) = receiver else {
        unreachable!("dispatch_int_method called with non-int receiver")
    };

    let n = ctx.names;

    // Binary arithmetic operators
    if method == n.add {
        require_args("add", 1, args.len())?;
        let b = require_scalar_int_arg("add", &args, 0)?;
        a.checked_add(b)
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("addition").into())
    } else if method == n.sub {
        require_args("sub", 1, args.len())?;
        let b = require_scalar_int_arg("sub", &args, 0)?;
        a.checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("subtraction").into())
    } else if method == n.mul {
        require_args("mul", 1, args.len())?;
        let b = require_scalar_int_arg("mul", &args, 0)?;
        a.checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("multiplication").into())
    } else if method == n.div {
        require_args("div", 1, args.len())?;
        let b = require_scalar_int_arg("div", &args, 0)?;
        if b.is_zero() {
            Err(division_by_zero().into())
        } else {
            a.checked_div(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("division").into())
        }
    } else if method == n.floor_div {
        require_args("floor_div", 1, args.len())?;
        let b = require_scalar_int_arg("floor_div", &args, 0)?;
        if b.is_zero() {
            Err(division_by_zero().into())
        } else {
            a.checked_floor_div(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("floor division").into())
        }
    } else if method == n.rem {
        require_args("rem", 1, args.len())?;
        let b = require_scalar_int_arg("rem", &args, 0)?;
        if b.is_zero() {
            Err(modulo_by_zero().into())
        } else {
            a.checked_rem(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("remainder").into())
        }
    // Unary operators
    } else if method == n.neg {
        require_args("neg", 0, args.len())?;
        a.checked_neg()
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("negation").into())
    // Bitwise operators
    } else if method == n.bit_and {
        require_args("bit_and", 1, args.len())?;
        let b = require_scalar_int_arg("bit_and", &args, 0)?;
        Ok(Value::Int(a & b))
    } else if method == n.bit_or {
        require_args("bit_or", 1, args.len())?;
        let b = require_scalar_int_arg("bit_or", &args, 0)?;
        Ok(Value::Int(a | b))
    } else if method == n.bit_xor {
        require_args("bit_xor", 1, args.len())?;
        let b = require_scalar_int_arg("bit_xor", &args, 0)?;
        Ok(Value::Int(a ^ b))
    } else if method == n.bit_not {
        require_args("bit_not", 0, args.len())?;
        Ok(Value::Int(!a))
    } else if method == n.shl {
        require_args("shl", 1, args.len())?;
        let b = require_scalar_int_arg("shl", &args, 0)?;
        a.checked_shl(b).map(Value::Int).ok_or_else(|| {
            EvalError::new(format!("shift amount {} out of range (0-63)", b.raw())).into()
        })
    } else if method == n.shr {
        require_args("shr", 1, args.len())?;
        let b = require_scalar_int_arg("shr", &args, 0)?;
        a.checked_shr(b).map(Value::Int).ok_or_else(|| {
            EvalError::new(format!("shift amount {} out of range (0-63)", b.raw())).into()
        })
    // Comparable trait
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let b = require_scalar_int_arg("compare", &args, 0)?;
        Ok(ordering_to_value(a.cmp(&b)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let b = require_scalar_int_arg("equals", &args, 0)?;
        Ok(Value::Bool(a == b))
    // Clone trait (Copy semantics for primitives)
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable and Debug traits
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(a.raw().to_string()))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        // For integers, use the value itself as its hash (simple but effective)
        Ok(Value::Int(a))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "int").into())
    }
}

/// Dispatch operator methods on float values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_float_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Float(a) = receiver else {
        unreachable!("dispatch_float_method called with non-float receiver")
    };

    let n = ctx.names;

    if method == n.add {
        require_args("add", 1, args.len())?;
        let b = require_float_arg("add", &args, 0)?;
        Ok(Value::Float(a + b))
    } else if method == n.sub {
        require_args("sub", 1, args.len())?;
        let b = require_float_arg("sub", &args, 0)?;
        Ok(Value::Float(a - b))
    } else if method == n.mul {
        require_args("mul", 1, args.len())?;
        let b = require_float_arg("mul", &args, 0)?;
        Ok(Value::Float(a * b))
    } else if method == n.div {
        require_args("div", 1, args.len())?;
        let b = require_float_arg("div", &args, 0)?;
        Ok(Value::Float(a / b))
    } else if method == n.neg {
        require_args("neg", 0, args.len())?;
        Ok(Value::Float(-a))
    // Comparable trait - IEEE 754 total ordering
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let b = require_float_arg("compare", &args, 0)?;
        // Use total_cmp for IEEE 754 total ordering (handles NaN consistently)
        Ok(ordering_to_value(a.total_cmp(&b)))
    // Eq trait - exact bit comparison (intentional for float equality)
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let b = require_float_arg("equals", &args, 0)?;
        #[expect(
            clippy::float_cmp,
            reason = "Exact float equality is intentional for Eq trait"
        )]
        Ok(Value::Bool(a == b))
    // Clone trait (Copy semantics for primitives)
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable and Debug traits
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(a.to_string()))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        // Use bits representation for consistent hashing
        Ok(Value::int(a.to_bits().cast_signed()))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "float").into())
    }
}
