//! Type conversion functions (`function_val`).
//!
//! These are the built-in type conversion functions like `int(x)`, `str(x)`, `float(x)`
//! that allow positional arguments per the Ori spec.

use ori_patterns::{EvalError, Value};

/// Convert a value to string.
///
/// Uses `display_value()` for raw representation (char 'a' -> "a")
/// rather than Display which adds quotes (char 'a' -> "'a'").
pub fn function_val_str(args: &[Value]) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::new("str expects 1 argument"));
    }
    Ok(Value::string(args[0].display_value()))
}

/// Convert a value to int.
pub fn function_val_int(args: &[Value]) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::new("int expects 1 argument"));
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::int(n.raw())),
        Value::Float(f) => {
            if f.is_nan() {
                return Err(EvalError::new("cannot convert NaN to int"));
            }
            if f.is_infinite() {
                return Err(EvalError::new("cannot convert infinity to int"));
            }
            let truncated = f.trunc();
            // Check if the truncated value is within i64 range using exact f64 bounds:
            // - 2^63 is exactly representable in f64 and equals i64::MAX + 1
            // - -2^63 is exactly representable in f64 and equals i64::MIN
            // So valid range is: -2^63 <= truncated < 2^63
            let two_pow_63 = 2.0_f64.powi(63);
            if truncated >= two_pow_63 || truncated < -two_pow_63 {
                return Err(EvalError::new(format!("float {f} out of range for int")));
            }
            // Safety: truncated is a whole number (from trunc()) within
            // i64 range (verified by bounds check above).
            #[expect(
                clippy::cast_possible_truncation,
                reason = "bounds-checked: -2^63 <= truncated < 2^63"
            )]
            Ok(Value::int(truncated as i64))
        }
        Value::Str(s) => s
            .parse::<i64>()
            .map(Value::int)
            .map_err(|_| EvalError::new(format!("cannot parse '{s}' as int"))),
        Value::Bool(b) => Ok(Value::int(i64::from(*b))),
        _ => Err(EvalError::new(format!(
            "cannot convert {} to int",
            args[0].type_name()
        ))),
    }
}

/// Convert a value to float.
pub fn function_val_float(args: &[Value]) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::new("float expects 1 argument"));
    }
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Int(n) => {
            let raw = n.raw();
            // Use i32 for lossless f64 conversion when possible
            if let Ok(i32_val) = i32::try_from(raw) {
                Ok(Value::Float(f64::from(i32_val)))
            } else {
                // Direct cast: precision loss is inherent to i64→f64 (i64 has 64 bits,
                // f64 mantissa has 53). Same behavior as the integer literal → f64 path.
                #[expect(clippy::cast_precision_loss, reason = "intentional i64→f64 conversion")]
                Ok(Value::Float(raw as f64))
            }
        }
        Value::Str(s) => s
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| EvalError::new(format!("cannot parse '{s}' as float"))),
        _ => Err(EvalError::new(format!(
            "cannot convert {} to float",
            args[0].type_name()
        ))),
    }
}

/// Convert a value to byte.
pub fn function_val_byte(args: &[Value]) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::new("byte expects 1 argument"));
    }
    match &args[0] {
        Value::Int(n) => u8::try_from(n.raw())
            .map(Value::Byte)
            .map_err(|_| EvalError::new(format!("byte value {} out of range (0-255)", n.raw()))),
        Value::Byte(b) => Ok(Value::Byte(*b)),
        Value::Char(c) => u8::try_from(u32::from(*c))
            .map(Value::Byte)
            .map_err(|_| EvalError::new(format!("cannot convert non-ASCII char '{c}' to byte"))),
        _ => Err(EvalError::new(format!(
            "cannot convert {} to byte",
            args[0].type_name()
        ))),
    }
}

/// Create an infinite iterator that yields the same value on every `next()`.
///
/// Usage: `repeat(value: 42).take(count: 5).collect()` → `[42, 42, 42, 42, 42]`
pub fn function_val_repeat(args: &[Value]) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::new("repeat expects 1 argument"));
    }
    Ok(Value::iterator(ori_patterns::IteratorValue::from_repeat(
        args[0].clone(),
    )))
}

/// Returns the current OS thread ID as an integer.
/// Useful for verifying parallel execution.
///
/// Parses the Debug format of `ThreadId` because `ThreadId::as_u64()` is
/// still unstable (`#![feature(thread_id_value)]`, tracking issue #67939).
/// Replace with `.id().as_u64().get()` once stabilized.
pub fn function_val_thread_id(args: &[Value]) -> Result<Value, EvalError> {
    if !args.is_empty() {
        return Err(EvalError::new("thread_id expects 0 arguments"));
    }
    let thread_id = std::thread::current().id();
    let id_str = format!("{thread_id:?}");
    let id_num = id_str
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse::<i64>()
        .map_err(|_| EvalError::new("failed to parse thread id"))?;
    Ok(Value::int(id_num))
}
