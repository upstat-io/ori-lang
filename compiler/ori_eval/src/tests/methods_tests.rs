//! Tests for method implementations.
//!
//! Relocated from `methods.rs` per coding guidelines (>200 lines).

use crate::methods::dispatch_builtin_method;
use ori_patterns::{RangeValue, Value};

mod list_methods {
    use super::*;

    #[test]
    fn len() {
        assert_eq!(
            dispatch_builtin_method(
                Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]),
                "len",
                vec![]
            )
            .unwrap(),
            Value::int(3)
        );
        assert_eq!(
            dispatch_builtin_method(Value::list(vec![]), "len", vec![]).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn is_empty() {
        assert_eq!(
            dispatch_builtin_method(Value::list(vec![]), "is_empty", vec![]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(Value::list(vec![Value::int(1)]), "is_empty", vec![]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn first() {
        let result = dispatch_builtin_method(
            Value::list(vec![Value::int(1), Value::int(2)]),
            "first",
            vec![],
        )
        .unwrap();
        assert_eq!(result, Value::some(Value::int(1)));

        let result = dispatch_builtin_method(Value::list(vec![]), "first", vec![]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn last() {
        let result = dispatch_builtin_method(
            Value::list(vec![Value::int(1), Value::int(2)]),
            "last",
            vec![],
        )
        .unwrap();
        assert_eq!(result, Value::some(Value::int(2)));

        let result = dispatch_builtin_method(Value::list(vec![]), "last", vec![]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn contains() {
        let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);

        assert_eq!(
            dispatch_builtin_method(list.clone(), "contains", vec![Value::int(2)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(list, "contains", vec![Value::int(5)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_wrong_arg_count() {
        let list = Value::list(vec![Value::int(1)]);

        assert!(dispatch_builtin_method(list.clone(), "contains", vec![]).is_err());
        assert!(
            dispatch_builtin_method(list, "contains", vec![Value::int(1), Value::int(2)]).is_err()
        );
    }
}

mod string_methods {
    use super::*;

    #[test]
    fn len() {
        assert_eq!(
            dispatch_builtin_method(Value::string("hello"), "len", vec![]).unwrap(),
            Value::int(5)
        );
        assert_eq!(
            dispatch_builtin_method(Value::string(""), "len", vec![]).unwrap(),
            Value::int(0)
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
        assert!(
            dispatch_builtin_method(Value::string("hello"), "contains", vec![Value::int(1)])
                .is_err()
        );
    }
}

mod range_methods {
    use super::*;

    #[test]
    fn len() {
        assert_eq!(
            dispatch_builtin_method(Value::Range(RangeValue::exclusive(0, 10)), "len", vec![])
                .unwrap(),
            Value::int(10)
        );
        assert_eq!(
            dispatch_builtin_method(Value::Range(RangeValue::inclusive(0, 10)), "len", vec![])
                .unwrap(),
            Value::int(11)
        );
    }

    #[test]
    fn contains() {
        let range = Value::Range(RangeValue::exclusive(0, 10));

        assert_eq!(
            dispatch_builtin_method(range.clone(), "contains", vec![Value::int(5)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            dispatch_builtin_method(range, "contains", vec![Value::int(10)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn contains_wrong_type() {
        let range = Value::Range(RangeValue::exclusive(0, 10));
        assert!(dispatch_builtin_method(range, "contains", vec![Value::string("5")]).is_err());
    }
}

mod option_methods {
    use super::*;

    #[test]
    fn unwrap_some() {
        assert_eq!(
            dispatch_builtin_method(Value::some(Value::int(42)), "unwrap", vec![]).unwrap(),
            Value::int(42)
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
                Value::some(Value::int(42)),
                "unwrap_or",
                vec![Value::int(0)]
            )
            .unwrap(),
            Value::int(42)
        );
        assert_eq!(
            dispatch_builtin_method(Value::None, "unwrap_or", vec![Value::int(0)]).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn is_some() {
        assert_eq!(
            dispatch_builtin_method(Value::some(Value::int(1)), "is_some", vec![]).unwrap(),
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
            dispatch_builtin_method(Value::some(Value::int(1)), "is_none", vec![]).unwrap(),
            Value::Bool(false)
        );
    }
}

mod result_methods {
    use super::*;

    #[test]
    fn unwrap_ok() {
        assert_eq!(
            dispatch_builtin_method(Value::ok(Value::int(42)), "unwrap", vec![]).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn unwrap_err_error() {
        assert!(
            dispatch_builtin_method(Value::err(Value::string("error")), "unwrap", vec![]).is_err()
        );
    }

    #[test]
    fn is_ok() {
        assert_eq!(
            dispatch_builtin_method(Value::ok(Value::int(1)), "is_ok", vec![]).unwrap(),
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
            dispatch_builtin_method(Value::ok(Value::int(1)), "is_err", vec![]).unwrap(),
            Value::Bool(false)
        );
    }
}

mod errors {
    use super::*;

    #[test]
    fn no_such_method() {
        assert!(dispatch_builtin_method(Value::list(vec![]), "nonexistent", vec![]).is_err());
        assert!(dispatch_builtin_method(Value::string("hello"), "nonexistent", vec![]).is_err());
        assert!(dispatch_builtin_method(Value::int(42), "len", vec![]).is_err());
    }
}
