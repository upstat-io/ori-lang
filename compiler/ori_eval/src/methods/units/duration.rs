//! Method dispatch for the Duration unit type (stored as i64 nanoseconds).

use ori_ir::builtin_constants::duration;
use ori_ir::Name;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, EvalResult, Value,
};

use super::super::arguments::{require_args, require_duration_arg, require_int_arg};
use super::super::compare::ordering_to_value;
use super::super::DispatchCtx;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Create a Duration value from an integer with a multiplier.
///
/// Reduces repetition in Duration factory methods (`from_microseconds`, `from_seconds`, etc.).
#[inline]
fn duration_from_int(method: &str, args: &[Value], multiplier: i64) -> EvalResult {
    require_args(method, 1, args.len())?;
    let val = require_int_arg(method, args, 0)?;
    val.checked_mul(multiplier)
        .map(Value::Duration)
        .ok_or_else(|| integer_overflow("duration factory conversion").into())
}

/// Dispatch Duration associated functions (factory methods).
///
/// These remain string-based since associated function calls are infrequent
/// and the caller already de-interns for the type name dispatch.
pub fn dispatch_duration_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_nanoseconds" | "from_nanos" => duration_from_int(method, args, 1),
        "from_microseconds" | "from_micros" => duration_from_int(method, args, duration::NS_PER_US),
        "from_milliseconds" | "from_millis" => duration_from_int(method, args, duration::NS_PER_MS),
        "from_seconds" => duration_from_int(method, args, duration::NS_PER_S),
        "from_minutes" => duration_from_int(method, args, duration::NS_PER_M),
        "from_hours" => duration_from_int(method, args, duration::NS_PER_H),
        "zero" => {
            require_args("zero", 0, args.len())?;
            Ok(Value::Duration(0))
        }
        "default" => {
            require_args("default", 0, args.len())?;
            Ok(Value::Duration(0)) // 0ns is the default Duration
        }
        _ => Err(no_such_method(method, "Duration").into()),
    }
}

/// Dispatch methods on Duration values.
/// Duration is stored as i64 nanoseconds.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_duration_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Duration(ns) = receiver else {
        unreachable!("dispatch_duration_method called with non-duration receiver")
    };

    let n = ctx.names;

    // Accessors
    if method == n.nanoseconds {
        Ok(Value::int(ns))
    } else if method == n.microseconds {
        Ok(Value::int(ns / duration::NS_PER_US))
    } else if method == n.milliseconds {
        Ok(Value::int(ns / duration::NS_PER_MS))
    } else if method == n.seconds {
        Ok(Value::int(ns / duration::NS_PER_S))
    } else if method == n.minutes {
        Ok(Value::int(ns / duration::NS_PER_M))
    } else if method == n.hours {
        Ok(Value::int(ns / duration::NS_PER_H))
    // Operator methods
    } else if method == n.add {
        require_args("add", 1, args.len())?;
        let other = require_duration_arg("add", &args, 0)?;
        ns.checked_add(other)
            .map(Value::Duration)
            .ok_or_else(|| integer_overflow("duration addition").into())
    } else if method == n.sub || method == n.subtract {
        require_args("sub", 1, args.len())?;
        let other = require_duration_arg("sub", &args, 0)?;
        ns.checked_sub(other)
            .map(Value::Duration)
            .ok_or_else(|| integer_overflow("duration subtraction").into())
    } else if method == n.mul || method == n.multiply {
        require_args("mul", 1, args.len())?;
        let scalar = require_int_arg("mul", &args, 0)?;
        ns.checked_mul(scalar)
            .map(Value::Duration)
            .ok_or_else(|| integer_overflow("duration multiplication").into())
    } else if method == n.div || method == n.divide {
        require_args("div", 1, args.len())?;
        let scalar = require_int_arg("div", &args, 0)?;
        if scalar == 0 {
            Err(division_by_zero().into())
        } else {
            ns.checked_div(scalar)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration division").into())
        }
    } else if method == n.rem || method == n.remainder {
        require_args("rem", 1, args.len())?;
        let other = require_duration_arg("rem", &args, 0)?;
        if other == 0 {
            Err(modulo_by_zero().into())
        } else {
            ns.checked_rem(other)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration modulo").into())
        }
    } else if method == n.neg || method == n.negate {
        require_args("neg", 0, args.len())?;
        ns.checked_neg()
            .map(Value::Duration)
            .ok_or_else(|| integer_overflow("duration negation").into())
    // Trait methods
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        let mut hasher = DefaultHasher::new();
        "Duration".hash(&mut hasher);
        ns.hash(&mut hasher);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "Hash values are opaque identifiers"
        )]
        Ok(Value::int(hasher.finish() as i64))
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::Duration(ns))
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(format_duration(ns)))
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_duration_arg("equals", &args, 0)?;
        Ok(Value::Bool(ns == other))
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_duration_arg("compare", &args, 0)?;
        Ok(ordering_to_value(ns.cmp(&other)))
    // Duration predicates and conversion (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_duration_method_str(ns, method_str, &args)
    }
}

