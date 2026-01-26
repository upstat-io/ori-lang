//! Tests for unary operator evaluation.
//!
//! Tests negation, logical not, bitwise not, and try operator.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use sigil_eval::UnaryOperatorRegistry;
use crate::eval::Value;
use sigil_ir::UnaryOp;

// Negation Tests

mod negation {
    use super::*;

    #[test]
    fn int_positive() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(5), UnaryOp::Neg).unwrap(),
            Value::Int(-5)
        );
    }

    #[test]
    fn int_negative() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(-5), UnaryOp::Neg).unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn int_zero() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(0), UnaryOp::Neg).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn int_max() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(i64::MAX), UnaryOp::Neg).unwrap(),
            Value::Int(-i64::MAX)
        );
    }

    #[test]
    #[should_panic(expected = "negate with overflow")]
    fn int_min_overflow_panics() {
        let registry = UnaryOperatorRegistry::new();
        // -i64::MIN overflows in debug mode
        let _ = registry.evaluate(Value::Int(i64::MIN), UnaryOp::Neg);
    }

    #[test]
    fn float_positive() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Float(3.14), UnaryOp::Neg).unwrap(),
            Value::Float(-3.14)
        );
    }

    #[test]
    fn float_negative() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Float(-3.14), UnaryOp::Neg).unwrap(),
            Value::Float(3.14)
        );
    }

    #[test]
    fn float_zero() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Float(0.0), UnaryOp::Neg).unwrap();
        if let Value::Float(f) = result {
            assert!(f == 0.0);
            // Check it's negative zero
            assert!(f.is_sign_negative());
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn float_negative_zero() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Float(-0.0), UnaryOp::Neg).unwrap();
        if let Value::Float(f) = result {
            assert!(f == 0.0);
            // Negating -0.0 gives +0.0
            assert!(f.is_sign_positive());
        } else {
            panic!("expected float");
        }
    }

    #[test]
    fn float_infinity() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Float(f64::INFINITY), UnaryOp::Neg).unwrap(),
            Value::Float(f64::NEG_INFINITY)
        );
    }

    #[test]
    fn float_neg_infinity() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Float(f64::NEG_INFINITY), UnaryOp::Neg).unwrap(),
            Value::Float(f64::INFINITY)
        );
    }

    #[test]
    fn float_nan() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Float(f64::NAN), UnaryOp::Neg).unwrap();
        if let Value::Float(f) = result {
            assert!(f.is_nan());
        } else {
            panic!("expected float");
        }
    }
}

// Logical Not Tests

mod logical_not {
    use super::*;

    #[test]
    fn true_becomes_false() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Bool(true), UnaryOp::Not).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn false_becomes_true() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Bool(false), UnaryOp::Not).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn double_negation() {
        let registry = UnaryOperatorRegistry::new();
        let once = registry.evaluate(Value::Bool(true), UnaryOp::Not).unwrap();
        let twice = registry.evaluate(once, UnaryOp::Not).unwrap();
        assert_eq!(twice, Value::Bool(true));
    }
}

// Bitwise Not Tests

mod bitwise_not {
    use super::*;

