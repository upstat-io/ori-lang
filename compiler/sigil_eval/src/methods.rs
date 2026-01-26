//! Method dispatch implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for built-in method calls. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use sigil_patterns::{no_such_method, wrong_arg_count, wrong_arg_type, EvalError, EvalResult, Value};

// Argument Validation Helpers

/// Validate expected argument count.
#[inline]
fn require_args(method: &str, expected: usize, actual: usize) -> Result<(), EvalError> {
    if actual != expected {
        Err(wrong_arg_count(method, expected, actual))
    } else {
        Ok(())
    }
}

/// Extract a string argument at the given index.
#[inline]
fn require_str_arg<'a>(method: &str, args: &'a [Value], index: usize) -> Result<&'a str, EvalError> {
    match args.get(index) {
        Some(Value::Str(s)) => Ok(s.as_str()),
        _ => Err(wrong_arg_type(method, "string")),
    }
}

/// Extract an integer argument at the given index.
#[inline]
fn require_int_arg(method: &str, args: &[Value], index: usize) -> Result<i64, EvalError> {
    match args.get(index) {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err(wrong_arg_type(method, "int")),
    }
}

/// Convert a collection length to a Value, with overflow check.
#[inline]
fn len_to_value(len: usize, collection_type: &str) -> EvalResult {
    i64::try_from(len)
        .map(Value::Int)
        .map_err(|_| EvalError::new(format!("{collection_type} too large")))
}

// Direct Dispatch Function

/// Dispatch a built-in method call using direct pattern matching.
///
/// This is the preferred entry point for built-in method calls. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
pub fn dispatch_builtin_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match &receiver {
        Value::List(_) => dispatch_list_method(receiver, method, args),
        Value::Str(_) => dispatch_string_method(receiver, method, args),
        Value::Map(_) => dispatch_map_method(receiver, method, args),
        Value::Range(_) => dispatch_range_method(receiver, method, args),
        Value::Some(_) | Value::None => dispatch_option_method(receiver, method, args),
        Value::Ok(_) | Value::Err(_) => dispatch_result_method(receiver, method, args),
        _ => Err(no_such_method(method, receiver.type_name())),
    }
}

// Type-Specific Dispatch Functions

/// Dispatch methods on list values.
fn dispatch_list_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::List(items) = receiver else {
        unreachable!();
    };

    match method {
        "len" => len_to_value(items.len(), "list"),
        "is_empty" => Ok(Value::Bool(items.is_empty())),
        "first" => Ok(items.first().cloned().map_or(Value::None, Value::some)),
        "last" => Ok(items.last().cloned().map_or(Value::None, Value::some)),
        "contains" => {
            require_args("contains", 1, args.len())?;
            Ok(Value::Bool(items.contains(&args[0])))
        }
        _ => Err(no_such_method(method, "list")),
    }
}

/// Dispatch methods on string values.
fn dispatch_string_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Str(s) = receiver else {
        unreachable!();
    };

    match method {
        "len" => len_to_value(s.len(), "string"),
        "is_empty" => Ok(Value::Bool(s.is_empty())),
        "to_uppercase" => Ok(Value::string(s.to_uppercase())),
        "to_lowercase" => Ok(Value::string(s.to_lowercase())),
        "trim" => Ok(Value::string(s.trim().to_string())),
        "contains" => {
            require_args("contains", 1, args.len())?;
            let needle = require_str_arg("contains", &args, 0)?;
            Ok(Value::Bool(s.contains(needle)))
        }
        "starts_with" => {
            require_args("starts_with", 1, args.len())?;
            let prefix = require_str_arg("starts_with", &args, 0)?;
            Ok(Value::Bool(s.starts_with(prefix)))
        }
        "ends_with" => {
            require_args("ends_with", 1, args.len())?;
            let suffix = require_str_arg("ends_with", &args, 0)?;
            Ok(Value::Bool(s.ends_with(suffix)))
        }
        _ => Err(no_such_method(method, "str")),
    }
}

/// Dispatch methods on range values.
fn dispatch_range_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Range(r) = receiver else {
        unreachable!();
    };

    match method {
        "len" => len_to_value(r.len(), "range"),
        "contains" => {
            require_args("contains", 1, args.len())?;
            let n = require_int_arg("contains", &args, 0)?;
            Ok(Value::Bool(r.contains(n)))
        }
        _ => Err(no_such_method(method, "range")),
    }
}

/// Dispatch methods on map values.
fn dispatch_map_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Map(map) = receiver else {
        unreachable!();
    };

    match method {
        "len" => len_to_value(map.len(), "map"),
        "is_empty" => Ok(Value::Bool(map.is_empty())),
        "contains_key" => {
            require_args("contains_key", 1, args.len())?;
            let key = require_str_arg("contains_key", &args, 0)?;
            Ok(Value::Bool(map.contains_key(key)))
        }
        "keys" => {
            let keys: Vec<Value> = map.keys().map(|k| Value::string(k.clone())).collect();
            Ok(Value::list(keys))
        }
        "values" => {
            let values: Vec<Value> = map.values().cloned().collect();
            Ok(Value::list(values))
        }
        _ => Err(no_such_method(method, "map")),
    }
}

