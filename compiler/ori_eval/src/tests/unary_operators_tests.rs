//! Tests for unary operator implementations.
//!
//! Relocated from `unary_operators.rs` per coding guidelines (>200 lines).

use crate::unary_operators::evaluate_unary;
use ori_ir::UnaryOp;
use ori_patterns::Value;

mod negation {
    use super::*;

    #[test]
    fn int_positive() {
        assert_eq!(
            evaluate_unary(Value::int(5), UnaryOp::Neg).unwrap(),
            Value::int(-5)
        );
    }

    #[test]
    fn int_negative() {
        assert_eq!(
            evaluate_unary(Value::int(-5), UnaryOp::Neg).unwrap(),
            Value::int(5)
        );
    }

    #[test]
    fn int_zero() {
        assert_eq!(
            evaluate_unary(Value::int(0), UnaryOp::Neg).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn int_max() {
        assert_eq!(
            evaluate_unary(Value::int(i64::MAX), UnaryOp::Neg).unwrap(),
            Value::int(-i64::MAX)
        );
    }

    #[test]
    fn int_min_overflow_errors() {
        let result = evaluate_unary(Value::int(i64::MIN), UnaryOp::Neg);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .into_eval_error()
            .message
            .contains("integer overflow"));
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "Testing float operations, not using PI"
    )]
    fn float_positive() {
        assert_eq!(
            evaluate_unary(Value::Float(3.14), UnaryOp::Neg).unwrap(),
            Value::Float(-3.14)
        );
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "Testing float operations, not using PI"
    )]
    fn float_negative() {
        assert_eq!(
            evaluate_unary(Value::Float(-3.14), UnaryOp::Neg).unwrap(),
            Value::Float(3.14)
        );
    }

    #[test]
    fn float_zero() {
        let result = evaluate_unary(Value::Float(0.0), UnaryOp::Neg).unwrap();
        if let Value::Float(f) = result {
            assert!(f == 0.0);
            assert!(f.is_sign_negative());
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn float_negative_zero() {
        let result = evaluate_unary(Value::Float(-0.0), UnaryOp::Neg).unwrap();
        if let Value::Float(f) = result {
            assert!(f == 0.0);
            assert!(f.is_sign_positive());
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn float_infinity() {
        assert_eq!(
            evaluate_unary(Value::Float(f64::INFINITY), UnaryOp::Neg).unwrap(),
            Value::Float(f64::NEG_INFINITY)
        );
    }

    #[test]
    fn float_neg_infinity() {
        assert_eq!(
            evaluate_unary(Value::Float(f64::NEG_INFINITY), UnaryOp::Neg).unwrap(),
            Value::Float(f64::INFINITY)
        );
    }

    #[test]
    fn float_nan() {
        let result = evaluate_unary(Value::Float(f64::NAN), UnaryOp::Neg).unwrap();
        if let Value::Float(f) = result {
            assert!(f.is_nan());
        } else {
            panic!("expected float");
        }
    }
}

mod logical_not {
    use super::*;

    #[test]
    fn true_becomes_false() {
        assert_eq!(
            evaluate_unary(Value::Bool(true), UnaryOp::Not).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn false_becomes_true() {
        assert_eq!(
            evaluate_unary(Value::Bool(false), UnaryOp::Not).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn double_negation() {
        let once = evaluate_unary(Value::Bool(true), UnaryOp::Not).unwrap();
        let twice = evaluate_unary(once, UnaryOp::Not).unwrap();
        assert_eq!(twice, Value::Bool(true));
    }
}

mod bitwise_not {
    use super::*;

    #[test]
    fn zero_becomes_minus_one() {
        assert_eq!(
            evaluate_unary(Value::int(0), UnaryOp::BitNot).unwrap(),
            Value::int(-1)
        );
    }

    #[test]
    fn minus_one_becomes_zero() {
        assert_eq!(
            evaluate_unary(Value::int(-1), UnaryOp::BitNot).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn positive_value() {
        assert_eq!(
            evaluate_unary(Value::int(5), UnaryOp::BitNot).unwrap(),
            Value::int(-6)
        );
    }

    #[test]
    fn negative_value() {
        assert_eq!(
            evaluate_unary(Value::int(-6), UnaryOp::BitNot).unwrap(),
            Value::int(5)
        );
    }

    #[test]
    fn max_value() {
        assert_eq!(
            evaluate_unary(Value::int(i64::MAX), UnaryOp::BitNot).unwrap(),
            Value::int(i64::MIN)
        );
    }

    #[test]
    fn min_value() {
        assert_eq!(
            evaluate_unary(Value::int(i64::MIN), UnaryOp::BitNot).unwrap(),
            Value::int(i64::MAX)
        );
    }

    #[test]
    fn double_negation_identity() {
        let val = Value::int(12345);
        let once = evaluate_unary(val.clone(), UnaryOp::BitNot).unwrap();
        let twice = evaluate_unary(once, UnaryOp::BitNot).unwrap();
        assert_eq!(twice, val);
    }

    #[test]
    fn powers_of_two() {
        assert_eq!(
            evaluate_unary(Value::int(1), UnaryOp::BitNot).unwrap(),
            Value::int(-2)
        );
        assert_eq!(
            evaluate_unary(Value::int(2), UnaryOp::BitNot).unwrap(),
            Value::int(-3)
        );
        assert_eq!(
            evaluate_unary(Value::int(4), UnaryOp::BitNot).unwrap(),
            Value::int(-5)
        );
    }
}

mod try_operator {
    use super::*;

    #[test]
    fn ok_unwraps() {
        let ok_val = Value::ok(Value::int(42));
        assert_eq!(
            evaluate_unary(ok_val, UnaryOp::Try).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn err_propagates() {
        let err_val = Value::err(Value::string("error"));
        let result = evaluate_unary(err_val, UnaryOp::Try);
        assert!(result.is_err());
    }

    #[test]
    fn some_unwraps() {
        let some_val = Value::some(Value::int(42));
        assert_eq!(
            evaluate_unary(some_val, UnaryOp::Try).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn none_propagates() {
        let result = evaluate_unary(Value::None, UnaryOp::Try);
        assert!(result.is_err());
    }

    #[test]
    fn nested_ok() {
        let nested = Value::ok(Value::ok(Value::int(42)));
        let result = evaluate_unary(nested, UnaryOp::Try).unwrap();
        assert_eq!(result, Value::ok(Value::int(42)));
    }

    #[test]
    fn nested_some() {
        let nested = Value::some(Value::some(Value::int(42)));
        let result = evaluate_unary(nested, UnaryOp::Try).unwrap();
        assert_eq!(result, Value::some(Value::int(42)));
    }

    #[test]
    fn passthrough_int() {
        assert_eq!(
            evaluate_unary(Value::int(42), UnaryOp::Try).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn passthrough_bool() {
        assert_eq!(
            evaluate_unary(Value::Bool(true), UnaryOp::Try).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn passthrough_string() {
        assert_eq!(
            evaluate_unary(Value::string("hello"), UnaryOp::Try).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn passthrough_void() {
        assert_eq!(
            evaluate_unary(Value::Void, UnaryOp::Try).unwrap(),
            Value::Void
        );
    }
}

mod negation_boundaries {
    use super::*;

    #[test]
    fn neg_min_plus_1_is_max() {
        // -(MIN + 1) = MAX
        assert_eq!(
            evaluate_unary(Value::int(i64::MIN + 1), UnaryOp::Neg).unwrap(),
            Value::int(i64::MAX)
        );
    }

    #[test]
    fn neg_max_roundtrip() {
        // -(-MAX) = MAX (double negation)
        let neg_max = evaluate_unary(Value::int(i64::MAX), UnaryOp::Neg).unwrap();
        assert_eq!(neg_max, Value::int(-i64::MAX));
        let back = evaluate_unary(neg_max, UnaryOp::Neg).unwrap();
        assert_eq!(back, Value::int(i64::MAX));
    }

    #[test]
    fn neg_1() {
        assert_eq!(
            evaluate_unary(Value::int(1), UnaryOp::Neg).unwrap(),
            Value::int(-1)
        );
    }

    #[test]
    fn neg_neg1() {
        assert_eq!(
            evaluate_unary(Value::int(-1), UnaryOp::Neg).unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn neg_min_overflow_message() {
        let result = evaluate_unary(Value::int(i64::MIN), UnaryOp::Neg);
        assert!(result.is_err());
        let msg = result.unwrap_err().into_eval_error().message;
        assert!(msg.contains("integer overflow"), "got: {msg}");
        assert!(msg.contains("negation"), "got: {msg}");
    }
}

mod bitwise_not_boundaries {
    use super::*;

    #[test]
    fn not_max_minus_1() {
        // ~(MAX - 1) = MIN + 1
        assert_eq!(
            evaluate_unary(Value::int(i64::MAX - 1), UnaryOp::BitNot).unwrap(),
            Value::int(i64::MIN + 1)
        );
    }

    #[test]
    fn not_min_plus_1() {
        // ~(MIN + 1) = MAX - 1
        assert_eq!(
            evaluate_unary(Value::int(i64::MIN + 1), UnaryOp::BitNot).unwrap(),
            Value::int(i64::MAX - 1)
        );
    }

    #[test]
    fn not_1() {
        // ~1 = -2
        assert_eq!(
            evaluate_unary(Value::int(1), UnaryOp::BitNot).unwrap(),
            Value::int(-2)
        );
    }

    #[test]
    fn not_neg2() {
        // ~(-2) = 1
        assert_eq!(
            evaluate_unary(Value::int(-2), UnaryOp::BitNot).unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn double_not_identity_at_max() {
        let once = evaluate_unary(Value::int(i64::MAX), UnaryOp::BitNot).unwrap();
        let twice = evaluate_unary(once, UnaryOp::BitNot).unwrap();
        assert_eq!(twice, Value::int(i64::MAX));
    }

    #[test]
    fn double_not_identity_at_min() {
        let once = evaluate_unary(Value::int(i64::MIN), UnaryOp::BitNot).unwrap();
        let twice = evaluate_unary(once, UnaryOp::BitNot).unwrap();
        assert_eq!(twice, Value::int(i64::MIN));
    }
}

mod type_errors {
    use super::*;

    #[test]
    fn negate_bool_fails() {
        let result = evaluate_unary(Value::Bool(true), UnaryOp::Neg);
        assert!(result.is_err());
    }

    #[test]
    fn negate_string_fails() {
        let result = evaluate_unary(Value::string("hello"), UnaryOp::Neg);
        assert!(result.is_err());
    }

    #[test]
    fn logical_not_int_fails() {
        let result = evaluate_unary(Value::int(1), UnaryOp::Not);
        assert!(result.is_err());
    }

    #[test]
    fn logical_not_string_fails() {
        let result = evaluate_unary(Value::string("hello"), UnaryOp::Not);
        assert!(result.is_err());
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "Testing float operations, not using PI"
    )]
    fn bitwise_not_float_fails() {
        let result = evaluate_unary(Value::Float(3.14), UnaryOp::BitNot);
        assert!(result.is_err());
    }

    #[test]
    fn bitwise_not_bool_fails() {
        let result = evaluate_unary(Value::Bool(true), UnaryOp::BitNot);
        assert!(result.is_err());
    }
}