/// String-based dispatch for Duration methods not hot enough to warrant
/// pre-interned Name fields.
#[expect(
    clippy::cast_precision_loss,
    reason = "i64-to-f64 intentional for as_*/to_* conversions"
)]
fn dispatch_duration_method_str(ns: i64, method: &str, args: &[Value]) -> EvalResult {
    match method {
        // Predicates
        "is_zero" => {
            require_args("is_zero", 0, args.len())?;
            Ok(Value::Bool(ns == 0))
        }
        "is_positive" => {
            require_args("is_positive", 0, args.len())?;
            Ok(Value::Bool(ns > 0))
        }
        "is_negative" => {
            require_args("is_negative", 0, args.len())?;
            Ok(Value::Bool(ns < 0))
        }
        // abs
        "abs" => {
            require_args("abs", 0, args.len())?;
            ns.checked_abs()
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration abs").into())
        }
        // Conversion to float (as_* returns fractional float)
        "as_nanos" => {
            require_args("as_nanos", 0, args.len())?;
            Ok(Value::Float(ns as f64))
        }
        "as_micros" => {
            require_args("as_micros", 0, args.len())?;
            Ok(Value::Float(ns as f64 / duration::NS_PER_US as f64))
        }
        "as_millis" => {
            require_args("as_millis", 0, args.len())?;
            Ok(Value::Float(ns as f64 / duration::NS_PER_MS as f64))
        }
        "as_seconds" => {
            require_args("as_seconds", 0, args.len())?;
            Ok(Value::Float(ns as f64 / duration::NS_PER_S as f64))
        }
        // Conversion to float (to_* aliases)
        "to_nanos" => {
            require_args("to_nanos", 0, args.len())?;
            Ok(Value::Float(ns as f64))
        }
        "to_micros" => {
            require_args("to_micros", 0, args.len())?;
            Ok(Value::Float(ns as f64 / duration::NS_PER_US as f64))
        }
        "to_millis" => {
            require_args("to_millis", 0, args.len())?;
            Ok(Value::Float(ns as f64 / duration::NS_PER_MS as f64))
        }
        "to_seconds" => {
            require_args("to_seconds", 0, args.len())?;
            Ok(Value::Float(ns as f64 / duration::NS_PER_S as f64))
        }
        // format (Formattable)
        "format" => {
            require_args("format", 0, args.len())?;
            Ok(Value::string(format_duration(ns)))
        }
        // Associated functions routed through instance dispatch for test coverage.
        // In production, these are called via dispatch_associated_function.
        "from_nanoseconds" | "from_microseconds" | "from_milliseconds" | "from_seconds"
        | "from_minutes" | "from_hours" | "from_nanos" | "from_micros" | "from_millis" | "zero"
        | "default" => dispatch_duration_associated(method, args),
        _ => Err(no_such_method(method, "Duration").into()),
    }
}

/// Format a Duration for Debug output. Same as Printable for Duration.
pub(in crate::methods) fn format_duration_debug(ns: i64) -> String {
    format_duration(ns)
}

/// Format a Duration (nanoseconds) as a human-readable string.
fn format_duration(ns: i64) -> String {
    use duration::unsigned as dur;

    let abs_ns = ns.unsigned_abs();
    let sign = if ns < 0 { "-" } else { "" };

    if abs_ns == 0 {
        return "0ns".to_string();
    }

    // Use the largest unit that gives a whole number
    if abs_ns.is_multiple_of(dur::NS_PER_H) {
        let hours = abs_ns / dur::NS_PER_H;
        format!("{sign}{hours}h")
    } else if abs_ns.is_multiple_of(dur::NS_PER_M) {
        let minutes = abs_ns / dur::NS_PER_M;
        format!("{sign}{minutes}m")
    } else if abs_ns.is_multiple_of(dur::NS_PER_S) {
        let seconds = abs_ns / dur::NS_PER_S;
        format!("{sign}{seconds}s")
    } else if abs_ns.is_multiple_of(dur::NS_PER_MS) {
        let milliseconds = abs_ns / dur::NS_PER_MS;
        format!("{sign}{milliseconds}ms")
    } else if abs_ns.is_multiple_of(dur::NS_PER_US) {
        let microseconds = abs_ns / dur::NS_PER_US;
        format!("{sign}{microseconds}us")
    } else {
        format!("{sign}{abs_ns}ns")
    }
}