/// Dispatch methods on Option values.
fn dispatch_option_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match (method, &receiver) {
        ("unwrap" | "unwrap_or", Value::Some(v)) => Ok((**v).clone()),
        ("unwrap", Value::None) => Err(EvalError::new("called unwrap on None")),
        ("is_some", Value::Some(_)) | ("is_none", Value::None) => Ok(Value::Bool(true)),
        ("is_some", Value::None) | ("is_none", Value::Some(_)) => Ok(Value::Bool(false)),
        ("unwrap_or", Value::None) => {
            require_args("unwrap_or", 1, args.len())?;
            match args.into_iter().next() {
                Some(default) => Ok(default),
                None => unreachable!("require_args verified length is 1"),
            }
        }
        _ => Err(no_such_method(method, "Option")),
    }
}

/// Dispatch methods on Result values.
fn dispatch_result_method(receiver: Value, method: &str, _args: Vec<Value>) -> EvalResult {
    match (method, &receiver) {
        ("unwrap", Value::Ok(v)) => Ok((**v).clone()),
        ("unwrap", Value::Err(e)) => {
            Err(EvalError::new(format!("called unwrap on Err: {e:?}")))
        }
        ("is_ok", Value::Ok(_)) | ("is_err", Value::Err(_)) => Ok(Value::Bool(true)),
        ("is_ok", Value::Err(_)) | ("is_err", Value::Ok(_)) => Ok(Value::Bool(false)),
        _ => Err(no_such_method(method, "Result")),
    }
}

// Legacy Method Dispatcher (Backward Compatibility)

/// Trait for handling method calls on values (deprecated).
///
/// Use [`dispatch_builtin_method`] directly instead. This trait is preserved
/// for backward compatibility with user-defined method dispatchers.
pub trait MethodDispatcher: Send + Sync {
    /// The type name this dispatcher handles (e.g., "list", "str").
    fn type_name(&self) -> &'static str;

    /// Check if this dispatcher has the given method.
    fn has_method(&self, method: &str) -> bool;

    /// Dispatch a method call.
    ///
    /// Only called if `has_method` returns true.
    fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult;
}

/// Registry of method dispatchers (deprecated).
///
/// Use [`dispatch_builtin_method`] directly instead. This type is preserved for
/// backward compatibility but delegates to `dispatch_builtin_method` internally.
#[deprecated(note = "Use dispatch_builtin_method() directly")]
pub struct MethodRegistry;

#[allow(deprecated)]
impl MethodRegistry {
    /// Create a new method registry.
    pub fn new() -> Self {
        MethodRegistry
    }

    /// Dispatch a method call.
    pub fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        dispatch_builtin_method(receiver, method, args)
    }
}

