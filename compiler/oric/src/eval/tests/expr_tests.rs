//! Tests for expression evaluation (literals, operators, indexing, field access).

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::exec::expr::{eval_index, get_collection_length};
use crate::eval::Value;
use crate::ir::BinaryOp;
use ori_eval::evaluate_binary;

// Binary Value Evaluation Tests (Index Context)

mod binary_values {
    use super::*;

    #[test]
    fn add() {
        let result = evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Add);
        assert_eq!(result.unwrap(), Value::int(5));
    }

    #[test]
    fn sub() {
        let result = evaluate_binary(Value::int(5), Value::int(3), BinaryOp::Sub);
        assert_eq!(result.unwrap(), Value::int(2));
    }

    #[test]
    fn mul() {
        let result = evaluate_binary(Value::int(4), Value::int(3), BinaryOp::Mul);
        assert_eq!(result.unwrap(), Value::int(12));
    }

    #[test]
    fn div() {
        let result = evaluate_binary(Value::int(10), Value::int(3), BinaryOp::Div);
        assert_eq!(result.unwrap(), Value::int(3));
    }

    #[test]
    fn div_by_zero_error() {
        let result = evaluate_binary(Value::int(10), Value::int(0), BinaryOp::Div);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .into_eval_error()
            .message
            .contains("division by zero"));
    }

    #[test]
    fn eq() {
        let result = evaluate_binary(Value::int(1), Value::int(1), BinaryOp::Eq);
        assert_eq!(result.unwrap(), Value::Bool(true));
    }

    #[test]
    fn float_add() {
        let result = evaluate_binary(Value::Float(1.0), Value::Float(2.0), BinaryOp::Add);
        assert_eq!(result.unwrap(), Value::Float(3.0));
    }
}

// Collection Length Tests

mod collection_length {
    use super::*;

    #[test]
    fn list_empty() {
        let list = Value::list(vec![]);
        assert_eq!(get_collection_length(&list).unwrap(), 0);
    }

    #[test]
    fn list_with_items() {
        let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
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
        let t = Value::tuple(vec![Value::int(1), Value::int(2)]);
        assert_eq!(get_collection_length(&t).unwrap(), 2);
    }

    #[test]
    fn map_empty() {
        let m = Value::map(std::collections::BTreeMap::new());
        assert_eq!(get_collection_length(&m).unwrap(), 0);
    }

    #[test]
    fn map_with_items() {
        let mut map = std::collections::BTreeMap::new();
        map.insert("a".to_string(), Value::int(1));
        map.insert("b".to_string(), Value::int(2));
        let m = Value::map(map);
        assert_eq!(get_collection_length(&m).unwrap(), 2);
    }

    #[test]
    fn int_error() {
        let result = get_collection_length(&Value::int(42));
        assert!(result.is_err());
    }
}

// Index Access Tests

mod index_access {
    use super::*;

    mod list_indexing {
        use super::*;

        #[test]
        fn first_element() {
            let list = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
            assert_eq!(eval_index(list, Value::int(0)).unwrap(), Value::int(10));
        }

        #[test]
        fn middle_element() {
            let list = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
            assert_eq!(eval_index(list, Value::int(1)).unwrap(), Value::int(20));
        }

        #[test]
        fn last_element() {
            let list = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
            assert_eq!(eval_index(list, Value::int(2)).unwrap(), Value::int(30));
        }

        #[test]
        fn negative_index_last() {
            let list = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
            assert_eq!(eval_index(list, Value::int(-1)).unwrap(), Value::int(30));
        }

        #[test]
        fn negative_index_first() {
            let list = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
            assert_eq!(eval_index(list, Value::int(-3)).unwrap(), Value::int(10));
        }

        #[test]
        fn negative_index_middle() {
            let list = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
            assert_eq!(eval_index(list, Value::int(-2)).unwrap(), Value::int(20));
        }

        #[test]
        fn out_of_bounds_positive() {
            let list = Value::list(vec![Value::int(1)]);
            let result = eval_index(list, Value::int(5));
            assert!(result.is_err());
        }

        #[test]
        fn out_of_bounds_negative() {
            let list = Value::list(vec![Value::int(1)]);
            let result = eval_index(list, Value::int(-5));
            assert!(result.is_err());
        }

        #[test]
        fn empty_list() {
            let list = Value::list(vec![]);
            let result = eval_index(list, Value::int(0));
            assert!(result.is_err());
        }

        #[test]
        fn single_element() {
            let list = Value::list(vec![Value::int(42)]);
            assert_eq!(
                eval_index(list.clone(), Value::int(0)).unwrap(),
                Value::int(42)
            );
            assert_eq!(eval_index(list, Value::int(-1)).unwrap(), Value::int(42));
        }
    }

    mod string_indexing {
        use super::*;

