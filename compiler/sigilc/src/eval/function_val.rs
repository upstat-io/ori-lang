//! Type conversion functions (`function_val`).
//!
//! These are the built-in type conversion functions like `int(x)`, `str(x)`, `float(x)`
//! that allow positional arguments per the Sigil spec.

use super::Value;

/// Convert a value to string.
pub fn function_val_str(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("str expects 1 argument".to_string());
    }
    Ok(Value::string(format!("{}", args[0])))
}

/// Convert a value to int.
pub fn function_val_int(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("int expects 1 argument".to_string());
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Float(f) => {
            if f.is_nan() {
                return Err("cannot convert NaN to int".to_string());
            }
            if f.is_infinite() {
                return Err("cannot convert infinity to int".to_string());
            }
            let truncated = f.trunc();
            // Check if the truncated value is within i64 range using exact f64 bounds:
            // - 2^63 is exactly representable in f64 and equals i64::MAX + 1
            // - -2^63 is exactly representable in f64 and equals i64::MIN
            // So valid range is: -2^63 <= truncated < 2^63
            let two_pow_63 = 2.0_f64.powi(63);
            if truncated >= two_pow_63 || truncated < -two_pow_63 {
                return Err(format!("float {f} out of range for int"));
            }
            // Convert using string parsing to avoid cast truncation warning
            // This is safe because truncated is an integer value (from trunc())
            // within i64 range (verified by bounds check)
            let int_str = format!("{truncated:.0}");
            int_str.parse::<i64>()
                .map(Value::Int)
                .map_err(|_| format!("float {f} out of range for int"))
        }
        Value::Str(s) => s.parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("cannot parse '{s}' as int")),
        Value::Bool(b) => Ok(Value::Int(i64::from(*b))),
        _ => Err(format!("cannot convert {} to int", args[0].type_name())),
    }
}

/// Convert a value to float.
pub fn function_val_float(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("float expects 1 argument".to_string());
    }
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Int(n) => {
            // Use i32 for lossless f64 conversion when possible
            if let Ok(i32_val) = i32::try_from(*n) {
                Ok(Value::Float(f64::from(i32_val)))
            } else {
                // For larger values, use string parsing to avoid cast warning
                // This matches "as f64" rounding behavior within f64's precision
                Ok(Value::Float(format!("{n}").parse().unwrap_or(f64::NAN)))
            }
        }
        Value::Str(s) => s.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("cannot parse '{s}' as float")),
        _ => Err(format!("cannot convert {} to float", args[0].type_name())),
    }
}

/// Convert a value to byte.
pub fn function_val_byte(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("byte expects 1 argument".to_string());
    }
    match &args[0] {
        Value::Int(n) => {
            u8::try_from(*n)
                .map(Value::Byte)
                .map_err(|_| format!("byte value {n} out of range (0-255)"))
        }
        Value::Byte(b) => Ok(Value::Byte(*b)),
        Value::Char(c) => {
            u8::try_from(u32::from(*c))
                .map(Value::Byte)
                .map_err(|_| format!("cannot convert non-ASCII char '{c}' to byte"))
        }
        _ => Err(format!("cannot convert {} to byte", args[0].type_name())),
    }
}

/// Returns the current OS thread ID as an integer.
/// Useful for verifying parallel execution.
pub fn function_val_thread_id(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("thread_id expects 0 arguments".to_string());
    }
    let thread_id = std::thread::current().id();
    let id_str = format!("{thread_id:?}");
    let id_num = id_str
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse::<i64>()
        .map_err(|_| "failed to parse thread id".to_string())?;
    Ok(Value::Int(id_num))
}