#[allow(deprecated)]
impl Default for MethodRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use sigil_patterns::RangeValue;

    mod list_methods {
        use super::*;

        #[test]
        fn len() {
            assert_eq!(
                dispatch_builtin_method(
                    Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
                    "len",
                    vec![]
                )
                .unwrap(),
                Value::Int(3)
            );
            assert_eq!(
                dispatch_builtin_method(Value::list(vec![]), "len", vec![]).unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn is_empty() {
            assert_eq!(
                dispatch_builtin_method(Value::list(vec![]), "is_empty", vec![]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(Value::list(vec![Value::Int(1)]), "is_empty", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn first() {
            let result = dispatch_builtin_method(
                Value::list(vec![Value::Int(1), Value::Int(2)]),
                "first",
                vec![],
            )
            .unwrap();
            assert_eq!(result, Value::some(Value::Int(1)));

            let result =
                dispatch_builtin_method(Value::list(vec![]), "first", vec![]).unwrap();
            assert_eq!(result, Value::None);
        }

        #[test]
        fn last() {
            let result = dispatch_builtin_method(
                Value::list(vec![Value::Int(1), Value::Int(2)]),
                "last",
                vec![],
            )
            .unwrap();
            assert_eq!(result, Value::some(Value::Int(2)));

            let result =
                dispatch_builtin_method(Value::list(vec![]), "last", vec![]).unwrap();
            assert_eq!(result, Value::None);
        }

        #[test]
        fn contains() {
            let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

            assert_eq!(
                dispatch_builtin_method(list.clone(), "contains", vec![Value::Int(2)]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(list, "contains", vec![Value::Int(5)]).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn contains_wrong_arg_count() {
            let list = Value::list(vec![Value::Int(1)]);

            assert!(dispatch_builtin_method(list.clone(), "contains", vec![]).is_err());
            assert!(dispatch_builtin_method(
                list,
                "contains",
                vec![Value::Int(1), Value::Int(2)]
            )
            .is_err());
        }
    }

    mod string_methods {
        use super::*;

        #[test]
        fn len() {
            assert_eq!(
                dispatch_builtin_method(Value::string("hello"), "len", vec![]).unwrap(),
                Value::Int(5)
            );
            assert_eq!(
                dispatch_builtin_method(Value::string(""), "len", vec![]).unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn is_empty() {
            assert_eq!(
                dispatch_builtin_method(Value::string(""), "is_empty", vec![]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(Value::string("hello"), "is_empty", vec![]).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn to_uppercase() {
            assert_eq!(
                dispatch_builtin_method(Value::string("hello"), "to_uppercase", vec![]).unwrap(),
                Value::string("HELLO")
            );
        }

        #[test]
        fn to_lowercase() {
            assert_eq!(
                dispatch_builtin_method(Value::string("HELLO"), "to_lowercase", vec![]).unwrap(),
                Value::string("hello")
            );
        }

        #[test]
        fn trim() {
            assert_eq!(
                dispatch_builtin_method(Value::string("  hello  "), "trim", vec![]).unwrap(),
                Value::string("hello")
            );
        }

        #[test]
        fn contains() {
            assert_eq!(
                dispatch_builtin_method(
                    Value::string("hello world"),
                    "contains",
                    vec![Value::string("world")]
                )
                .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(
                    Value::string("hello"),
                    "contains",
                    vec![Value::string("xyz")]
                )
                .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn starts_with() {
            assert_eq!(
                dispatch_builtin_method(
                    Value::string("hello world"),
                    "starts_with",
                    vec![Value::string("hello")]
                )
                .unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn ends_with() {
            assert_eq!(
                dispatch_builtin_method(
                    Value::string("hello world"),
                    "ends_with",
                    vec![Value::string("world")]
                )
                .unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn wrong_arg_type() {
            assert!(dispatch_builtin_method(
                Value::string("hello"),
                "contains",
                vec![Value::Int(1)]
            )
            .is_err());
        }
    }

    mod range_methods {
        use super::*;

        #[test]
        fn len() {
            assert_eq!(
                dispatch_builtin_method(
                    Value::Range(RangeValue::exclusive(0, 10)),
                    "len",
                    vec![]
                )
                .unwrap(),
                Value::Int(10)
            );
            assert_eq!(
                dispatch_builtin_method(
                    Value::Range(RangeValue::inclusive(0, 10)),
                    "len",
                    vec![]
                )
                .unwrap(),
                Value::Int(11)
            );
        }

        #[test]
        fn contains() {
            let range = Value::Range(RangeValue::exclusive(0, 10));

            assert_eq!(
                dispatch_builtin_method(range.clone(), "contains", vec![Value::Int(5)]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(range, "contains", vec![Value::Int(10)]).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn contains_wrong_type() {
            let range = Value::Range(RangeValue::exclusive(0, 10));
            assert!(
                dispatch_builtin_method(range, "contains", vec![Value::string("5")]).is_err()
            );
        }
    }

    mod option_methods {
        use super::*;

        #[test]
        fn unwrap_some() {
            assert_eq!(
                dispatch_builtin_method(Value::some(Value::Int(42)), "unwrap", vec![]).unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn unwrap_none_error() {
            assert!(dispatch_builtin_method(Value::None, "unwrap", vec![]).is_err());
        }

        #[test]
        fn unwrap_or() {
            assert_eq!(
                dispatch_builtin_method(
                    Value::some(Value::Int(42)),
                    "unwrap_or",
                    vec![Value::Int(0)]
                )
                .unwrap(),
                Value::Int(42)
            );
            assert_eq!(
                dispatch_builtin_method(Value::None, "unwrap_or", vec![Value::Int(0)]).unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn is_some() {
            assert_eq!(
                dispatch_builtin_method(Value::some(Value::Int(1)), "is_some", vec![]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(Value::None, "is_some", vec![]).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn is_none() {
            assert_eq!(
                dispatch_builtin_method(Value::None, "is_none", vec![]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(Value::some(Value::Int(1)), "is_none", vec![]).unwrap(),
                Value::Bool(false)
            );
        }
    }

    mod result_methods {
        use super::*;

        #[test]
        fn unwrap_ok() {
            assert_eq!(
                dispatch_builtin_method(Value::ok(Value::Int(42)), "unwrap", vec![]).unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn unwrap_err_error() {
            assert!(dispatch_builtin_method(
                Value::err(Value::string("error")),
                "unwrap",
                vec![]
            )
            .is_err());
        }

        #[test]
        fn is_ok() {
            assert_eq!(
                dispatch_builtin_method(Value::ok(Value::Int(1)), "is_ok", vec![]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(Value::err(Value::string("e")), "is_ok", vec![]).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn is_err() {
            assert_eq!(
                dispatch_builtin_method(Value::err(Value::string("e")), "is_err", vec![]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                dispatch_builtin_method(Value::ok(Value::Int(1)), "is_err", vec![]).unwrap(),
                Value::Bool(false)
            );
        }
    }

    mod errors {
        use super::*;

        #[test]
        fn no_such_method() {
            assert!(
                dispatch_builtin_method(Value::list(vec![]), "nonexistent", vec![]).is_err()
            );
            assert!(
                dispatch_builtin_method(Value::string("hello"), "nonexistent", vec![]).is_err()
            );
            assert!(dispatch_builtin_method(Value::Int(42), "len", vec![]).is_err());
        }
    }
}
