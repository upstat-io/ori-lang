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
    clippy::too_many_lines,
    reason = "exhaustive integer operator method dispatch"
)]
#[expect(
    clippy::cognitive_complexity,
    reason = "exhaustive integer method dispatch — flat if-else chain"
)]
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
    // Into trait: int -> float (lossless widening)
    } else if method == n.into || method == n.to_float || method == n.f {
        require_args("to_float", 0, args.len())?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "int->float is the defined Into conversion"
        )]
        Ok(Value::Float(a.raw() as f64))
    // abs
    } else if method == n.abs {
        require_args("abs", 0, args.len())?;
        a.raw()
            .checked_abs()
            .map(Value::int)
            .ok_or_else(|| integer_overflow("absolute value").into())
    // byte / to_byte
    } else if method == n.byte || method == n.to_byte {
        require_args("to_byte", 0, args.len())?;
        let raw = a.raw();
        u8::try_from(raw)
            .map(Value::Byte)
            .map_err(|_| EvalError::new(format!("integer {raw} out of byte range (0-255)")).into())
    // clamp
    } else if method == n.clamp {
        require_args("clamp", 2, args.len())?;
        let lo = require_scalar_int_arg("clamp", &args, 0)?;
        let hi = require_scalar_int_arg("clamp", &args, 1)?;
        let raw = a.raw();
        let lo_raw = lo.raw();
        let hi_raw = hi.raw();
        Ok(Value::int(raw.clamp(lo_raw, hi_raw)))
    // is_even
    } else if method == n.is_even {
        require_args("is_even", 0, args.len())?;
        Ok(Value::Bool(a.raw() % 2 == 0))
    // is_negative
    } else if method == n.is_negative {
        require_args("is_negative", 0, args.len())?;
        Ok(Value::Bool(a.raw() < 0))
    // is_odd
    } else if method == n.is_odd {
        require_args("is_odd", 0, args.len())?;
        Ok(Value::Bool(a.raw() % 2 != 0))
    // is_positive
    } else if method == n.is_positive {
        require_args("is_positive", 0, args.len())?;
        Ok(Value::Bool(a.raw() > 0))
    // is_zero
    } else if method == n.is_zero {
        require_args("is_zero", 0, args.len())?;
        Ok(Value::Bool(a.is_zero()))
    // max
    } else if method == n.max {
        require_args("max", 1, args.len())?;
        let b = require_scalar_int_arg("max", &args, 0)?;
        Ok(Value::Int(a.max(b)))
    // min
    } else if method == n.min {
        require_args("min", 1, args.len())?;
        let b = require_scalar_int_arg("min", &args, 0)?;
        Ok(Value::Int(a.min(b)))
    // pow
    } else if method == n.pow {
        require_args("pow", 1, args.len())?;
        let exp = require_scalar_int_arg("pow", &args, 0)?;
        let exp_raw = exp.raw();
        if exp_raw < 0 {
            return Err(EvalError::new(format!("exponent {exp_raw} must be non-negative")).into());
        }
        let Ok(exp_u32) = u32::try_from(exp_raw) else {
            return Err(EvalError::new(format!("exponent {exp_raw} too large")).into());
        };
        a.raw()
            .checked_pow(exp_u32)
            .map(Value::int)
            .ok_or_else(|| integer_overflow("exponentiation").into())
    // signum
    } else if method == n.signum {
        require_args("signum", 0, args.len())?;
        Ok(Value::int(a.raw().signum()))
    // to_int (identity for int)
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        Ok(receiver)
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "int").into())
    }
}

