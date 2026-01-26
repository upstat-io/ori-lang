//! Method dispatch implementations for the evaluator.
//!
//! This module extracts method call logic from the evaluator,
//! following the Open/Closed Principle. New methods can be added
//! by implementing the `MethodDispatcher` trait.

use std::collections::HashMap;
use sigil_patterns::{no_such_method, wrong_arg_count, wrong_arg_type, EvalError, EvalResult, Value};

// =============================================================================
// Method Dispatcher Trait
// =============================================================================

/// Trait for handling method calls on values.
///
/// Implementations handle methods for specific types.
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

// =============================================================================
// List Methods
// =============================================================================

/// Methods available on list values.
pub struct ListMethods;

impl MethodDispatcher for ListMethods {
    fn type_name(&self) -> &'static str {
        "list"
    }

    fn has_method(&self, method: &str) -> bool {
        matches!(method, "len" | "is_empty" | "first" | "last" | "contains")
    }

    fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        let Value::List(items) = receiver else {
            return Err(EvalError::new("expected list"));
        };

        match method {
            "len" => i64::try_from(items.len())
                .map(Value::Int)
                .map_err(|_| EvalError::new("collection too large")),
            "is_empty" => Ok(Value::Bool(items.is_empty())),
            "first" => Ok(items
                .first()
                .cloned()
                .map_or(Value::None, Value::some)),
            "last" => Ok(items
                .last()
                .cloned()
                .map_or(Value::None, Value::some)),
            "contains" => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("contains", 1, args.len()));
                }
                Ok(Value::Bool(items.contains(&args[0])))
            }
            _ => Err(no_such_method(method, "list")),
        }
    }
}

// =============================================================================
// String Methods
// =============================================================================

/// Methods available on string values.
pub struct StringMethods;

impl MethodDispatcher for StringMethods {
    fn type_name(&self) -> &'static str {
        "str"
    }

    fn has_method(&self, method: &str) -> bool {
        matches!(
            method,
            "len"
                | "is_empty"
                | "to_uppercase"
                | "to_lowercase"
                | "trim"
                | "contains"
                | "starts_with"
                | "ends_with"
        )
    }

    fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        let Value::Str(s) = receiver else {
            return Err(EvalError::new("expected string"));
        };

        match method {
            "len" => i64::try_from(s.len())
                .map(Value::Int)
                .map_err(|_| EvalError::new("string too large")),
            "is_empty" => Ok(Value::Bool(s.is_empty())),
            "to_uppercase" => Ok(Value::string(s.to_uppercase())),
            "to_lowercase" => Ok(Value::string(s.to_lowercase())),
            "trim" => Ok(Value::string(s.trim().to_string())),
            "contains" => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("contains", 1, args.len()));
                }
                if let Value::Str(needle) = &args[0] {
                    Ok(Value::Bool(s.contains(needle.as_str())))
                } else {
                    Err(wrong_arg_type("contains", "string"))
                }
            }
            "starts_with" => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("starts_with", 1, args.len()));
                }
                if let Value::Str(prefix) = &args[0] {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(wrong_arg_type("starts_with", "string"))
                }
            }
            "ends_with" => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("ends_with", 1, args.len()));
                }
                if let Value::Str(suffix) = &args[0] {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(wrong_arg_type("ends_with", "string"))
                }
            }
            _ => Err(no_such_method(method, "str")),
        }
    }
}

// =============================================================================
// Range Methods
// =============================================================================

/// Methods available on range values.
pub struct RangeMethods;

impl MethodDispatcher for RangeMethods {
    fn type_name(&self) -> &'static str {
        "range"
    }

    fn has_method(&self, method: &str) -> bool {
        matches!(method, "len" | "contains")
    }

    fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        let Value::Range(r) = receiver else {
            return Err(EvalError::new("expected range"));
        };

        match method {
            "len" => i64::try_from(r.len())
                .map(Value::Int)
                .map_err(|_| EvalError::new("range too large")),
            "contains" => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("contains", 1, args.len()));
                }
                if let Value::Int(n) = args[0] {
                    Ok(Value::Bool(r.contains(n)))
                } else {
                    Err(wrong_arg_type("contains", "int"))
                }
            }
            _ => Err(no_such_method(method, "range")),
        }
    }
}

// =============================================================================
// Map Methods
// =============================================================================

/// Methods available on map values.
pub struct MapMethods;

impl MethodDispatcher for MapMethods {
    fn type_name(&self) -> &'static str {
        "map"
    }

    fn has_method(&self, method: &str) -> bool {
        matches!(method, "len" | "is_empty" | "contains_key" | "keys" | "values")
    }

    fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        let Value::Map(map) = receiver else {
            return Err(EvalError::new("expected map"));
        };

        match method {
            "len" => i64::try_from(map.len())
                .map(Value::Int)
                .map_err(|_| EvalError::new("map too large")),
            "is_empty" => Ok(Value::Bool(map.is_empty())),
            "contains_key" => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("contains_key", 1, args.len()));
                }
                if let Value::Str(key) = &args[0] {
                    Ok(Value::Bool(map.contains_key(key.as_str())))
                } else {
                    Err(wrong_arg_type("contains_key", "string"))
                }
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
}

