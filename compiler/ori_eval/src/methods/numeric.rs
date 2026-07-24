//! Method dispatch for numeric types (int, float).

use ori_ir::Name;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, EvalError, EvalResult,
    Value,
};

use super::arguments::{require_args, require_float_arg, require_scalar_int_arg};
use super::compare::ordering_to_value;
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
    } else if let Some(result) = dispatch_int_bitwise(a, method, &args, ctx) {
        result
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
    } else {
        dispatch_int_named_method(a, receiver, method, &args, ctx)
    }
}

fn dispatch_int_bitwise(
    a: ori_patterns::ScalarInt,
    method: Name,
    args: &[Value],
    ctx: &DispatchCtx<'_>,
) -> Option<EvalResult> {
    let n = ctx.names;
    if ![n.bit_and, n.bit_or, n.bit_xor, n.bit_not, n.shl, n.shr].contains(&method) {
        return None;
    }
    Some((|| {
        if method == n.bit_and {
            require_args("bit_and", 1, args.len())?;
            Ok(Value::Int(a & require_scalar_int_arg("bit_and", args, 0)?))
        } else if method == n.bit_or {
            require_args("bit_or", 1, args.len())?;
            Ok(Value::Int(a | require_scalar_int_arg("bit_or", args, 0)?))
        } else if method == n.bit_xor {
            require_args("bit_xor", 1, args.len())?;
            Ok(Value::Int(a ^ require_scalar_int_arg("bit_xor", args, 0)?))
        } else if method == n.bit_not {
            require_args("bit_not", 0, args.len())?;
            Ok(Value::Int(!a))
        } else {
            let operation = if method == n.shl { "shl" } else { "shr" };
            require_args(operation, 1, args.len())?;
            let shift = require_scalar_int_arg(operation, args, 0)?;
            let value = if method == n.shl {
                a.checked_shl(shift)
            } else {
                a.checked_shr(shift)
            };
            value.map(Value::Int).ok_or_else(|| {
                EvalError::new(format!("shift amount {} out of range (0-63)", shift.raw())).into()
            })
        }
    })())
}

fn dispatch_int_named_method(
    a: ori_patterns::ScalarInt,
    receiver: Value,
    method: Name,
    args: &[Value],
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;
    if method == n.abs {
        require_args("abs", 0, args.len())?;
        a.raw()
            .checked_abs()
            .map(Value::int)
            .ok_or_else(|| integer_overflow("absolute value").into())
    } else if method == n.byte || method == n.to_byte {
        require_args("to_byte", 0, args.len())?;
        let raw = a.raw();
        u8::try_from(raw)
            .map(Value::Byte)
            .map_err(|_| EvalError::new(format!("integer {raw} out of byte range (0-255)")).into())
    } else if method == n.clamp {
        require_args("clamp", 2, args.len())?;
        let lo = require_scalar_int_arg("clamp", args, 0)?;
        let hi = require_scalar_int_arg("clamp", args, 1)?;
        Ok(Value::int(a.raw().clamp(lo.raw(), hi.raw())))
    } else if method == n.is_even {
        require_args("is_even", 0, args.len())?;
        Ok(Value::Bool(a.raw() % 2 == 0))
    } else if method == n.is_negative {
        require_args("is_negative", 0, args.len())?;
        Ok(Value::Bool(a.raw() < 0))
    } else if method == n.is_odd {
        require_args("is_odd", 0, args.len())?;
        Ok(Value::Bool(a.raw() % 2 != 0))
    } else if method == n.is_positive {
        require_args("is_positive", 0, args.len())?;
        Ok(Value::Bool(a.raw() > 0))
    } else if method == n.is_zero {
        require_args("is_zero", 0, args.len())?;
        Ok(Value::Bool(a.is_zero()))
    } else if method == n.max {
        require_args("max", 1, args.len())?;
        Ok(Value::Int(a.max(require_scalar_int_arg("max", args, 0)?)))
    } else if method == n.min {
        require_args("min", 1, args.len())?;
        Ok(Value::Int(a.min(require_scalar_int_arg("min", args, 0)?)))
    } else if method == n.pow {
        require_args("pow", 1, args.len())?;
        let exponent = require_scalar_int_arg("pow", args, 0)?.raw();
        if exponent < 0 {
            return Err(EvalError::new(format!("exponent {exponent} must be non-negative")).into());
        }
        let Ok(exponent) = u32::try_from(exponent) else {
            return Err(EvalError::new(format!("exponent {exponent} too large")).into());
        };
        a.raw()
            .checked_pow(exponent)
            .map(Value::int)
            .ok_or_else(|| integer_overflow("exponentiation").into())
    } else if method == n.signum {
        require_args("signum", 0, args.len())?;
        Ok(Value::int(a.raw().signum()))
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        Ok(receiver)
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
    // Hashable trait - IEEE 754 normalized hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(super::compare::hash_float(a)))
    // rem (float)
    } else if method == n.rem {
        require_args("rem", 1, args.len())?;
        let b = require_float_arg("rem", &args, 0)?;
        Ok(Value::Float(a % b))
    } else if let Some(result) = dispatch_float_math(a, method, &args, ctx) {
        result
    } else if let Some(result) = dispatch_float_predicate(a, method, &args, ctx) {
        result
    } else {
        dispatch_float_named_method(a, method, &args, ctx)
    }
}

