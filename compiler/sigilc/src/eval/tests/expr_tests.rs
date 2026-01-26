//! Tests for expression evaluation (literals, operators, indexing, field access).

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::exec::expr::{
    eval_literal, eval_binary_values, get_collection_length, eval_index,
};
use crate::eval::Value;
use crate::ir::{ExprKind, BinaryOp, SharedInterner};

// =============================================================================
// Literal Evaluation Tests
// =============================================================================

mod literals {
    use super::*;

    #[test]
    fn int() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Int(42), &interner);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), Value::Int(42));
    }

    #[test]
    fn int_zero() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Int(0), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Int(0));
    }

    #[test]
    fn int_negative() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Int(-42), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Int(-42));
    }

    #[test]
    fn int_max() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Int(i64::MAX), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Int(i64::MAX));
    }

    #[test]
    fn int_min() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Int(i64::MIN), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Int(i64::MIN));
    }

    #[test]
    fn float() {
        let interner = SharedInterner::default();
        let bits = 3.14_f64.to_bits();
        let result = eval_literal(&ExprKind::Float(bits), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Float(3.14));
    }

    #[test]
    fn float_zero() {
        let interner = SharedInterner::default();
        let bits = 0.0_f64.to_bits();
        let result = eval_literal(&ExprKind::Float(bits), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Float(0.0));
    }

    #[test]
    fn bool_true() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Bool(true), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Bool(true));
    }

    #[test]
    fn bool_false() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Bool(false), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Bool(false));
    }

    #[test]
    fn unit() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Unit, &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Void);
    }

    #[test]
    fn char_ascii() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Char('a'), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Char('a'));
    }

    #[test]
    fn char_unicode() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Char('λ'), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Char('λ'));
    }

    #[test]
    fn char_emoji() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Char('😀'), &interner);
        assert_eq!(result.unwrap().unwrap(), Value::Char('😀'));
    }

    #[test]
    fn non_literal_returns_none() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Error, &interner);
        assert!(result.is_none());
    }
}

// =============================================================================
// Binary Value Evaluation Tests (Index Context)
// =============================================================================

mod binary_values {
    use super::*;

    #[test]
    fn add() {
        let result = eval_binary_values(Value::Int(2), BinaryOp::Add, Value::Int(3));
        assert_eq!(result.unwrap(), Value::Int(5));
    }

    #[test]
    fn sub() {
        let result = eval_binary_values(Value::Int(5), BinaryOp::Sub, Value::Int(3));
        assert_eq!(result.unwrap(), Value::Int(2));
    }

    #[test]
    fn mul() {
        let result = eval_binary_values(Value::Int(4), BinaryOp::Mul, Value::Int(3));
        assert_eq!(result.unwrap(), Value::Int(12));
    }

    #[test]
    fn div() {
        let result = eval_binary_values(Value::Int(10), BinaryOp::Div, Value::Int(3));
        assert_eq!(result.unwrap(), Value::Int(3));
    }

    #[test]
    fn div_by_zero_error() {
        let result = eval_binary_values(Value::Int(10), BinaryOp::Div, Value::Int(0));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("division by zero"));
    }

    #[test]
    fn unsupported_op_error() {
        let result = eval_binary_values(Value::Int(1), BinaryOp::Eq, Value::Int(1));
        assert!(result.is_err());
    }

    #[test]
    fn non_integer_error() {
        let result = eval_binary_values(Value::Float(1.0), BinaryOp::Add, Value::Float(2.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("non-integer"));
    }
}

// =============================================================================
// Collection Length Tests
// =============================================================================

mod collection_length {
    use super::*;

    #[test]
    fn list_empty() {
        let list = Value::list(vec![]);
        assert_eq!(get_collection_length(&list).unwrap(), 0);
    }

    #[test]
    fn list_with_items() {
        let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(get_collection_length(&list).unwrap(), 3);
    }

    #[test]
    fn string_empty() {
        let s = Value::string("");
        assert_eq!(get_collection_length(&s).unwrap(), 0);
    }

