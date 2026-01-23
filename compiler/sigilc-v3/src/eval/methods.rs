//! Method dispatch implementations for the evaluator.
//!
//! This module extracts method call logic from the evaluator,
//! following the Open/Closed Principle. New methods can be added
//! by implementing the `MethodDispatcher` trait.

use std::rc::Rc;
use std::collections::HashMap;
use super::value::Value;
use super::evaluator::{EvalResult, EvalError};
use super::errors;

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
        let items = match receiver {
            Value::List(items) => items,
            _ => return Err(EvalError::new("expected list")),
        };

        match method {
            "len" => Ok(Value::Int(items.len() as i64)),
            "is_empty" => Ok(Value::Bool(items.is_empty())),
            "first" => Ok(items
                .first()
                .cloned()
                .map(|v| Value::Some(Box::new(v)))
                .unwrap_or(Value::None)),
            "last" => Ok(items
                .last()
                .cloned()
                .map(|v| Value::Some(Box::new(v)))
                .unwrap_or(Value::None)),
            "contains" => {
                if args.len() != 1 {
                    return Err(errors::wrong_arg_count("contains", 1, args.len()));
                }
                Ok(Value::Bool(items.contains(&args[0])))
            }
            _ => Err(errors::no_such_method(method, "list")),
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
        let s = match receiver {
            Value::Str(s) => s,
            _ => return Err(EvalError::new("expected string")),
        };

        match method {
            "len" => Ok(Value::Int(s.len() as i64)),
            "is_empty" => Ok(Value::Bool(s.is_empty())),
            "to_uppercase" => Ok(Value::Str(Rc::new(s.to_uppercase()))),
            "to_lowercase" => Ok(Value::Str(Rc::new(s.to_lowercase()))),
            "trim" => Ok(Value::Str(Rc::new(s.trim().to_string()))),
            "contains" => {
                if args.len() != 1 {
                    return Err(errors::wrong_arg_count("contains", 1, args.len()));
                }
                if let Value::Str(needle) = &args[0] {
                    Ok(Value::Bool(s.contains(needle.as_str())))
                } else {
                    Err(errors::wrong_arg_type("contains", "string"))
                }
            }
            "starts_with" => {
                if args.len() != 1 {
                    return Err(errors::wrong_arg_count("starts_with", 1, args.len()));
                }
                if let Value::Str(prefix) = &args[0] {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(errors::wrong_arg_type("starts_with", "string"))
                }
            }
            "ends_with" => {
                if args.len() != 1 {
                    return Err(errors::wrong_arg_count("ends_with", 1, args.len()));
                }
                if let Value::Str(suffix) = &args[0] {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(errors::wrong_arg_type("ends_with", "string"))
                }
            }
            _ => Err(errors::no_such_method(method, "str")),
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
        let r = match receiver {
            Value::Range(r) => r,
            _ => return Err(EvalError::new("expected range")),
        };

        match method {
            "len" => Ok(Value::Int(r.len() as i64)),
            "contains" => {
                if args.len() != 1 {
                    return Err(errors::wrong_arg_count("contains", 1, args.len()));
                }
                if let Value::Int(n) = args[0] {
                    Ok(Value::Bool(r.contains(n)))
                } else {
                    Err(errors::wrong_arg_type("contains", "int"))
                }
            }
            _ => Err(errors::no_such_method(method, "range")),
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
            ("unwrap", Value::Some(v)) => Ok((**v).clone()),
            ("unwrap", Value::None) => Err(EvalError::new("called unwrap on None")),
            ("is_some", Value::Some(_)) => Ok(Value::Bool(true)),
            ("is_some", Value::None) => Ok(Value::Bool(false)),
            ("is_none", Value::Some(_)) => Ok(Value::Bool(false)),
            ("is_none", Value::None) => Ok(Value::Bool(true)),
            ("unwrap_or", Value::Some(v)) => Ok((**v).clone()),
            ("unwrap_or", Value::None) => {
                if args.len() != 1 {
                    return Err(errors::wrong_arg_count("unwrap_or", 1, args.len()));
                }
                Ok(args[0].clone())
            }
            _ => Err(errors::no_such_method(method, "Option")),
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
                Err(EvalError::new(format!("called unwrap on Err: {:?}", e)))
            }
            ("is_ok", Value::Ok(_)) => Ok(Value::Bool(true)),
            ("is_ok", Value::Err(_)) => Ok(Value::Bool(false)),
            ("is_err", Value::Ok(_)) => Ok(Value::Bool(false)),
            ("is_err", Value::Err(_)) => Ok(Value::Bool(true)),
            _ => Err(errors::no_such_method(method, "Result")),
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
        dispatchers.insert("range", Box::new(RangeMethods));
        // Option and Result are handled specially since they share type names
        dispatchers.insert("Option", Box::new(OptionMethods));
        dispatchers.insert("Result", Box::new(ResultMethods));

        MethodRegistry { dispatchers }
    }

    /// Dispatch a method call.
    ///
    /// Returns the result of the method call, or an error if the method is not found.
    pub fn dispatch(&self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        // Determine the type name based on the receiver
        let type_name = match &receiver {
            Value::List(_) => "list",
            Value::Str(_) => "str",
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

        Err(errors::no_such_method(method, receiver.type_name()))
    }
}

impl Default for MethodRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_len() {
        let registry = MethodRegistry::new();
        let list = Value::List(Rc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));

        assert_eq!(
            registry.dispatch(list, "len", vec![]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn test_list_is_empty() {
        let registry = MethodRegistry::new();

        let empty = Value::List(Rc::new(vec![]));
        assert_eq!(
            registry.dispatch(empty, "is_empty", vec![]).unwrap(),
            Value::Bool(true)
        );

        let non_empty = Value::List(Rc::new(vec![Value::Int(1)]));
        assert_eq!(
            registry.dispatch(non_empty, "is_empty", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_string_to_uppercase() {
        let registry = MethodRegistry::new();
        let s = Value::Str(Rc::new("hello".to_string()));

        assert_eq!(
            registry.dispatch(s, "to_uppercase", vec![]).unwrap(),
            Value::Str(Rc::new("HELLO".to_string()))
        );
    }

    #[test]
    fn test_option_unwrap() {
        let registry = MethodRegistry::new();

        let some = Value::Some(Box::new(Value::Int(42)));
        assert_eq!(
            registry.dispatch(some, "unwrap", vec![]).unwrap(),
            Value::Int(42)
        );

        let none = Value::None;
        assert!(registry.dispatch(none, "unwrap", vec![]).is_err());
    }

    #[test]
    fn test_no_such_method() {
        let registry = MethodRegistry::new();
        let list = Value::List(Rc::new(vec![]));

        assert!(registry.dispatch(list, "nonexistent", vec![]).is_err());
    }
}
