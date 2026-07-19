//! Method dispatch for the Size unit type (a non-negative signed byte count).

use ori_ir::builtin_constants::size;
use ori_ir::Name;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, size_negative_divide,
    size_negative_multiply, size_would_be_negative, EvalError, EvalResult, Value,
};

use super::super::arguments::{require_args, require_int_arg, require_size_arg};
use super::super::compare::ordering_to_value;
use super::super::DispatchCtx;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn checked_size(result: Option<u64>, operation: &'static str) -> EvalResult {
    result
        .filter(|value| *value <= size::MAX_BYTES)
        .map(Value::Size)
        .ok_or_else(|| integer_overflow(operation).into())
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
    checked_size(
        (val as u64).checked_mul(multiplier),
        "size factory conversion",
    )
}

/// Dispatch Size associated functions (factory methods).
///
/// These remain string-based since associated function calls are infrequent
/// and the caller already de-interns for the type name dispatch.
pub fn dispatch_size_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_bytes" => size_from_int(method, args, 1),
        "from_kilobytes" | "from_kb" => size_from_int(method, args, size::BYTES_PER_KB),
        "from_megabytes" | "from_mb" => size_from_int(method, args, size::BYTES_PER_MB),
        "from_gigabytes" | "from_gb" => size_from_int(method, args, size::BYTES_PER_GB),
        "from_terabytes" | "from_tb" => size_from_int(method, args, size::BYTES_PER_TB),
        "zero" => {
            require_args("zero", 0, args.len())?;
            Ok(Value::Size(0))
        }
        "default" => {
            require_args("default", 0, args.len())?;
            Ok(Value::Size(0)) // 0b is the default Size
        }
        _ => Err(no_such_method(method, "Size").into()),
    }
}

/// Dispatch methods on Size values.
/// Size uses a u64 carrier constrained to the non-negative i64 range.
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
        checked_size(bytes.checked_add(other), "size addition")
    } else if method == n.sub || method == n.subtract {
        require_args("sub", 1, args.len())?;
        let other = require_size_arg("sub", &args, 0)?;
        bytes
            .checked_sub(other)
            .map(Value::Size)
            .ok_or_else(|| size_would_be_negative().into())
    } else if method == n.mul || method == n.multiply {
        require_args("mul", 1, args.len())?;
        let scalar = require_int_arg("mul", &args, 0)?;
        if scalar < 0 {
            return Err(size_negative_multiply().into());
        }
        #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
        checked_size(bytes.checked_mul(scalar as u64), "size multiplication")
    } else if method == n.div || method == n.divide {
        require_args("div", 1, args.len())?;
        let scalar = require_int_arg("div", &args, 0)?;
        if scalar == 0 {
            return Err(division_by_zero().into());
        }
        if scalar < 0 {
            return Err(size_negative_divide().into());
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
    } else if method == n.to_str || method == n.debug {
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
    // Size predicates and conversion (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_size_method_str(bytes, method_str, &args)
    }
}

/// String-based dispatch for Size methods not hot enough to warrant
/// pre-interned Name fields.
fn dispatch_size_method_str(bytes: u64, method: &str, args: &[Value]) -> EvalResult {
    let to_int = |v: u64| -> EvalResult {
        i64::try_from(v)
            .map(Value::int)
            .map_err(|_| EvalError::new("size value too large for int").into())
    };

    match method {
        // Predicates
        "is_zero" => {
            require_args("is_zero", 0, args.len())?;
            Ok(Value::Bool(bytes == 0))
        }
        // Conversion accessors (as_bytes and to_bytes are aliases for bytes)
        "as_bytes" | "to_bytes" => {
            require_args(method, 0, args.len())?;
            to_int(bytes)
        }
        "to_kb" => {
            require_args("to_kb", 0, args.len())?;
            to_int(bytes / size::BYTES_PER_KB)
        }
        "to_mb" => {
            require_args("to_mb", 0, args.len())?;
            to_int(bytes / size::BYTES_PER_MB)
        }
        "to_gb" => {
            require_args("to_gb", 0, args.len())?;
            to_int(bytes / size::BYTES_PER_GB)
        }
        "to_tb" => {
            require_args("to_tb", 0, args.len())?;
            to_int(bytes / size::BYTES_PER_TB)
        }
        // format (Formattable)
        "format" => {
            require_args("format", 0, args.len())?;
            Ok(Value::string(format_size(bytes)))
        }
        // Associated functions routed through instance dispatch for test coverage
        "from_bytes" | "from_kb" | "from_kilobytes" | "from_mb" | "from_megabytes" | "from_gb"
        | "from_gigabytes" | "from_tb" | "from_terabytes" | "zero" | "default" => {
            dispatch_size_associated(method, args)
        }
        _ => Err(no_such_method(method, "Size").into()),
    }
}

/// Format a Size for Debug output. Same as Printable for Size.
pub(in crate::methods) fn format_size_debug(bytes: u64) -> String {
    format_size(bytes)
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