// =============================================================================
// Option Methods
// =============================================================================

/// Methods available on Option values.
pub struct OptionMethods;

impl MethodDispatcher for OptionMethods {
    fn type_name(&self) -> &'static str {
        "Option"
    }

    fn has_method(&self, method: &str) -> bool {
        matches!(method, "unwrap" | "is_some" | "is_none" | "unwrap_or")
    }

    fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        match (method, &receiver) {
            ("unwrap" | "unwrap_or", Value::Some(v)) => Ok((**v).clone()),
            ("unwrap", Value::None) => Err(EvalError::new("called unwrap on None")),
            ("is_some", Value::Some(_)) | ("is_none", Value::None) => Ok(Value::Bool(true)),
            ("is_some", Value::None) | ("is_none", Value::Some(_)) => Ok(Value::Bool(false)),
            ("unwrap_or", Value::None) => {
                if args.len() != 1 {
                    return Err(wrong_arg_count("unwrap_or", 1, args.len()));
                }
                Ok(args[0].clone())
            }
            _ => Err(no_such_method(method, "Option")),
        }
    }
}

// =============================================================================
// Result Methods
// =============================================================================

/// Methods available on Result values.
pub struct ResultMethods;

impl MethodDispatcher for ResultMethods {
    fn type_name(&self) -> &'static str {
        "Result"
    }

    fn has_method(&self, method: &str) -> bool {
        matches!(method, "unwrap" | "is_ok" | "is_err")
    }

    fn dispatch(&self, receiver: Value, method: &str, _args: Vec<Value>) -> EvalResult {
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
}

// =============================================================================
// Method Registry
// =============================================================================

/// Registry of method dispatchers.
///
/// Provides a way to dispatch method calls by delegating to registered dispatchers.
pub struct MethodRegistry {
    dispatchers: HashMap<&'static str, Box<dyn MethodDispatcher>>,
}

impl MethodRegistry {
    /// Create a new method registry with all built-in dispatchers.
    pub fn new() -> Self {
        let mut dispatchers: HashMap<&'static str, Box<dyn MethodDispatcher>> = HashMap::new();

        dispatchers.insert("list", Box::new(ListMethods));
        dispatchers.insert("str", Box::new(StringMethods));
        dispatchers.insert("map", Box::new(MapMethods));
        dispatchers.insert("range", Box::new(RangeMethods));
        dispatchers.insert("Option", Box::new(OptionMethods));
        dispatchers.insert("Result", Box::new(ResultMethods));

        MethodRegistry { dispatchers }
    }

    /// Dispatch a method call.
    ///
    /// Returns the result of the method call, or an error if the method is not found.
    pub fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        let type_name = match &receiver {
            Value::List(_) => "list",
            Value::Str(_) => "str",
            Value::Map(_) => "map",
            Value::Range(_) => "range",
            Value::Some(_) | Value::None => "Option",
            Value::Ok(_) | Value::Err(_) => "Result",
            _ => receiver.type_name(),
        };

        if let Some(dispatcher) = self.dispatchers.get(type_name) {
            if dispatcher.has_method(method) {
                return dispatcher.dispatch(receiver, method, args);
            }
        }

