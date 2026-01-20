// Builtin functions and method calls

use super::value::{Environment, Value};

/// Evaluate a builtin function call
/// Returns Ok(Some(value)) if handled, Ok(None) if not a builtin
pub fn eval_builtin(name: &str, args: &[Value]) -> Result<Option<Value>, String> {
    match name {
        "print" => {
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{}", arg);
            }
            println!();
            Ok(Some(Value::Nil))
        }
        "str" => {
            if args.len() != 1 {
                return Err("str() takes 1 argument".to_string());
            }
            Ok(Some(Value::String(format!("{}", args[0]))))
        }
        "int" => {
            if args.len() != 1 {
                return Err("int() takes 1 argument".to_string());
            }
            match &args[0] {
                Value::Int(n) => Ok(Some(Value::Int(*n))),
                Value::Float(f) => Ok(Some(Value::Int(*f as i64))),
                Value::String(s) => s
                    .parse::<i64>()
                    .map(|n| Some(Value::Int(n)))
                    .map_err(|_| "Cannot parse as int".to_string()),
                _ => Err("Cannot convert to int".to_string()),
            }
        }
        "len" => {
            if args.len() != 1 {
                return Err("len() takes 1 argument".to_string());
            }
            match &args[0] {
                Value::String(s) => Ok(Some(Value::Int(s.len() as i64))),
                Value::List(l) => Ok(Some(Value::Int(l.len() as i64))),
                _ => Err("Cannot get length".to_string()),
            }
        }
        "assert" => {
            if args.len() != 1 {
                return Err("assert() takes 1 argument".to_string());
            }
            match &args[0] {
                Value::Bool(true) => Ok(Some(Value::Nil)),
                Value::Bool(false) => Err("Assertion failed".to_string()),
                other => Err(format!("assert() expected bool, got {}", other)),
            }
        }
        "assert_eq" => {
            if args.len() != 2 {
                return Err("assert_eq() takes 2 arguments".to_string());
            }
            if args[0] == args[1] {
                Ok(Some(Value::Nil))
            } else {
                Err(format!("Assertion failed: {} != {}", args[0], args[1]))
            }
        }
        "assert_err" => {
            if args.len() != 1 {
                return Err("assert_err() takes 1 argument".to_string());
            }
            match &args[0] {
                Value::Err(_) => Ok(Some(Value::Nil)),
                other => Err(format!("Expected Err, got {}", other)),
            }
        }
        _ => Ok(None),
    }
}

/// Evaluate a method call on a value
pub fn eval_method_call(
    recv: &Value,
    method: &str,
    args: Vec<Value>,
    _env: &Environment,
) -> Result<Value, String> {
    match (recv, method) {
        // String methods
        (Value::String(s), "len") => Ok(Value::Int(s.len() as i64)),
        (Value::String(s), "upper") => Ok(Value::String(s.to_uppercase())),
        (Value::String(s), "lower") => Ok(Value::String(s.to_lowercase())),
        (Value::String(s), "contains") => {
            if let Some(Value::String(sub)) = args.first() {
                Ok(Value::Bool(s.contains(sub.as_str())))
            } else {
                Err("contains() requires a string argument".to_string())
            }
        }
        (Value::String(s), "split") => {
            if let Some(Value::String(sep)) = args.first() {
                let parts: Vec<Value> = s
                    .split(sep.as_str())
                    .map(|p| Value::String(p.to_string()))
                    .collect();
                Ok(Value::List(parts))
            } else {
                Err("split() requires a string argument".to_string())
            }
        }
        (Value::String(s), "slice") => {
            if args.len() != 2 {
                return Err("slice() takes 2 arguments".to_string());
            }
            let start = match &args[0] {
                Value::Int(n) => *n as usize,
                _ => return Err("slice start must be int".to_string()),
            };
            let end = match &args[1] {
                Value::Int(n) => *n as usize,
                _ => return Err("slice end must be int".to_string()),
            };
            let end = end.min(s.len());
            let start = start.min(end);
            Ok(Value::String(s[start..end].to_string()))
        }

        // List methods
        (Value::List(items), "len") => Ok(Value::Int(items.len() as i64)),
        (Value::List(items), "join") => {
            if let Some(Value::String(sep)) = args.first() {
                let s: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
                Ok(Value::String(s.join(sep)))
            } else {
                Err("join() requires a string argument".to_string())
            }
        }
        (Value::List(items), "first") => Ok(items
            .first()
            .cloned()
            .map(|v| Value::Some(Box::new(v)))
            .unwrap_or(Value::None_)),
        (Value::List(items), "last") => Ok(items
            .last()
            .cloned()
            .map(|v| Value::Some(Box::new(v)))
            .unwrap_or(Value::None_)),
        (Value::List(items), "push") => {
            let mut new_items = items.clone();
            for arg in args {
                new_items.push(arg);
            }
            Ok(Value::List(new_items))
        }
        (Value::List(items), "pop") => {
            if items.is_empty() {
                Ok(Value::List(vec![]))
            } else {
                let new_items: Vec<_> = items[..items.len() - 1].to_vec();
                Ok(Value::List(new_items))
            }
        }
        (Value::List(items), "slice") => {
            if args.len() != 2 {
                return Err("slice() takes 2 arguments".to_string());
            }
            let start = match &args[0] {
                Value::Int(n) => *n as usize,
                _ => return Err("slice start must be int".to_string()),
            };
            let end = match &args[1] {
                Value::Int(n) => *n as usize,
                _ => return Err("slice end must be int".to_string()),
            };
            let end = end.min(items.len());
            let start = start.min(end);
            Ok(Value::List(items[start..end].to_vec()))
        }

        // Result/Option methods
        (Value::Ok(inner), "unwrap") => Ok(*inner.clone()),
        (Value::Some(inner), "unwrap") => Ok(*inner.clone()),

        _ => Err(format!("Unknown method: {}", method)),
    }
}
