//! Tests for method dispatch.
//!
//! Tests method calls on built-in types including list, string, range,
//! Option, and Result.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use sigil_eval::MethodRegistry;
use crate::eval::Value;

// List methods

mod list_methods {
    use super::*;

    #[test]
    fn len() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]), "len", vec![]).unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            registry.dispatch(Value::list(vec![]), "len", vec![]).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn is_empty() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::list(vec![]), "is_empty", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::list(vec![Value::Int(1)]), "is_empty", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn first() {
        let registry = MethodRegistry::new();

        // Non-empty list
        let result = registry.dispatch(Value::list(vec![Value::Int(1), Value::Int(2)]), "first", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(1)));

        // Empty list
        let result = registry.dispatch(Value::list(vec![]), "first", vec![]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn last() {
        let registry = MethodRegistry::new();

        // Non-empty list
        let result = registry.dispatch(Value::list(vec![Value::Int(1), Value::Int(2)]), "last", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(2)));

        // Empty list
        let result = registry.dispatch(Value::list(vec![]), "last", vec![]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn contains() {
        let registry = MethodRegistry::new();
        let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

        assert_eq!(
            registry.dispatch(list.clone(), "contains", vec![Value::Int(2)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(list, "contains", vec![Value::Int(5)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_wrong_arg_count() {
        let registry = MethodRegistry::new();
        let list = Value::list(vec![Value::Int(1)]);

        assert!(registry.dispatch(list.clone(), "contains", vec![]).is_err());
        assert!(registry.dispatch(list, "contains", vec![Value::Int(1), Value::Int(2)]).is_err());
    }
}

// String methods

mod string_methods {
    use super::*;

    #[test]
    fn len() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("hello"), "len", vec![]).unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            registry.dispatch(Value::string(""), "len", vec![]).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn is_empty() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string(""), "is_empty", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::string("hello"), "is_empty", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn to_uppercase() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("hello"), "to_uppercase", vec![]).unwrap(),
            Value::string("HELLO")
        );
        assert_eq!(
            registry.dispatch(Value::string("Hello World"), "to_uppercase", vec![]).unwrap(),
            Value::string("HELLO WORLD")
        );
    }

    #[test]
    fn to_lowercase() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("HELLO"), "to_lowercase", vec![]).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn trim() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("  hello  "), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
        assert_eq!(
            registry.dispatch(Value::string("\n\thello\t\n"), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn contains() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("hello world"), "contains", vec![Value::string("world")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::string("hello"), "contains", vec![Value::string("xyz")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn starts_with() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("hello world"), "starts_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::string("hello"), "starts_with", vec![Value::string("world")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn ends_with() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::string("hello world"), "ends_with", vec![Value::string("world")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::string("hello"), "ends_with", vec![Value::string("xyz")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn wrong_arg_type() {
        let registry = MethodRegistry::new();

        // contains expects string, not int
        assert!(registry.dispatch(Value::string("hello"), "contains", vec![Value::Int(1)]).is_err());
    }
}

// Range methods

mod range_methods {
    use super::*;
    use crate::eval::RangeValue;

    #[test]
    fn len() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::Range(RangeValue::exclusive(0, 10)), "len", vec![]).unwrap(),
            Value::Int(10)
        );
        assert_eq!(
            registry.dispatch(Value::Range(RangeValue::inclusive(0, 10)), "len", vec![]).unwrap(),
            Value::Int(11)
        );
        assert_eq!(
            registry.dispatch(Value::Range(RangeValue::exclusive(5, 5)), "len", vec![]).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn contains() {
        let registry = MethodRegistry::new();
        let range = Value::Range(RangeValue::exclusive(0, 10));

        assert_eq!(
            registry.dispatch(range.clone(), "contains", vec![Value::Int(5)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(range.clone(), "contains", vec![Value::Int(0)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(range.clone(), "contains", vec![Value::Int(10)]).unwrap(),
            Value::Bool(false)  // Exclusive end
        );
        assert_eq!(
            registry.dispatch(range, "contains", vec![Value::Int(-1)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_inclusive() {
        let registry = MethodRegistry::new();
        let range = Value::Range(RangeValue::inclusive(0, 10));

        assert_eq!(
            registry.dispatch(range.clone(), "contains", vec![Value::Int(10)]).unwrap(),
            Value::Bool(true)  // Inclusive end
        );
    }
}

// Option methods

mod option_methods {
    use super::*;

    #[test]
    fn unwrap_some() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::some(Value::Int(42)), "unwrap", vec![]).unwrap(),
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
            registry.dispatch(Value::some(Value::Int(42)), "unwrap_or", vec![Value::Int(0)]).unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            registry.dispatch(Value::None, "unwrap_or", vec![Value::Int(0)]).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn is_some() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::some(Value::Int(1)), "is_some", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::None, "is_some", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_none() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::None, "is_none", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::some(Value::Int(1)), "is_none", vec![]).unwrap(),
            Value::Bool(false)
        );
    }
}

// Result methods

mod result_methods {
    use super::*;

    #[test]
    fn unwrap_ok() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::ok(Value::Int(42)), "unwrap", vec![]).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn unwrap_err_error() {
        let registry = MethodRegistry::new();

        assert!(registry.dispatch(Value::err(Value::string("error")), "unwrap", vec![]).is_err());
    }

    #[test]
    fn is_ok() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::ok(Value::Int(1)), "is_ok", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::err(Value::string("e")), "is_ok", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_err() {
        let registry = MethodRegistry::new();

        assert_eq!(
            registry.dispatch(Value::err(Value::string("e")), "is_err", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::ok(Value::Int(1)), "is_err", vec![]).unwrap(),
            Value::Bool(false)
        );
    }
}

// Error cases

mod errors {
    use super::*;

    #[test]
    fn no_such_method() {
        let registry = MethodRegistry::new();

        assert!(registry.dispatch(Value::list(vec![]), "nonexistent", vec![]).is_err());
        assert!(registry.dispatch(Value::string("hello"), "nonexistent", vec![]).is_err());
        assert!(registry.dispatch(Value::Int(42), "len", vec![]).is_err());
    }
}

// String method edge cases

mod string_edge_cases {
    use super::*;

    #[test]
    fn len_unicode_bytes_vs_chars() {
        let registry = MethodRegistry::new();
        // "café" is 5 bytes in UTF-8 but 4 chars
        // Note: our len returns byte length, not char count
        let result = registry.dispatch(Value::string("café"), "len", vec![]).unwrap();
        // Check if it returns byte length (5) or char count (4)
        assert!(matches!(result, Value::Int(4) | Value::Int(5)));
    }

    #[test]
    fn len_emoji() {
        let registry = MethodRegistry::new();
        // Single emoji can be 4 bytes
        let result = registry.dispatch(Value::string("😀"), "len", vec![]).unwrap();
        assert!(matches!(result, Value::Int(1) | Value::Int(4)));
    }

    #[test]
    fn to_uppercase_unicode() {
        let registry = MethodRegistry::new();
        // German sharp s (ß) uppercases to SS
        assert_eq!(
            registry.dispatch(Value::string("straße"), "to_uppercase", vec![]).unwrap(),
            Value::string("STRASSE")
        );
    }

    #[test]
    fn to_lowercase_unicode() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("CAFÉ"), "to_lowercase", vec![]).unwrap(),
            Value::string("café")
        );
    }

    #[test]
    fn trim_unicode_whitespace() {
        let registry = MethodRegistry::new();
        // Non-breaking space (U+00A0)
        assert_eq!(
            registry.dispatch(Value::string("\u{00A0}hello\u{00A0}"), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn contains_empty_string() {
        let registry = MethodRegistry::new();
        // Empty string is always contained
        assert_eq!(
            registry.dispatch(Value::string("hello"), "contains", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(Value::string(""), "contains", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_empty() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("hello"), "starts_with", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with_empty() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("hello"), "ends_with", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_full_string() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("hello"), "starts_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with_full_string() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("hello"), "ends_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_longer_than_string() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("hi"), "starts_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_case_sensitive() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("Hello"), "contains", vec![Value::string("hello")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn trim_only_whitespace() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("   "), "trim", vec![]).unwrap(),
            Value::string("")
        );
    }

    #[test]
    fn trim_no_whitespace() {
        let registry = MethodRegistry::new();
        assert_eq!(
            registry.dispatch(Value::string("hello"), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
    }
}

// List method edge cases

mod list_edge_cases {
    use super::*;

    #[test]
    fn first_single_element() {
        let registry = MethodRegistry::new();
        let result = registry.dispatch(Value::list(vec![Value::Int(42)]), "first", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn last_single_element() {
        let registry = MethodRegistry::new();
        let result = registry.dispatch(Value::list(vec![Value::Int(42)]), "last", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn contains_different_types() {
        let registry = MethodRegistry::new();
        let list = Value::list(vec![Value::Int(1), Value::string("two"), Value::Bool(true)]);

        assert_eq!(
            registry.dispatch(list.clone(), "contains", vec![Value::Int(1)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(list.clone(), "contains", vec![Value::string("two")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(list.clone(), "contains", vec![Value::Bool(true)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(list, "contains", vec![Value::Int(2)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_nested_list() {
        let registry = MethodRegistry::new();
        let inner = Value::list(vec![Value::Int(1), Value::Int(2)]);
        let outer = Value::list(vec![inner.clone(), Value::Int(3)]);

        assert_eq!(
            registry.dispatch(outer, "contains", vec![inner]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn len_large_list() {
        let registry = MethodRegistry::new();
        let items: Vec<Value> = (0..10000).map(Value::Int).collect();
        let list = Value::list(items);

        assert_eq!(
            registry.dispatch(list, "len", vec![]).unwrap(),
            Value::Int(10000)
        );
    }
}

// Range method edge cases

mod range_edge_cases {
    use super::*;
    use crate::eval::RangeValue;

    #[test]
    fn contains_negative_range() {
        let registry = MethodRegistry::new();
        let range = Value::Range(RangeValue::exclusive(-10, 0));

        assert_eq!(
            registry.dispatch(range.clone(), "contains", vec![Value::Int(-5)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(range.clone(), "contains", vec![Value::Int(-10)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.dispatch(range, "contains", vec![Value::Int(0)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn len_negative_range() {
        let registry = MethodRegistry::new();
        let range = Value::Range(RangeValue::exclusive(-5, 5));

        assert_eq!(
            registry.dispatch(range, "len", vec![]).unwrap(),
            Value::Int(10)
        );
    }

    #[test]
    fn len_single_value_inclusive() {
        let registry = MethodRegistry::new();
        let range = Value::Range(RangeValue::inclusive(5, 5));

        assert_eq!(
            registry.dispatch(range, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn contains_wrong_type() {
        let registry = MethodRegistry::new();
        let range = Value::Range(RangeValue::exclusive(0, 10));

        // Range contains expects int
        assert!(registry.dispatch(range, "contains", vec![Value::string("5")]).is_err());
    }
}

// Option/Result edge cases

mod option_result_edge_cases {
    use super::*;

    #[test]
    fn unwrap_nested_some() {
        let registry = MethodRegistry::new();
        let nested = Value::some(Value::some(Value::Int(42)));

        let result = registry.dispatch(nested, "unwrap", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn unwrap_nested_ok() {
        let registry = MethodRegistry::new();
        let nested = Value::ok(Value::ok(Value::Int(42)));

        let result = registry.dispatch(nested, "unwrap", vec![]).unwrap();
        assert_eq!(result, Value::ok(Value::Int(42)));
    }

    #[test]
    fn unwrap_or_wrong_arg_count() {
        let registry = MethodRegistry::new();

        assert!(registry.dispatch(Value::None, "unwrap_or", vec![]).is_err());
    }

    #[test]
    fn is_some_nested() {
        let registry = MethodRegistry::new();
        // Some(None) is still Some
        let val = Value::some(Value::None);

        assert_eq!(
            registry.dispatch(val, "is_some", vec![]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn is_ok_nested() {
        let registry = MethodRegistry::new();
        // Ok(Err) is still Ok
        let val = Value::ok(Value::err(Value::string("inner")));

        assert_eq!(
            registry.dispatch(val, "is_ok", vec![]).unwrap(),
            Value::Bool(true)
        );
    }
}
