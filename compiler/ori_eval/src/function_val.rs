//! Type conversion functions (`function_val`).
//!
//! These are the built-in type conversion functions like `int(x)`, `str(x)`, `float(x)`
//! that allow positional arguments per the Ori spec.

use ori_patterns::Value;

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
        Value::Int(n) => Ok(Value::int(n.raw())),
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
            int_str
                .parse::<i64>()
                .map(Value::int)
                .map_err(|_| format!("float {f} out of range for int"))
        }
        Value::Str(s) => s
            .parse::<i64>()
            .map(Value::int)
            .map_err(|_| format!("cannot parse '{s}' as int")),
        Value::Bool(b) => Ok(Value::int(i64::from(*b))),
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
            let raw = n.raw();
            // Use i32 for lossless f64 conversion when possible
            if let Ok(i32_val) = i32::try_from(raw) {
                Ok(Value::Float(f64::from(i32_val)))
            } else {
                // For larger values, use string parsing to avoid cast warning
                // This matches "as f64" rounding behavior within f64's precision
                Ok(Value::Float(format!("{raw}").parse().unwrap_or(f64::NAN)))
            }
        }
        Value::Str(s) => s
            .parse::<f64>()
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
        Value::Int(n) => u8::try_from(n.raw())
            .map(Value::Byte)
            .map_err(|_| format!("byte value {} out of range (0-255)", n.raw())),
        Value::Byte(b) => Ok(Value::Byte(*b)),
        Value::Char(c) => u8::try_from(u32::from(*c))
            .map(Value::Byte)
            .map_err(|_| format!("cannot convert non-ASCII char '{c}' to byte")),
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
    Ok(Value::int(id_num))
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    mod int_conversion {
        use super::*;

        #[test]
        fn int_from_float_basic() {
            assert_eq!(
                function_val_int(&[Value::Float(3.7)]).unwrap(),
                Value::int(3)
            );
            assert_eq!(
                function_val_int(&[Value::Float(3.2)]).unwrap(),
                Value::int(3)
            );
            assert_eq!(
                function_val_int(&[Value::Float(-3.7)]).unwrap(),
                Value::int(-3)
            );
            assert_eq!(
                function_val_int(&[Value::Float(-3.2)]).unwrap(),
                Value::int(-3)
            );
        }

        #[test]
        fn int_from_float_whole_numbers() {
            assert_eq!(
                function_val_int(&[Value::Float(0.0)]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::Float(-0.0)]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::Float(1.0)]).unwrap(),
                Value::int(1)
            );
            assert_eq!(
                function_val_int(&[Value::Float(-1.0)]).unwrap(),
                Value::int(-1)
            );
            assert_eq!(
                function_val_int(&[Value::Float(42.0)]).unwrap(),
                Value::int(42)
            );
        }

        #[test]
        fn int_from_float_near_zero() {
            assert_eq!(
                function_val_int(&[Value::Float(0.1)]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::Float(0.9)]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::Float(-0.1)]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::Float(-0.9)]).unwrap(),
                Value::int(0)
            );
        }

        #[test]
        fn int_from_float_nan_error() {
            let result = function_val_int(&[Value::Float(f64::NAN)]);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("NaN"));
        }

        #[test]
        fn int_from_float_infinity_error() {
            let result = function_val_int(&[Value::Float(f64::INFINITY)]);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("infinity"));

            let result = function_val_int(&[Value::Float(f64::NEG_INFINITY)]);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("infinity"));
        }

        #[test]
        fn int_from_float_overflow_error() {
            let result = function_val_int(&[Value::Float(1e19)]);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("out of range"));
        }

        #[test]
        fn int_from_string_basic() {
            assert_eq!(
                function_val_int(&[Value::string("0")]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::string("1")]).unwrap(),
                Value::int(1)
            );
            assert_eq!(
                function_val_int(&[Value::string("-1")]).unwrap(),
                Value::int(-1)
            );
            assert_eq!(
                function_val_int(&[Value::string("12345")]).unwrap(),
                Value::int(12345)
            );
        }

        #[test]
        fn int_from_string_boundaries() {
            assert_eq!(
                function_val_int(&[Value::string("9223372036854775807")]).unwrap(),
                Value::int(i64::MAX)
            );
            assert_eq!(
                function_val_int(&[Value::string("-9223372036854775808")]).unwrap(),
                Value::int(i64::MIN)
            );
        }

        #[test]
        fn int_from_string_invalid_syntax() {
            assert!(function_val_int(&[Value::string("")]).is_err());
            assert!(function_val_int(&[Value::string("abc")]).is_err());
            assert!(function_val_int(&[Value::string("123abc")]).is_err());
            assert!(function_val_int(&[Value::string("12.34")]).is_err());
        }

        #[test]
        fn int_from_bool() {
            assert_eq!(
                function_val_int(&[Value::Bool(true)]).unwrap(),
                Value::int(1)
            );
            assert_eq!(
                function_val_int(&[Value::Bool(false)]).unwrap(),
                Value::int(0)
            );
        }

        #[test]
        fn int_from_int_identity() {
            assert_eq!(
                function_val_int(&[Value::int(0)]).unwrap(),
                Value::int(0)
            );
            assert_eq!(
                function_val_int(&[Value::int(42)]).unwrap(),
                Value::int(42)
            );
            assert_eq!(
                function_val_int(&[Value::int(i64::MAX)]).unwrap(),
                Value::int(i64::MAX)
            );
            assert_eq!(
                function_val_int(&[Value::int(i64::MIN)]).unwrap(),
                Value::int(i64::MIN)
            );
        }

        #[test]
        fn int_wrong_arg_count() {
            assert!(function_val_int(&[]).is_err());
            assert!(function_val_int(&[Value::int(1), Value::int(2)]).is_err());
        }

        #[test]
        fn int_from_invalid_type() {
            assert!(function_val_int(&[Value::list(vec![])]).is_err());
            assert!(function_val_int(&[Value::Void]).is_err());
        }
    }

    mod float_conversion {
        use super::*;

        #[test]
        fn float_from_int_basic() {
            assert_eq!(
                function_val_float(&[Value::int(0)]).unwrap(),
                Value::Float(0.0)
            );
            assert_eq!(
                function_val_float(&[Value::int(1)]).unwrap(),
                Value::Float(1.0)
            );
            assert_eq!(
                function_val_float(&[Value::int(-1)]).unwrap(),
                Value::Float(-1.0)
            );
            assert_eq!(
                function_val_float(&[Value::int(42)]).unwrap(),
                Value::Float(42.0)
            );
        }

        #[test]
        fn float_from_int_i32_boundaries() {
            assert_eq!(
                function_val_float(&[Value::int(i64::from(i32::MAX))]).unwrap(),
                Value::Float(f64::from(i32::MAX))
            );
            assert_eq!(
                function_val_float(&[Value::int(i64::from(i32::MIN))]).unwrap(),
                Value::Float(f64::from(i32::MIN))
            );
        }

        #[test]
        fn float_from_string_basic() {
            assert_eq!(
                function_val_float(&[Value::string("0")]).unwrap(),
                Value::Float(0.0)
            );
            assert_eq!(
                function_val_float(&[Value::string("1")]).unwrap(),
                Value::Float(1.0)
            );
            assert_eq!(
                function_val_float(&[Value::string("1.5")]).unwrap(),
                Value::Float(1.5)
            );
        }

        #[test]
        fn float_from_string_scientific() {
            assert_eq!(
                function_val_float(&[Value::string("1e10")]).unwrap(),
                Value::Float(1e10)
            );
            assert_eq!(
                function_val_float(&[Value::string("1E10")]).unwrap(),
                Value::Float(1e10)
            );
            assert_eq!(
                function_val_float(&[Value::string("1e-10")]).unwrap(),
                Value::Float(1e-10)
            );
        }

        #[test]
        fn float_from_string_invalid_syntax() {
            assert!(function_val_float(&[Value::string("")]).is_err());
            assert!(function_val_float(&[Value::string("abc")]).is_err());
            assert!(function_val_float(&[Value::string("1.2.3")]).is_err());
        }

        #[test]
        #[expect(clippy::approx_constant, reason = "Testing float operations, not using PI")]
        fn float_from_float_identity() {
            assert_eq!(
                function_val_float(&[Value::Float(0.0)]).unwrap(),
                Value::Float(0.0)
            );
            assert_eq!(
                function_val_float(&[Value::Float(3.14)]).unwrap(),
                Value::Float(3.14)
            );
        }

        #[test]
        fn float_wrong_arg_count() {
            assert!(function_val_float(&[]).is_err());
            assert!(function_val_float(&[Value::int(1), Value::int(2)]).is_err());
        }

        #[test]
        fn float_from_invalid_type() {
            assert!(function_val_float(&[Value::Bool(true)]).is_err());
            assert!(function_val_float(&[Value::list(vec![])]).is_err());
        }
    }

    mod str_conversion {
        use super::*;

        #[test]
        fn str_from_int() {
            assert_eq!(
                function_val_str(&[Value::int(0)]).unwrap(),
                Value::string("0")
            );
            assert_eq!(
                function_val_str(&[Value::int(1)]).unwrap(),
                Value::string("1")
            );
            assert_eq!(
                function_val_str(&[Value::int(-1)]).unwrap(),
                Value::string("-1")
            );
            assert_eq!(
                function_val_str(&[Value::int(42)]).unwrap(),
                Value::string("42")
            );
        }

        #[test]
        #[expect(clippy::approx_constant, reason = "Testing float operations, not using PI")]
        fn str_from_float() {
            assert_eq!(
                function_val_str(&[Value::Float(0.0)]).unwrap(),
                Value::string("0")
            );
            assert_eq!(
                function_val_str(&[Value::Float(3.14)]).unwrap(),
                Value::string("3.14")
            );
        }

        #[test]
        fn str_from_bool() {
            assert_eq!(
                function_val_str(&[Value::Bool(true)]).unwrap(),
                Value::string("true")
            );
            assert_eq!(
                function_val_str(&[Value::Bool(false)]).unwrap(),
                Value::string("false")
            );
        }

        #[test]
        fn str_wrong_arg_count() {
            assert!(function_val_str(&[]).is_err());
            assert!(function_val_str(&[Value::int(1), Value::int(2)]).is_err());
        }
    }

    mod byte_conversion {
        use super::*;

        #[test]
        fn byte_from_int_valid_range() {
            assert_eq!(
                function_val_byte(&[Value::int(0)]).unwrap(),
                Value::Byte(0)
            );
            assert_eq!(
                function_val_byte(&[Value::int(1)]).unwrap(),
                Value::Byte(1)
            );
            assert_eq!(
                function_val_byte(&[Value::int(127)]).unwrap(),
                Value::Byte(127)
            );
            assert_eq!(
                function_val_byte(&[Value::int(255)]).unwrap(),
                Value::Byte(255)
            );
        }

        #[test]
        fn byte_from_int_out_of_range() {
            assert!(function_val_byte(&[Value::int(-1)]).is_err());
            assert!(function_val_byte(&[Value::int(256)]).is_err());
            assert!(function_val_byte(&[Value::int(1000)]).is_err());
        }

        #[test]
        fn byte_from_byte_identity() {
            assert_eq!(
                function_val_byte(&[Value::Byte(0)]).unwrap(),
                Value::Byte(0)
            );
            assert_eq!(
                function_val_byte(&[Value::Byte(255)]).unwrap(),
                Value::Byte(255)
            );
        }

        #[test]
        fn byte_from_char_ascii() {
            assert_eq!(
                function_val_byte(&[Value::Char('a')]).unwrap(),
                Value::Byte(97)
            );
            assert_eq!(
                function_val_byte(&[Value::Char('A')]).unwrap(),
                Value::Byte(65)
            );
            assert_eq!(
                function_val_byte(&[Value::Char('0')]).unwrap(),
                Value::Byte(48)
            );
        }

        #[test]
        fn byte_from_char_non_ascii_error() {
            assert!(function_val_byte(&[Value::Char('λ')]).is_err());
            assert!(function_val_byte(&[Value::Char('中')]).is_err());
        }

        #[test]
        fn byte_wrong_arg_count() {
            assert!(function_val_byte(&[]).is_err());
            assert!(function_val_byte(&[Value::int(1), Value::int(2)]).is_err());
        }

        #[test]
        fn byte_from_invalid_type() {
            assert!(function_val_byte(&[Value::Float(1.0)]).is_err());
            assert!(function_val_byte(&[Value::string("1")]).is_err());
            assert!(function_val_byte(&[Value::Bool(true)]).is_err());
        }
    }
}