    #[test]
    fn string_ascii() {
        let s = Value::string("hello");
        assert_eq!(get_collection_length(&s).unwrap(), 5);
    }

    #[test]
    fn string_unicode() {
        // Unicode string: "hello" in Greek
        let s = Value::string("γεια");
        // 4 characters (not 8 bytes)
        assert_eq!(get_collection_length(&s).unwrap(), 4);
    }

    #[test]
    fn string_emoji() {
        let s = Value::string("😀😁😂");
        // 3 emoji characters
        assert_eq!(get_collection_length(&s).unwrap(), 3);
    }

    #[test]
    fn tuple_empty() {
        let t = Value::tuple(vec![]);
        assert_eq!(get_collection_length(&t).unwrap(), 0);
    }

    #[test]
    fn tuple_with_items() {
        let t = Value::tuple(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(get_collection_length(&t).unwrap(), 2);
    }

    #[test]
    fn map_empty() {
        let m = Value::map(std::collections::HashMap::new());
        assert_eq!(get_collection_length(&m).unwrap(), 0);
    }

    #[test]
    fn map_with_items() {
        let mut map = std::collections::HashMap::new();
        map.insert("a".to_string(), Value::Int(1));
        map.insert("b".to_string(), Value::Int(2));
        let m = Value::map(map);
        assert_eq!(get_collection_length(&m).unwrap(), 2);
    }

    #[test]
    fn int_error() {
        let result = get_collection_length(&Value::Int(42));
        assert!(result.is_err());
    }
}

// =============================================================================
// Index Access Tests
// =============================================================================

mod index_access {
    use super::*;

    mod list_indexing {
        use super::*;

        #[test]
        fn first_element() {
            let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
            assert_eq!(eval_index(list, Value::Int(0)).unwrap(), Value::Int(10));
        }

        #[test]
        fn middle_element() {
            let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
            assert_eq!(eval_index(list, Value::Int(1)).unwrap(), Value::Int(20));
        }

        #[test]
        fn last_element() {
            let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
            assert_eq!(eval_index(list, Value::Int(2)).unwrap(), Value::Int(30));
        }

        #[test]
        fn negative_index_last() {
            let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
            assert_eq!(eval_index(list, Value::Int(-1)).unwrap(), Value::Int(30));
        }

        #[test]
        fn negative_index_first() {
            let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
            assert_eq!(eval_index(list, Value::Int(-3)).unwrap(), Value::Int(10));
        }

        #[test]
        fn negative_index_middle() {
            let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
            assert_eq!(eval_index(list, Value::Int(-2)).unwrap(), Value::Int(20));
        }

        #[test]
        fn out_of_bounds_positive() {
            let list = Value::list(vec![Value::Int(1)]);
            let result = eval_index(list, Value::Int(5));
            assert!(result.is_err());
        }

        #[test]
        fn out_of_bounds_negative() {
            let list = Value::list(vec![Value::Int(1)]);
            let result = eval_index(list, Value::Int(-5));
            assert!(result.is_err());
        }

        #[test]
        fn empty_list() {
            let list = Value::list(vec![]);
            let result = eval_index(list, Value::Int(0));
            assert!(result.is_err());
        }

        #[test]
        fn single_element() {
            let list = Value::list(vec![Value::Int(42)]);
            assert_eq!(eval_index(list.clone(), Value::Int(0)).unwrap(), Value::Int(42));
            assert_eq!(eval_index(list, Value::Int(-1)).unwrap(), Value::Int(42));
        }
    }

    mod string_indexing {
        use super::*;

        #[test]
        fn first_char() {
            let s = Value::string("hello");
            assert_eq!(eval_index(s, Value::Int(0)).unwrap(), Value::Char('h'));
        }

        #[test]
        fn last_char() {
            let s = Value::string("hello");
            assert_eq!(eval_index(s, Value::Int(4)).unwrap(), Value::Char('o'));
        }

        #[test]
        fn negative_index() {
            let s = Value::string("hello");
            assert_eq!(eval_index(s, Value::Int(-1)).unwrap(), Value::Char('o'));
        }

