//! Method dispatch for unit types (Duration, Size).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ori_ir::builtin_constants::{duration, size};
use ori_ir::Name;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, EvalError, EvalResult,
    Value,
};

use super::compare::ordering_to_value;
use super::helpers::{require_args, require_duration_arg, require_int_arg, require_size_arg};
use super::DispatchCtx;

/// Create a Duration value from an integer with a multiplier.
///
/// Reduces repetition in Duration factory methods (`from_microseconds`, `from_seconds`, etc.).
#[inline]
fn duration_from_int(method: &str, args: &[Value], multiplier: i64) -> EvalResult {
    require_args(method, 1, args.len())?;
    let val = require_int_arg(method, args, 0)?;
    val.checked_mul(multiplier)
        .map(Value::Duration)
        .ok_or_else(|| EvalError::new("duration overflow").into())
}

/// Create a Size value from an integer with a multiplier.
///
/// Reduces repetition in Size factory methods (`from_kilobytes`, `from_megabytes`, etc.).
/// Handles the negative value check that Size requires.
#[inline]
fn size_from_int(method: &str, args: &[Value], multiplier: u64) -> EvalResult {
    require_args(method, 1, args.len())?;
    let val = require_int_arg(method, args, 0)?;
    if val < 0 {
        return Err(EvalError::new("Size cannot be negative").into());
    }
    #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
    (val as u64)
        .checked_mul(multiplier)
        .map(Value::Size)
        .ok_or_else(|| EvalError::new("size overflow").into())
}

/// Dispatch Duration associated functions (factory methods).
///
/// These remain string-based since associated function calls are infrequent
/// and the caller already de-interns for the type name dispatch.
pub fn dispatch_duration_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_nanoseconds" => duration_from_int(method, args, 1),
        "from_microseconds" => duration_from_int(method, args, duration::NS_PER_US),
        "from_milliseconds" => duration_from_int(method, args, duration::NS_PER_MS),
        "from_seconds" => duration_from_int(method, args, duration::NS_PER_S),
        "from_minutes" => duration_from_int(method, args, duration::NS_PER_M),
        "from_hours" => duration_from_int(method, args, duration::NS_PER_H),
        "default" => {
            require_args("default", 0, args.len())?;
            Ok(Value::Duration(0)) // 0ns is the default Duration
        }
        _ => Err(no_such_method(method, "Duration").into()),
    }
}

/// Dispatch Size associated functions (factory methods).
///
/// These remain string-based since associated function calls are infrequent
/// and the caller already de-interns for the type name dispatch.
pub fn dispatch_size_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_bytes" => size_from_int(method, args, 1),
        "from_kilobytes" => size_from_int(method, args, size::BYTES_PER_KB),
        "from_megabytes" => size_from_int(method, args, size::BYTES_PER_MB),
        "from_gigabytes" => size_from_int(method, args, size::BYTES_PER_GB),
        "from_terabytes" => size_from_int(method, args, size::BYTES_PER_TB),
        "default" => {
            require_args("default", 0, args.len())?;
            Ok(Value::Size(0)) // 0b is the default Size
        }
        _ => Err(no_such_method(method, "Size").into()),
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
    } else if method == n.to_str {
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
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Duration").into())
    }
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

/// Dispatch methods on Size values.
/// Size is stored as u64 bytes.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_size_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Size(bytes) = receiver else {
        unreachable!("dispatch_size_method called with non-size receiver")
    };

    let n = ctx.names;

    // Convert u64 to i64 safely (truncating division results fit in i64)
    let to_int = |v: u64| -> EvalResult {
        i64::try_from(v)
            .map(Value::int)
            .map_err(|_| EvalError::new("size value too large for int").into())
    };

    // SI units: 1kb = 1000 bytes, 1mb = 1,000,000 bytes, etc.
    if method == n.bytes {
        to_int(bytes)
    } else if method == n.kilobytes {
        to_int(bytes / size::BYTES_PER_KB)
    } else if method == n.megabytes {
        to_int(bytes / size::BYTES_PER_MB)
    } else if method == n.gigabytes {
        to_int(bytes / size::BYTES_PER_GB)
    } else if method == n.terabytes {
        to_int(bytes / size::BYTES_PER_TB)
    // Operator methods
    } else if method == n.add {
        require_args("add", 1, args.len())?;
        let other = require_size_arg("add", &args, 0)?;
        bytes
            .checked_add(other)
            .map(Value::Size)
            .ok_or_else(|| integer_overflow("size addition").into())
    } else if method == n.sub || method == n.subtract {
        require_args("sub", 1, args.len())?;
        let other = require_size_arg("sub", &args, 0)?;
        bytes
            .checked_sub(other)
            .map(Value::Size)
            .ok_or_else(|| EvalError::new("size subtraction would result in negative value").into())
    } else if method == n.mul || method == n.multiply {
        require_args("mul", 1, args.len())?;
        let scalar = require_int_arg("mul", &args, 0)?;
        if scalar < 0 {
            return Err(EvalError::new("cannot multiply Size by negative integer").into());
        }
        #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
        bytes
            .checked_mul(scalar as u64)
            .map(Value::Size)
            .ok_or_else(|| integer_overflow("size multiplication").into())
    } else if method == n.div || method == n.divide {
        require_args("div", 1, args.len())?;
        let scalar = require_int_arg("div", &args, 0)?;
        if scalar == 0 {
            return Err(division_by_zero().into());
        }
        if scalar < 0 {
            return Err(EvalError::new("cannot divide Size by negative integer").into());
        }
        #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
        bytes
            .checked_div(scalar as u64)
            .map(Value::Size)
            .ok_or_else(|| integer_overflow("size division").into())
    } else if method == n.rem || method == n.remainder {
        require_args("rem", 1, args.len())?;
        let other = require_size_arg("rem", &args, 0)?;
        if other == 0 {
            Err(modulo_by_zero().into())
        } else {
            bytes
                .checked_rem(other)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size modulo").into())
        }
    // Trait methods
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        let mut hasher = DefaultHasher::new();
        "Size".hash(&mut hasher);
        bytes.hash(&mut hasher);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "Hash values are opaque identifiers"
        )]
        Ok(Value::int(hasher.finish() as i64))
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::Size(bytes))
    } else if method == n.to_str {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(format_size(bytes)))
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_size_arg("equals", &args, 0)?;
        Ok(Value::Bool(bytes == other))
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_size_arg("compare", &args, 0)?;
        Ok(ordering_to_value(bytes.cmp(&other)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Size").into())
    }
}

/// Format a Size (bytes) as a human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0b".to_string();
    }

    // Use the largest unit that gives a whole number
    if bytes.is_multiple_of(size::BYTES_PER_TB) {
        let terabytes = bytes / size::BYTES_PER_TB;
        format!("{terabytes}tb")
    } else if bytes.is_multiple_of(size::BYTES_PER_GB) {
        let gigabytes = bytes / size::BYTES_PER_GB;
        format!("{gigabytes}gb")
    } else if bytes.is_multiple_of(size::BYTES_PER_MB) {
        let megabytes = bytes / size::BYTES_PER_MB;
        format!("{megabytes}mb")
    } else if bytes.is_multiple_of(size::BYTES_PER_KB) {
        let kilobytes = bytes / size::BYTES_PER_KB;
        format!("{kilobytes}kb")
    } else {
        format!("{bytes}b")
    }
}