        Err(no_such_method(method, receiver.type_name()))
    }
}

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
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(
                        Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
                        "len",
                        vec![]
                    )
                    .unwrap(),
                Value::Int(3)
            );
            assert_eq!(
                registry
                    .dispatch(Value::list(vec![]), "len", vec![])
                    .unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn is_empty() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::list(vec![]), "is_empty", vec![])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(Value::list(vec![Value::Int(1)]), "is_empty", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn first() {
            let registry = MethodRegistry::new();

            let result = registry
                .dispatch(
                    Value::list(vec![Value::Int(1), Value::Int(2)]),
                    "first",
                    vec![],
                )
                .unwrap();
            assert_eq!(result, Value::some(Value::Int(1)));

            let result = registry
                .dispatch(Value::list(vec![]), "first", vec![])
                .unwrap();
            assert_eq!(result, Value::None);
        }

        #[test]
        fn last() {
            let registry = MethodRegistry::new();

            let result = registry
                .dispatch(
                    Value::list(vec![Value::Int(1), Value::Int(2)]),
                    "last",
                    vec![],
                )
                .unwrap();
            assert_eq!(result, Value::some(Value::Int(2)));

            let result = registry
                .dispatch(Value::list(vec![]), "last", vec![])
                .unwrap();
            assert_eq!(result, Value::None);
        }

        #[test]
        fn contains() {
            let registry = MethodRegistry::new();
            let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

            assert_eq!(
                registry
                    .dispatch(list.clone(), "contains", vec![Value::Int(2)])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(list, "contains", vec![Value::Int(5)])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn contains_wrong_arg_count() {
            let registry = MethodRegistry::new();
            let list = Value::list(vec![Value::Int(1)]);

            assert!(registry
                .dispatch(list.clone(), "contains", vec![])
                .is_err());
            assert!(registry
                .dispatch(list, "contains", vec![Value::Int(1), Value::Int(2)])
                .is_err());
        }
    }

    mod string_methods {
        use super::*;

        #[test]
        fn len() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::string("hello"), "len", vec![])
                    .unwrap(),
                Value::Int(5)
            );
            assert_eq!(
                registry
                    .dispatch(Value::string(""), "len", vec![])
                    .unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn is_empty() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::string(""), "is_empty", vec![])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(Value::string("hello"), "is_empty", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn to_uppercase() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::string("hello"), "to_uppercase", vec![])
                    .unwrap(),
                Value::string("HELLO")
            );
        }

        #[test]
        fn to_lowercase() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::string("HELLO"), "to_lowercase", vec![])
                    .unwrap(),
                Value::string("hello")
            );
        }

        #[test]
        fn trim() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::string("  hello  "), "trim", vec![])
                    .unwrap(),
                Value::string("hello")
            );
        }

        #[test]
        fn contains() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(
                        Value::string("hello world"),
                        "contains",
                        vec![Value::string("world")]
                    )
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(
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
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(
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
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(
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
            let registry = MethodRegistry::new();
            assert!(registry
                .dispatch(Value::string("hello"), "contains", vec![Value::Int(1)])
                .is_err());
        }
    }

    mod range_methods {
        use super::*;

        #[test]
        fn len() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(
                        Value::Range(RangeValue::exclusive(0, 10)),
                        "len",
                        vec![]
                    )
                    .unwrap(),
                Value::Int(10)
            );
            assert_eq!(
                registry
                    .dispatch(
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
            let registry = MethodRegistry::new();
            let range = Value::Range(RangeValue::exclusive(0, 10));

            assert_eq!(
                registry
                    .dispatch(range.clone(), "contains", vec![Value::Int(5)])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(range.clone(), "contains", vec![Value::Int(10)])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn contains_wrong_type() {
            let registry = MethodRegistry::new();
            let range = Value::Range(RangeValue::exclusive(0, 10));
            assert!(registry
                .dispatch(range, "contains", vec![Value::string("5")])
                .is_err());
        }
    }

    mod option_methods {
        use super::*;

        #[test]
        fn unwrap_some() {
            let registry = MethodRegistry::new();
            assert_eq!(
                registry
                    .dispatch(Value::some(Value::Int(42)), "unwrap", vec![])
                    .unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn unwrap_none_error() {
            let registry = MethodRegistry::new();
            assert!(registry.dispatch(Value::None, "unwrap", vec![]).is_err());
        }

        #[test]
        fn unwrap_or() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(
                        Value::some(Value::Int(42)),
                        "unwrap_or",
                        vec![Value::Int(0)]
                    )
                    .unwrap(),
                Value::Int(42)
            );
            assert_eq!(
                registry
                    .dispatch(Value::None, "unwrap_or", vec![Value::Int(0)])
                    .unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn is_some() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::some(Value::Int(1)), "is_some", vec![])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(Value::None, "is_some", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn is_none() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::None, "is_none", vec![])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(Value::some(Value::Int(1)), "is_none", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }
    }

    mod result_methods {
        use super::*;

        #[test]
        fn unwrap_ok() {
            let registry = MethodRegistry::new();
            assert_eq!(
                registry
                    .dispatch(Value::ok(Value::Int(42)), "unwrap", vec![])
                    .unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn unwrap_err_error() {
            let registry = MethodRegistry::new();
            assert!(registry
                .dispatch(Value::err(Value::string("error")), "unwrap", vec![])
                .is_err());
        }

        #[test]
        fn is_ok() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::ok(Value::Int(1)), "is_ok", vec![])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(Value::err(Value::string("e")), "is_ok", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn is_err() {
            let registry = MethodRegistry::new();

            assert_eq!(
                registry
                    .dispatch(Value::err(Value::string("e")), "is_err", vec![])
                    .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                registry
                    .dispatch(Value::ok(Value::Int(1)), "is_err", vec![])
                    .unwrap(),
                Value::Bool(false)
            );
        }
    }

    mod errors {
        use super::*;

        #[test]
        fn no_such_method() {
            let registry = MethodRegistry::new();

            assert!(registry
                .dispatch(Value::list(vec![]), "nonexistent", vec![])
                .is_err());
            assert!(registry
                .dispatch(Value::string("hello"), "nonexistent", vec![])
                .is_err());
            assert!(registry
                .dispatch(Value::Int(42), "len", vec![])
                .is_err());
        }
    }
}