        #[test]
        fn unicode_char() {
            let s = Value::string("héllo");
            assert_eq!(eval_index(s, Value::Int(1)).unwrap(), Value::Char('é'));
        }

        #[test]
        fn emoji() {
            let s = Value::string("a😀b");
            assert_eq!(eval_index(s.clone(), Value::Int(0)).unwrap(), Value::Char('a'));
            assert_eq!(eval_index(s.clone(), Value::Int(1)).unwrap(), Value::Char('😀'));
            assert_eq!(eval_index(s, Value::Int(2)).unwrap(), Value::Char('b'));
        }

        #[test]
        fn out_of_bounds() {
            let s = Value::string("hi");
            let result = eval_index(s, Value::Int(10));
            assert!(result.is_err());
        }

        #[test]
        fn empty_string() {
            let s = Value::string("");
            let result = eval_index(s, Value::Int(0));
            assert!(result.is_err());
        }
    }

    mod map_indexing {
        use super::*;

        #[test]
        fn existing_key() {
            let mut map = std::collections::HashMap::new();
            map.insert("key".to_string(), Value::Int(42));
            let m = Value::map(map);
            assert_eq!(eval_index(m, Value::string("key")).unwrap(), Value::Int(42));
        }

        #[test]
        fn missing_key() {
            let map: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
            let m = Value::map(map);
            let result = eval_index(m, Value::string("missing"));
            assert!(result.is_err());
        }

        #[test]
        fn empty_string_key() {
            let mut map = std::collections::HashMap::new();
            map.insert("".to_string(), Value::Int(1));
            let m = Value::map(map);
            assert_eq!(eval_index(m, Value::string("")).unwrap(), Value::Int(1));
        }
    }

    mod type_errors {
        use super::*;

        #[test]
        fn int_not_indexable() {
            let result = eval_index(Value::Int(42), Value::Int(0));
            assert!(result.is_err());
        }

        #[test]
        fn bool_not_indexable() {
            let result = eval_index(Value::Bool(true), Value::Int(0));
            assert!(result.is_err());
        }

        #[test]
        fn list_with_string_index() {
            let list = Value::list(vec![Value::Int(1)]);
            let result = eval_index(list, Value::string("0"));
            assert!(result.is_err());
        }

        #[test]
        fn string_with_string_index() {
            let s = Value::string("hello");
            let result = eval_index(s, Value::string("0"));
            assert!(result.is_err());
        }
    }
}

// =============================================================================
// Boundary Tests
// =============================================================================

mod boundaries {
    use super::*;

    #[test]
    fn large_list_first() {
        let items: Vec<Value> = (0..10000).map(Value::Int).collect();
        let list = Value::list(items);
        assert_eq!(eval_index(list, Value::Int(0)).unwrap(), Value::Int(0));
    }

    #[test]
    fn large_list_last() {
        let items: Vec<Value> = (0..10000).map(Value::Int).collect();
        let list = Value::list(items);
        assert_eq!(eval_index(list.clone(), Value::Int(9999)).unwrap(), Value::Int(9999));
        assert_eq!(eval_index(list, Value::Int(-1)).unwrap(), Value::Int(9999));
    }

    #[test]
    fn long_string_first() {
        let s = Value::string(&"a".repeat(10000));
        assert_eq!(eval_index(s, Value::Int(0)).unwrap(), Value::Char('a'));
    }

    #[test]
    fn long_string_last() {
        let s = Value::string(&"a".repeat(10000));
        assert_eq!(eval_index(s.clone(), Value::Int(9999)).unwrap(), Value::Char('a'));
        assert_eq!(eval_index(s, Value::Int(-1)).unwrap(), Value::Char('a'));
    }

    #[test]
    fn index_at_boundary() {
        let list = Value::list(vec![Value::Int(0), Value::Int(1)]);
        // Boundary checks
        assert!(eval_index(list.clone(), Value::Int(1)).is_ok());
        assert!(eval_index(list.clone(), Value::Int(2)).is_err());
        assert!(eval_index(list.clone(), Value::Int(-2)).is_ok());
        assert!(eval_index(list, Value::Int(-3)).is_err());
    }
}
