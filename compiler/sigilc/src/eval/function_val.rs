//! Type conversion functions (function_val).
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
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Str(s) => s.parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("cannot parse '{}' as int", s)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
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
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Str(s) => s.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("cannot parse '{}' as float", s)),
        _ => Err(format!("cannot convert {} to float", args[0].type_name())),
    }
}

/// Returns the current OS thread ID as an integer.
/// Useful for verifying parallel execution.
pub fn function_val_thread_id(_args: &[Value]) -> Result<Value, String> {
    let thread_id = std::thread::current().id();
    let id_str = format!("{:?}", thread_id);
    let id_num = id_str
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse::<i64>()
        .unwrap_or(0);
    Ok(Value::Int(id_num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_val_str() {
        assert_eq!(
            function_val_str(&[Value::Int(42)]).unwrap(),
            Value::string("42")
        );
    }

    #[test]
    fn test_function_val_int() {
        assert_eq!(function_val_int(&[Value::Float(3.7)]).unwrap(), Value::Int(3));
        assert_eq!(function_val_int(&[Value::Bool(true)]).unwrap(), Value::Int(1));
        assert_eq!(
            function_val_int(&[Value::string("42")]).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn test_function_val_float() {
        assert_eq!(function_val_float(&[Value::Int(3)]).unwrap(), Value::Float(3.0));
    }

    #[test]
    fn test_function_val_str_various() {
        assert_eq!(function_val_str(&[Value::Bool(true)]).unwrap(), Value::string("true"));
        assert_eq!(function_val_str(&[Value::Float(3.14)]).unwrap(), Value::string("3.14"));
    }

    #[test]
    fn test_function_val_int_error() {
        let result = function_val_int(&[Value::string("not a number")]);
        assert!(result.is_err());
    }

    #[test]
    fn test_function_val_float_error() {
        let result = function_val_float(&[Value::string("not a number")]);
        assert!(result.is_err());
    }
}
