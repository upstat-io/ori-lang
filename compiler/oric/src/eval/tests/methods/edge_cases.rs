//! Edge case tests for methods.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::{RangeValue, Value};
use ori_eval::dispatch_builtin_method;

mod errors {
    use super::*;

    #[test]
    fn no_such_method() {
        assert!(dispatch_builtin_method(Value::list(vec![]), "nonexistent", vec![]).is_err());
        assert!(dispatch_builtin_method(Value::string("hello"), "nonexistent", vec![]).is_err());
        assert!(dispatch_builtin_method(Value::int(42), "len", vec![]).is_err());
    }
}

mod string_edge_cases {
    use super::*;

    #[test]
    fn len_unicode_bytes_vs_chars() {
        // "café" is 5 bytes in UTF-8 but 4 chars
        // Note: our len returns byte length, not char count
        let result = dispatch_builtin_method(Value::string("café"), "len", vec![]).unwrap();
        // Check if it returns byte length (5) or char count (4)
        assert!(matches!(result, Value::Int(n) if n.raw() == 4 || n.raw() == 5));
    }

    #[test]
    fn len_emoji() {
        // Single emoji can be 4 bytes
        let result = dispatch_builtin_method(Value::string("😀"), "len", vec![]).unwrap();
        assert!(matches!(result, Value::Int(n) if n.raw() == 1 || n.raw() == 4));
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
            dispatch_builtin_method(Value::string("\u{00A0}hello\u{00A0}"), "trim", vec![])
                .unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn contains_empty_string() {
        // Empty string is always contained
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "contains", vec![Value::string("")])
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string(""), "contains", vec![Value::string("")])
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_empty() {
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello"),
                "starts_with",
                vec![Value::string("")]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with_empty() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "ends_with", vec![Value::string("")])
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_full_string() {
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello"),
                "starts_with",
                vec![Value::string("hello")]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with_full_string() {
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello"),
                "ends_with",
                vec![Value::string("hello")]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn starts_with_longer_than_string() {
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hi"),
                "starts_with",
                vec![Value::string("hello")]
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_case_sensitive() {
        assert_eq!(
            dispatch_builtin_method(
                Value::string("Hello"),
                "contains",
                vec![Value::string("hello")]
            )
            .unwrap(),
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

mod list_edge_cases {
    use super::*;

    #[test]
    fn first_single_element() {
        let result =
            dispatch_builtin_method(Value::list(vec![Value::int(42)]), "first", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::int(42)));
    }

    #[test]
    fn last_single_element() {
        let result =
            dispatch_builtin_method(Value::list(vec![Value::int(42)]), "last", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::int(42)));
    }

    #[test]
    fn contains_different_types() {
        let list = Value::list(vec![Value::int(1), Value::string("two"), Value::Bool(true)]);

        assert_eq!(
            dispatch_builtin_method(list.clone(), "contains", vec![Value::int(1)]).unwrap(),
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
            dispatch_builtin_method(list, "contains", vec![Value::int(2)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_nested_list() {
        let inner = Value::list(vec![Value::int(1), Value::int(2)]);
        let outer = Value::list(vec![inner.clone(), Value::int(3)]);

        assert_eq!(
            dispatch_builtin_method(outer, "contains", vec![inner]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn len_large_list() {
        let items: Vec<Value> = (0..10000).map(Value::int).collect();
        let list = Value::list(items);

        assert_eq!(
            dispatch_builtin_method(list, "len", vec![]).unwrap(),
            Value::int(10000)
        );
    }
}

mod range_edge_cases {
    use super::*;

    #[test]
    fn contains_negative_range() {
        let range = Value::Range(RangeValue::exclusive(-10, 0));

        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::int(-5)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::int(-10)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range, "contains", vec![Value::int(0)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn len_negative_range() {
        let range = Value::Range(RangeValue::exclusive(-5, 5));

        assert_eq!(
            dispatch_builtin_method(range, "len", vec![]).unwrap(),
            Value::int(10)
        );
    }

    #[test]
    fn len_single_value_inclusive() {
        let range = Value::Range(RangeValue::inclusive(5, 5));

        assert_eq!(
            dispatch_builtin_method(range, "len", vec![]).unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn contains_wrong_type() {
        let range = Value::Range(RangeValue::exclusive(0, 10));

        // Range contains expects int
        assert!(dispatch_builtin_method(range, "contains", vec![Value::string("5")]).is_err());
    }
}

mod option_result_edge_cases {
    use super::*;

    #[test]
    fn unwrap_nested_some() {
        let nested = Value::some(Value::some(Value::int(42)));

        let result = dispatch_builtin_method(nested, "unwrap", vec![]).unwrap();
        assert_eq!(result, Value::some(Value::int(42)));
    }

    #[test]
    fn unwrap_nested_ok() {
        let nested = Value::ok(Value::ok(Value::int(42)));

        let result = dispatch_builtin_method(nested, "unwrap", vec![]).unwrap();
        assert_eq!(result, Value::ok(Value::int(42)));
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
