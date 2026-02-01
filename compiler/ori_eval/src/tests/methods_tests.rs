//! Tests for method implementations.
//!
//! Relocated from `methods.rs` per coding guidelines (>200 lines).

use crate::methods::dispatch_builtin_method;
use ori_ir::StringInterner;
use ori_patterns::{RangeValue, Value};

/// Create a test interner for method dispatch tests.
fn test_interner() -> StringInterner {
    StringInterner::new()
}

mod list_methods {
    use super::*;

    #[test]
    fn len() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]),
                "len",
                vec![],
                &interner
            )
            .unwrap(),
            Value::int(3)
        );
        assert_eq!(
            dispatch_builtin_method(Value::list(vec![]), "len", vec![], &interner).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn is_empty() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::list(vec![]), "is_empty", vec![], &interner).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::list(vec![Value::int(1)]),
                "is_empty",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn first() {
        let interner = test_interner();
        let result = dispatch_builtin_method(
            Value::list(vec![Value::int(1), Value::int(2)]),
            "first",
            vec![],
            &interner,
        )
        .unwrap();
        assert_eq!(result, Value::some(Value::int(1)));

        let result =
            dispatch_builtin_method(Value::list(vec![]), "first", vec![], &interner).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn last() {
        let interner = test_interner();
        let result = dispatch_builtin_method(
            Value::list(vec![Value::int(1), Value::int(2)]),
            "last",
            vec![],
            &interner,
        )
        .unwrap();
        assert_eq!(result, Value::some(Value::int(2)));

        let result =
            dispatch_builtin_method(Value::list(vec![]), "last", vec![], &interner).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn contains() {
        let interner = test_interner();
        let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);

        assert_eq!(
            dispatch_builtin_method(list.clone(), "contains", vec![Value::int(2)], &interner)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(list, "contains", vec![Value::int(5)], &interner).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_wrong_arg_count() {
        let interner = test_interner();
        let list = Value::list(vec![Value::int(1)]);

        assert!(dispatch_builtin_method(list.clone(), "contains", vec![], &interner).is_err());
        assert!(dispatch_builtin_method(
            list,
            "contains",
            vec![Value::int(1), Value::int(2)],
            &interner
        )
        .is_err());
    }
}

mod string_methods {
    use super::*;

    #[test]
    fn len() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "len", vec![], &interner).unwrap(),
            Value::int(5)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string(""), "len", vec![], &interner).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn is_empty() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::string(""), "is_empty", vec![], &interner).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "is_empty", vec![], &interner).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn to_uppercase() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "to_uppercase", vec![], &interner)
                .unwrap(),
            Value::string("HELLO")
        );
    }

    #[test]
    fn to_lowercase() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::string("HELLO"), "to_lowercase", vec![], &interner)
                .unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn trim() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::string("  hello  "), "trim", vec![], &interner).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn contains() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello world"),
                "contains",
                vec![Value::string("world")],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello"),
                "contains",
                vec![Value::string("xyz")],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn starts_with() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello world"),
                "starts_with",
                vec![Value::string("hello")],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ends_with() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::string("hello world"),
                "ends_with",
                vec![Value::string("world")],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn wrong_arg_type() {
        let interner = test_interner();
        assert!(dispatch_builtin_method(
            Value::string("hello"),
            "contains",
            vec![Value::int(1)],
            &interner
        )
        .is_err());
    }
}

mod range_methods {
    use super::*;

    #[test]
    fn len() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Range(RangeValue::exclusive(0, 10)),
                "len",
                vec![],
                &interner
            )
            .unwrap(),
            Value::int(10)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Range(RangeValue::inclusive(0, 10)),
                "len",
                vec![],
                &interner
            )
            .unwrap(),
            Value::int(11)
        );
    }

    #[test]
    fn contains() {
        let interner = test_interner();
        let range = Value::Range(RangeValue::exclusive(0, 10));

        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::int(5)], &interner)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range, "contains", vec![Value::int(10)], &interner).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_wrong_type() {
        let interner = test_interner();
        let range = Value::Range(RangeValue::exclusive(0, 10));
        assert!(
            dispatch_builtin_method(range, "contains", vec![Value::string("5")], &interner)
                .is_err()
        );
    }
}

mod option_methods {
    use super::*;