        // String indexing returns a single-codepoint str (not char)

        #[test]
        fn first_char() {
            let s = Value::string("hello");
            assert_eq!(eval_index(s, Value::int(0)).unwrap(), Value::string("h"));
        }

        #[test]
        fn last_char() {
            let s = Value::string("hello");
            assert_eq!(eval_index(s, Value::int(4)).unwrap(), Value::string("o"));
        }

        #[test]
        fn negative_index() {
            let s = Value::string("hello");
            assert_eq!(eval_index(s, Value::int(-1)).unwrap(), Value::string("o"));
        }

        #[test]
        fn unicode_char() {
            let s = Value::string("héllo");
            assert_eq!(eval_index(s, Value::int(1)).unwrap(), Value::string("é"));
        }

        #[test]
        fn emoji() {
            let s = Value::string("a😀b");
            assert_eq!(
                eval_index(s.clone(), Value::int(0)).unwrap(),
                Value::string("a")
            );
            assert_eq!(
                eval_index(s.clone(), Value::int(1)).unwrap(),
                Value::string("😀")
            );
            assert_eq!(eval_index(s, Value::int(2)).unwrap(), Value::string("b"));
        }

        #[test]
        fn out_of_bounds() {
            let s = Value::string("hi");
            let result = eval_index(s, Value::int(10));
            assert!(result.is_err());
        }

        #[test]
        fn empty_string() {
            let s = Value::string("");
            let result = eval_index(s, Value::int(0));
            assert!(result.is_err());
        }
    }

    mod map_indexing {
        use super::*;

        // Map indexing returns Option<V>: Some(value) if found, None if not

        #[test]
        fn existing_key() {
            let mut map = std::collections::BTreeMap::new();
            // Map keys must use type-prefixed format (e.g., "s:key" for strings)
            // This matches how the interpreter stores keys via Value::to_map_key()
            map.insert("s:key".to_string(), Value::int(42));
            let m = Value::map(map);
            assert_eq!(
                eval_index(m, Value::string("key")).unwrap(),
                Value::some(Value::int(42))
            );
        }

        #[test]
        fn missing_key() {
            let map: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
            let m = Value::map(map);
            assert_eq!(
                eval_index(m, Value::string("missing")).unwrap(),
                Value::None
            );
        }

        #[test]
        fn empty_string_key() {
            let mut map = std::collections::BTreeMap::new();
            // Empty string key is "s:" (type prefix only)
            map.insert("s:".to_string(), Value::int(1));
            let m = Value::map(map);
            assert_eq!(
                eval_index(m, Value::string("")).unwrap(),
                Value::some(Value::int(1))
            );
        }
    }

    mod type_errors {
        use super::*;

        #[test]
        fn int_not_indexable() {
            let result = eval_index(Value::int(42), Value::int(0));
            assert!(result.is_err());
        }

        #[test]
        fn bool_not_indexable() {
            let result = eval_index(Value::Bool(true), Value::int(0));
            assert!(result.is_err());
        }

        #[test]
        fn list_with_string_index() {
            let list = Value::list(vec![Value::int(1)]);
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

// Boundary Tests

mod boundaries {
    use super::*;

    #[test]
    fn large_list_first() {
        let items: Vec<Value> = (0..10000).map(Value::int).collect();
        let list = Value::list(items);
        assert_eq!(eval_index(list, Value::int(0)).unwrap(), Value::int(0));
    }

    #[test]
    fn large_list_last() {
        let items: Vec<Value> = (0..10000).map(Value::int).collect();
        let list = Value::list(items);
        assert_eq!(
            eval_index(list.clone(), Value::int(9999)).unwrap(),
            Value::int(9999)
        );
        assert_eq!(eval_index(list, Value::int(-1)).unwrap(), Value::int(9999));
    }

    #[test]
    fn long_string_first() {
        let s = Value::string("a".repeat(10000));
        // String indexing returns single-codepoint str
        assert_eq!(eval_index(s, Value::int(0)).unwrap(), Value::string("a"));
    }

    #[test]
    fn long_string_last() {
        let s = Value::string("a".repeat(10000));
        // String indexing returns single-codepoint str
        assert_eq!(
            eval_index(s.clone(), Value::int(9999)).unwrap(),
            Value::string("a")
        );
        assert_eq!(eval_index(s, Value::int(-1)).unwrap(), Value::string("a"));
    }

    #[test]
    fn index_at_boundary() {
        let list = Value::list(vec![Value::int(0), Value::int(1)]);
        // Boundary checks
        assert!(eval_index(list.clone(), Value::int(1)).is_ok());
        assert!(eval_index(list.clone(), Value::int(2)).is_err());
        assert!(eval_index(list.clone(), Value::int(-2)).is_ok());
        assert!(eval_index(list, Value::int(-3)).is_err());
    }
}