    #[test]
    fn zero_becomes_minus_one() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(0), UnaryOp::BitNot).unwrap(),
            Value::Int(-1)
        );
    }

    #[test]
    fn minus_one_becomes_zero() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(-1), UnaryOp::BitNot).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn positive_value() {
        let registry = UnaryOperatorRegistry::new();
        // ~5 = -6 in two's complement
        assert_eq!(
            registry.evaluate(Value::Int(5), UnaryOp::BitNot).unwrap(),
            Value::Int(-6)
        );
    }

    #[test]
    fn negative_value() {
        let registry = UnaryOperatorRegistry::new();
        // ~(-6) = 5
        assert_eq!(
            registry.evaluate(Value::Int(-6), UnaryOp::BitNot).unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn max_value() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(i64::MAX), UnaryOp::BitNot).unwrap(),
            Value::Int(i64::MIN)
        );
    }

    #[test]
    fn min_value() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Int(i64::MIN), UnaryOp::BitNot).unwrap(),
            Value::Int(i64::MAX)
        );
    }

    #[test]
    fn double_negation_identity() {
        let registry = UnaryOperatorRegistry::new();
        let val = Value::Int(12345);
        let once = registry.evaluate(val.clone(), UnaryOp::BitNot).unwrap();
        let twice = registry.evaluate(once, UnaryOp::BitNot).unwrap();
        assert_eq!(twice, val);
    }

    #[test]
    fn powers_of_two() {
        let registry = UnaryOperatorRegistry::new();
        // ~1 = -2
        assert_eq!(
            registry.evaluate(Value::Int(1), UnaryOp::BitNot).unwrap(),
            Value::Int(-2)
        );
        // ~2 = -3
        assert_eq!(
            registry.evaluate(Value::Int(2), UnaryOp::BitNot).unwrap(),
            Value::Int(-3)
        );
        // ~4 = -5
        assert_eq!(
            registry.evaluate(Value::Int(4), UnaryOp::BitNot).unwrap(),
            Value::Int(-5)
        );
    }
}

// Try Operator Tests

mod try_operator {
    use super::*;

    #[test]
    fn ok_unwraps() {
        let registry = UnaryOperatorRegistry::new();
        let ok_val = Value::ok(Value::Int(42));
        assert_eq!(
            registry.evaluate(ok_val, UnaryOp::Try).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn err_propagates() {
        let registry = UnaryOperatorRegistry::new();
        let err_val = Value::err(Value::string("error"));
        let result = registry.evaluate(err_val, UnaryOp::Try);
        assert!(result.is_err());
    }

    #[test]
    fn some_unwraps() {
        let registry = UnaryOperatorRegistry::new();
        let some_val = Value::some(Value::Int(42));
        assert_eq!(
            registry.evaluate(some_val, UnaryOp::Try).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn none_propagates() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::None, UnaryOp::Try);
        assert!(result.is_err());
    }

    #[test]
    fn nested_ok() {
        let registry = UnaryOperatorRegistry::new();
        let nested = Value::ok(Value::ok(Value::Int(42)));
        let result = registry.evaluate(nested, UnaryOp::Try).unwrap();
        assert_eq!(result, Value::ok(Value::Int(42)));
    }

    #[test]
    fn nested_some() {
        let registry = UnaryOperatorRegistry::new();
        let nested = Value::some(Value::some(Value::Int(42)));
        let result = registry.evaluate(nested, UnaryOp::Try).unwrap();
        assert_eq!(result, Value::some(Value::Int(42)));
    }

    #[test]
    fn passthrough_int() {
        let registry = UnaryOperatorRegistry::new();
        // Non-Option/Result types pass through unchanged
        assert_eq!(
            registry.evaluate(Value::Int(42), UnaryOp::Try).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn passthrough_bool() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Bool(true), UnaryOp::Try).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn passthrough_string() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::string("hello"), UnaryOp::Try).unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn passthrough_void() {
        let registry = UnaryOperatorRegistry::new();
        assert_eq!(
            registry.evaluate(Value::Void, UnaryOp::Try).unwrap(),
            Value::Void
        );
    }
}

// Type Error Tests

mod type_errors {
    use super::*;

    #[test]
    fn negate_bool_fails() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Bool(true), UnaryOp::Neg);
        assert!(result.is_err());
    }

    #[test]
    fn negate_string_fails() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::string("hello"), UnaryOp::Neg);
        assert!(result.is_err());
    }

    #[test]
    fn logical_not_int_fails() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Int(1), UnaryOp::Not);
        assert!(result.is_err());
    }

    #[test]
    fn logical_not_string_fails() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::string("hello"), UnaryOp::Not);
        assert!(result.is_err());
    }

    #[test]
    fn bitwise_not_float_fails() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Float(3.14), UnaryOp::BitNot);
        assert!(result.is_err());
    }

    #[test]
    fn bitwise_not_bool_fails() {
        let registry = UnaryOperatorRegistry::new();
        let result = registry.evaluate(Value::Bool(true), UnaryOp::BitNot);
        assert!(result.is_err());
    }
}