    #[test]
    fn unwrap_some() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::some(Value::int(42)), "unwrap", vec![], &interner)
                .unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn unwrap_none_error() {
        let interner = test_interner();
        assert!(dispatch_builtin_method(Value::None, "unwrap", vec![], &interner).is_err());
    }

    #[test]
    fn unwrap_or() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::some(Value::int(42)),
                "unwrap_or",
                vec![Value::int(0)],
                &interner
            )
            .unwrap(),
            Value::int(42)
        );
        assert_eq!(
            dispatch_builtin_method(Value::None, "unwrap_or", vec![Value::int(0)], &interner)
                .unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn is_some() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::some(Value::int(1)), "is_some", vec![], &interner)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::None, "is_some", vec![], &interner).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_none() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::None, "is_none", vec![], &interner).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::some(Value::int(1)), "is_none", vec![], &interner)
                .unwrap(),
            Value::Bool(false)
        );
    }
}

mod result_methods {
    use super::*;

    #[test]
    fn unwrap_ok() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::ok(Value::int(42)), "unwrap", vec![], &interner)
                .unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn unwrap_err_error() {
        let interner = test_interner();
        assert!(dispatch_builtin_method(
            Value::err(Value::string("error")),
            "unwrap",
            vec![],
            &interner
        )
        .is_err());
    }

    #[test]
    fn is_ok() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::ok(Value::int(1)), "is_ok", vec![], &interner).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::err(Value::string("e")), "is_ok", vec![], &interner)
                .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_err() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(Value::err(Value::string("e")), "is_err", vec![], &interner)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::ok(Value::int(1)), "is_err", vec![], &interner).unwrap(),
            Value::Bool(false)
        );
    }
}

mod ordering_methods {
    use super::*;
    use ori_patterns::OrderingValue;

    #[test]
    fn is_less() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "is_less",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "is_less",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "is_less",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_equal() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "is_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "is_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "is_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_greater() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "is_greater",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "is_greater",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "is_greater",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn is_less_or_equal() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "is_less_or_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "is_less_or_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "is_less_or_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn is_greater_or_equal() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "is_greater_or_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "is_greater_or_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "is_greater_or_equal",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn reverse() {
        let interner = test_interner();

        // Less.reverse() -> Greater
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "reverse",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Ordering(OrderingValue::Greater)
        );

        // Equal.reverse() -> Equal
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "reverse",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Ordering(OrderingValue::Equal)
        );

        // Greater.reverse() -> Less
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "reverse",
                vec![],
                &interner
            )
            .unwrap(),
            Value::Ordering(OrderingValue::Less)
        );
    }

    #[test]
    fn clone() {
        let interner = test_interner();
        let original = Value::Ordering(OrderingValue::Less);
        let cloned = dispatch_builtin_method(original.clone(), "clone", vec![], &interner).unwrap();
        assert_eq!(original, cloned);
    }

    #[test]
    fn to_str() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "to_str",
                vec![],
                &interner
            )
            .unwrap(),
            Value::string("Less")
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "to_str",
                vec![],
                &interner
            )
            .unwrap(),
            Value::string("Equal")
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "to_str",
                vec![],
                &interner
            )
            .unwrap(),
            Value::string("Greater")
        );
    }

    #[test]
    fn hash() {
        let interner = test_interner();
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Less),
                "hash",
                vec![],
                &interner
            )
            .unwrap(),
            Value::int(-1)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Equal),
                "hash",
                vec![],
                &interner
            )
            .unwrap(),
            Value::int(0)
        );
        assert_eq!(
            dispatch_builtin_method(
                Value::Ordering(OrderingValue::Greater),
                "hash",
                vec![],
                &interner
            )
            .unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn no_such_method() {
        let interner = test_interner();
        assert!(dispatch_builtin_method(
            Value::Ordering(OrderingValue::Less),
            "nonexistent",
            vec![],
            &interner
        )
        .is_err());
    }
}

mod errors {
    use super::*;

    #[test]
    fn no_such_method() {
        let interner = test_interner();
        assert!(
            dispatch_builtin_method(Value::list(vec![]), "nonexistent", vec![], &interner).is_err()
        );
        assert!(
            dispatch_builtin_method(Value::string("hello"), "nonexistent", vec![], &interner)
                .is_err()
        );
        assert!(dispatch_builtin_method(Value::int(42), "len", vec![], &interner).is_err());
    }
}
