//! Tests for method dispatch.
//!
//! Tests method calls on built-in types including list, string, range,
//! Option, and Result.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use ori_eval::dispatch_builtin_method;
use crate::eval::Value;

// List methods

mod list_methods {
    use super::*;

    #[test]
    fn len() {
        assert_eq!(
            dispatch_builtin_method(Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]), "len", vec![]).unwrap(),
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
            dispatch_builtin_method(Value::list(vec![Value::Int(1)]), "is_empty", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn first() {
        
        // Non-empty list
        let result = dispatch_builtin_method(Value::list(vec![Value::Int(1), Value::Int(2)]), "first", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(1)));

        // Empty list
        let result = dispatch_builtin_method(Value::list(vec![]), "first", vec![]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn last() {
        
        // Non-empty list
        let result = dispatch_builtin_method(Value::list(vec![Value::Int(1), Value::Int(2)]), "last", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(2)));

        // Empty list
        let result = dispatch_builtin_method(Value::list(vec![]), "last", vec![]).unwrap();
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
        assert!(dispatch_builtin_method(list, "contains", vec![Value::Int(1), Value::Int(2)]).is_err());
    }
}

// String methods

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
        assert_eq!(
            dispatch_builtin_method(Value::string("Hello World"), "to_uppercase", vec![]).unwrap(),
            Value::string("HELLO WORLD")
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
        assert_eq!(
            dispatch_builtin_method(Value::string("\n\thello\t\n"), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn contains() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello world"), "contains", vec![Value::string("world")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "contains", vec![Value::string("xyz")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn starts_with() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello world"), "starts_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "starts_with", vec![Value::string("world")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn ends_with() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello world"), "ends_with", vec![Value::string("world")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "ends_with", vec![Value::string("xyz")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn wrong_arg_type() {
        
        // contains expects string, not int
        assert!(dispatch_builtin_method(Value::string("hello"), "contains", vec![Value::Int(1)]).is_err());
    }
}

// Range methods

mod range_methods {
    use super::*;
    use crate::eval::RangeValue;

    #[test]
    fn len() {
        assert_eq!(
            dispatch_builtin_method(Value::Range(RangeValue::exclusive(0, 10)), "len", vec![]).unwrap(),
            Value::Int(10)
        );
        assert_eq!(
            dispatch_builtin_method(Value::Range(RangeValue::inclusive(0, 10)), "len", vec![]).unwrap(),
            Value::Int(11)
        );
        assert_eq!(
            dispatch_builtin_method(Value::Range(RangeValue::exclusive(5, 5)), "len", vec![]).unwrap(),
            Value::Int(0)
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
            dispatch_builtin_method(range.clone(), "contains", vec![Value::Int(0)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::Int(10)]).unwrap(),
            Value::Bool(false)  // Exclusive end
        );
        assert_eq!(
            dispatch_builtin_method(range, "contains", vec![Value::Int(-1)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_inclusive() {
        let range = Value::Range(RangeValue::inclusive(0, 10));

        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::Int(10)]).unwrap(),
            Value::Bool(true)  // Inclusive end
        );
    }
}

// Option methods

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
            dispatch_builtin_method(Value::some(Value::Int(42)), "unwrap_or", vec![Value::Int(0)]).unwrap(),
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

// Result methods

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
        
        assert!(dispatch_builtin_method(Value::err(Value::string("error")), "unwrap", vec![]).is_err());
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

// Error cases

mod errors {
    use super::*;

    #[test]
    fn no_such_method() {
        
        assert!(dispatch_builtin_method(Value::list(vec![]), "nonexistent", vec![]).is_err());
        assert!(dispatch_builtin_method(Value::string("hello"), "nonexistent", vec![]).is_err());
        assert!(dispatch_builtin_method(Value::Int(42), "len", vec![]).is_err());
    }
}

// String method edge cases

mod string_edge_cases {
    use super::*;

    #[test]
    fn len_unicode_bytes_vs_chars() {
        // "café" is 5 bytes in UTF-8 but 4 chars
        // Note: our len returns byte length, not char count
        let result = dispatch_builtin_method(Value::string("café"), "len", vec![]).unwrap();
        // Check if it returns byte length (5) or char count (4)
        assert!(matches!(result, Value::Int(4) | Value::Int(5)));
    }

    #[test]
    fn len_emoji() {
        // Single emoji can be 4 bytes
        let result = dispatch_builtin_method(Value::string("😀"), "len", vec![]).unwrap();
        assert!(matches!(result, Value::Int(1) | Value::Int(4)));
    }

    #[test]
    fn to_uppercase_unicode() {
        // German sharp s (ß) uppercases to SS
        assert_eq!(
            dispatch_builtin_method(Value::string("straße"), "to_uppercase", vec![]).unwrap(),
            Value::string("STRASSE")
        );
    }

    #[test]
    fn to_lowercase_unicode() {
        assert_eq!(
            dispatch_builtin_method(Value::string("CAFÉ"), "to_lowercase", vec![]).unwrap(),
            Value::string("café")
        );
    }

    #[test]
    fn trim_unicode_whitespace() {
        // Non-breaking space (U+00A0)
        assert_eq!(
            dispatch_builtin_method(Value::string("\u{00A0}hello\u{00A0}"), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn contains_empty_string() {
        // Empty string is always contained
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "contains", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string(""), "contains", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_empty() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "starts_with", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with_empty() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "ends_with", vec![Value::string("")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_full_string() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "starts_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with_full_string() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "ends_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_longer_than_string() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hi"), "starts_with", vec![Value::string("hello")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_case_sensitive() {
        assert_eq!(
            dispatch_builtin_method(Value::string("Hello"), "contains", vec![Value::string("hello")]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn trim_only_whitespace() {
        assert_eq!(
            dispatch_builtin_method(Value::string("   "), "trim", vec![]).unwrap(),
            Value::string("")
        );
    }

    #[test]
    fn trim_no_whitespace() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "trim", vec![]).unwrap(),
            Value::string("hello")
        );
    }
}

// List method edge cases

mod list_edge_cases {
    use super::*;

    #[test]
    fn first_single_element() {
        let result = dispatch_builtin_method(Value::list(vec![Value::Int(42)]), "first", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn last_single_element() {
        let result = dispatch_builtin_method(Value::list(vec![Value::Int(42)]), "last", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn contains_different_types() {
        let list = Value::list(vec![Value::Int(1), Value::string("two"), Value::Bool(true)]);

        assert_eq!(
            dispatch_builtin_method(list.clone(), "contains", vec![Value::Int(1)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(list.clone(), "contains", vec![Value::string("two")]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(list.clone(), "contains", vec![Value::Bool(true)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(list, "contains", vec![Value::Int(2)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_nested_list() {
        let inner = Value::list(vec![Value::Int(1), Value::Int(2)]);
        let outer = Value::list(vec![inner.clone(), Value::Int(3)]);

        assert_eq!(
            dispatch_builtin_method(outer, "contains", vec![inner]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn len_large_list() {
        let items: Vec<Value> = (0..10000).map(Value::Int).collect();
        let list = Value::list(items);

        assert_eq!(
            dispatch_builtin_method(list, "len", vec![]).unwrap(),
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
        let range = Value::Range(RangeValue::exclusive(-10, 0));

        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::Int(-5)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::Int(-10)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range, "contains", vec![Value::Int(0)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn len_negative_range() {
        let range = Value::Range(RangeValue::exclusive(-5, 5));

        assert_eq!(
            dispatch_builtin_method(range, "len", vec![]).unwrap(),
            Value::Int(10)
        );
    }

    #[test]
    fn len_single_value_inclusive() {
        let range = Value::Range(RangeValue::inclusive(5, 5));

        assert_eq!(
            dispatch_builtin_method(range, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn contains_wrong_type() {
        let range = Value::Range(RangeValue::exclusive(0, 10));

        // Range contains expects int
        assert!(dispatch_builtin_method(range, "contains", vec![Value::string("5")]).is_err());
    }
}

// Option/Result edge cases

mod option_result_edge_cases {
    use super::*;

    #[test]
    fn unwrap_nested_some() {
        let nested = Value::some(Value::some(Value::Int(42)));

        let result = dispatch_builtin_method(nested, "unwrap", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn unwrap_nested_ok() {
        let nested = Value::ok(Value::ok(Value::Int(42)));

        let result = dispatch_builtin_method(nested, "unwrap", vec![]).unwrap();
        assert_eq!(result, Value::ok(Value::Int(42)));
    }

    #[test]
    fn unwrap_or_wrong_arg_count() {
        
        assert!(dispatch_builtin_method(Value::None, "unwrap_or", vec![]).is_err());
    }

    #[test]
    fn is_some_nested() {
        // Some(None) is still Some
        let val = Value::some(Value::None);

        assert_eq!(
            dispatch_builtin_method(val, "is_some", vec![]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn is_ok_nested() {
        // Ok(Err) is still Ok
        let val = Value::ok(Value::err(Value::string("inner")));

        assert_eq!(
            dispatch_builtin_method(val, "is_ok", vec![]).unwrap(),
            Value::Bool(true)
        );
    }
}