fn dispatch_float_math(
    a: f64,
    method: Name,
    args: &[Value],
    ctx: &DispatchCtx<'_>,
) -> Option<EvalResult> {
    let n = ctx.names;
    let unary = [
        (n.abs, "abs", f64::abs as fn(f64) -> f64),
        (n.acos, "acos", f64::acos),
        (n.asin, "asin", f64::asin),
        (n.atan, "atan", f64::atan),
        (n.cbrt, "cbrt", f64::cbrt),
        (n.ceil, "ceil", f64::ceil),
        (n.cos, "cos", f64::cos),
        (n.exp, "exp", f64::exp),
        (n.floor, "floor", f64::floor),
        (n.ln, "ln", f64::ln),
        (n.log10, "log10", f64::log10),
        (n.log2, "log2", f64::log2),
        (n.round, "round", f64::round),
        (n.sin, "sin", f64::sin),
        (n.sqrt, "sqrt", f64::sqrt),
        (n.tan, "tan", f64::tan),
        (n.trunc, "trunc", f64::trunc),
    ];
    if let Some((_, name, operation)) = unary.iter().find(|(name, _, _)| *name == method) {
        return Some((|| {
            require_args(name, 0, args.len())?;
            Ok(Value::Float(operation(a)))
        })());
    }
    if method != n.atan2 && method != n.pow {
        return None;
    }
    Some((|| {
        let name = if method == n.atan2 { "atan2" } else { "pow" };
        require_args(name, 1, args.len())?;
        let operand = require_float_arg(name, args, 0)?;
        Ok(Value::Float(if method == n.atan2 {
            a.atan2(operand)
        } else {
            a.powf(operand)
        }))
    })())
}

fn dispatch_float_predicate(
    a: f64,
    method: Name,
    args: &[Value],
    ctx: &DispatchCtx<'_>,
) -> Option<EvalResult> {
    let n = ctx.names;
    let (name, result) = if method == n.is_finite {
        ("is_finite", a.is_finite())
    } else if method == n.is_infinite {
        ("is_infinite", a.is_infinite())
    } else if method == n.is_nan {
        ("is_nan", a.is_nan())
    } else if method == n.is_negative {
        ("is_negative", a.is_sign_negative() && a != 0.0)
    } else if method == n.is_normal {
        ("is_normal", a.is_normal())
    } else if method == n.is_positive {
        ("is_positive", a.is_sign_positive() && a != 0.0)
    } else if method == n.is_zero {
        ("is_zero", a == 0.0)
    } else {
        return None;
    };
    Some((|| {
        require_args(name, 0, args.len())?;
        Ok(Value::Bool(result))
    })())
}

fn dispatch_float_named_method(
    a: f64,
    method: Name,
    args: &[Value],
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;
    if method == n.clamp {
        require_args("clamp", 2, args.len())?;
        let lo = require_float_arg("clamp", args, 0)?;
        let hi = require_float_arg("clamp", args, 1)?;
        Ok(Value::Float(a.clamp(lo, hi)))
    } else if method == n.max {
        require_args("max", 1, args.len())?;
        Ok(Value::Float(a.max(require_float_arg("max", args, 0)?)))
    } else if method == n.min {
        require_args("min", 1, args.len())?;
        Ok(Value::Float(a.min(require_float_arg("min", args, 0)?)))
    } else if method == n.to_int {
        require_args("to_int", 0, args.len())?;
        float_to_int(a)
    } else if method == n.signum {
        require_args("signum", 0, args.len())?;
        Ok(Value::Float(a.signum()))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "float").into())
    }
}

fn float_to_int(value: f64) -> EvalResult {
    if value.is_nan() {
        return Err(EvalError::new("cannot convert NaN to int").into());
    }
    if value.is_infinite() {
        return Err(EvalError::new("cannot convert infinity to int").into());
    }
    let truncated = value.trunc();
    // INVARIANT: The valid range is `-2^63 <= value < 2^63`.
    let two_pow_63 = 2.0_f64.powi(63);
    if truncated >= two_pow_63 || truncated < -two_pow_63 {
        return Err(
            EvalError::new(format!("float {value} out of range for int conversion")).into(),
        );
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "bounds-checked: -2^63 <= truncated < 2^63"
    )]
    Ok(Value::int(truncated as i64))
}