/// Dispatch operator methods on float values.
#[expect(clippy::too_many_lines, reason = "exhaustive float method dispatch")]
#[expect(
    clippy::cognitive_complexity,
    reason = "exhaustive float method dispatch — flat if-else chain"
)]
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
    // Hashable trait - IEEE 754 normalized hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(super::compare::hash_float(a)))
    // rem (float)
    } else if method == n.rem {
        require_args("rem", 1, args.len())?;
        let b = require_float_arg("rem", &args, 0)?;
        Ok(Value::Float(a % b))
    // Math: unary f64 methods
    } else if method == n.abs {
        require_args("abs", 0, args.len())?;
        Ok(Value::Float(a.abs()))
    } else if method == n.acos {
        require_args("acos", 0, args.len())?;
        Ok(Value::Float(a.acos()))
    } else if method == n.asin {
        require_args("asin", 0, args.len())?;
        Ok(Value::Float(a.asin()))
    } else if method == n.atan {
        require_args("atan", 0, args.len())?;
        Ok(Value::Float(a.atan()))
    } else if method == n.atan2 {
        require_args("atan2", 1, args.len())?;
        let b = require_float_arg("atan2", &args, 0)?;
        Ok(Value::Float(a.atan2(b)))
    } else if method == n.cbrt {
        require_args("cbrt", 0, args.len())?;
        Ok(Value::Float(a.cbrt()))
    } else if method == n.ceil {
        require_args("ceil", 0, args.len())?;
        Ok(Value::Float(a.ceil()))
    } else if method == n.cos {
        require_args("cos", 0, args.len())?;
        Ok(Value::Float(a.cos()))
    } else if method == n.exp {
        require_args("exp", 0, args.len())?;
        Ok(Value::Float(a.exp()))
    } else if method == n.floor {
        require_args("floor", 0, args.len())?;
        Ok(Value::Float(a.floor()))
    } else if method == n.ln {
        require_args("ln", 0, args.len())?;
        Ok(Value::Float(a.ln()))
    } else if method == n.log10 {
        require_args("log10", 0, args.len())?;
        Ok(Value::Float(a.log10()))
    } else if method == n.log2 {
        require_args("log2", 0, args.len())?;
        Ok(Value::Float(a.log2()))
    } else if method == n.pow {
        require_args("pow", 1, args.len())?;
        let b = require_float_arg("pow", &args, 0)?;
        Ok(Value::Float(a.powf(b)))
    } else if method == n.round {
        require_args("round", 0, args.len())?;
        Ok(Value::Float(a.round()))
    } else if method == n.sin {
        require_args("sin", 0, args.len())?;
        Ok(Value::Float(a.sin()))
    } else if method == n.sqrt {
        require_args("sqrt", 0, args.len())?;
        Ok(Value::Float(a.sqrt()))
    } else if method == n.tan {
        require_args("tan", 0, args.len())?;
        Ok(Value::Float(a.tan()))
    } else if method == n.trunc {
        require_args("trunc", 0, args.len())?;
        Ok(Value::Float(a.trunc()))
    // Float predicates
    } else if method == n.is_finite {
        require_args("is_finite", 0, args.len())?;
        Ok(Value::Bool(a.is_finite()))
    } else if method == n.is_infinite {
        require_args("is_infinite", 0, args.len())?;
        Ok(Value::Bool(a.is_infinite()))
    } else if method == n.is_nan {
        require_args("is_nan", 0, args.len())?;
        Ok(Value::Bool(a.is_nan()))
    } else if method == n.is_negative {
        require_args("is_negative", 0, args.len())?;
        Ok(Value::Bool(a.is_sign_negative() && a != 0.0))
    } else if method == n.is_normal {
        require_args("is_normal", 0, args.len())?;
        Ok(Value::Bool(a.is_normal()))
    } else if method == n.is_positive {
        require_args("is_positive", 0, args.len())?;
        Ok(Value::Bool(a.is_sign_positive() && a != 0.0))
    } else if method == n.is_zero {
        require_args("is_zero", 0, args.len())?;
        Ok(Value::Bool(a == 0.0))
    // Float binary: clamp, max, min
    } else if method == n.clamp {
        require_args("clamp", 2, args.len())?;
        let lo = require_float_arg("clamp", &args, 0)?;
        let hi = require_float_arg("clamp", &args, 1)?;
        Ok(Value::Float(a.clamp(lo, hi)))
    } else if method == n.max {
        require_args("max", 1, args.len())?;
        let b = require_float_arg("max", &args, 0)?;
        Ok(Value::Float(a.max(b)))
    } else if method == n.min {
        require_args("min", 1, args.len())?;
        let b = require_float_arg("min", &args, 0)?;
        Ok(Value::Float(a.min(b)))
    // Float conversion: to_int
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        if a.is_nan() {
            Err(EvalError::new("cannot convert NaN to int").into())
        } else if a.is_infinite() {
            Err(EvalError::new("cannot convert infinity to int").into())
        } else {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "float->int truncation is the defined conversion"
            )]
            let n = a as i64;
            // Check if conversion is valid by round-tripping
            #[expect(
                clippy::cast_precision_loss,
                reason = "checking round-trip for float->int conversion"
            )]
            if (n as f64 - a).abs() > 1.0 {
                Err(EvalError::new(format!("float {a} out of range for int conversion")).into())
            } else {
                Ok(Value::int(n))
            }
        }
    // Float signum
    } else if method == n.signum {
        require_args("signum", 0, args.len())?;
        Ok(Value::Float(a.signum()))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "float").into())
    }
}
