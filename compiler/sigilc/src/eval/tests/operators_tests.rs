//! Tests for binary operators.
//!
//! Tests operator evaluation including arithmetic, comparison, bitwise,
//! and type checking.

use sigil_eval::OperatorRegistry;
use crate::eval::Value;
use sigil_ir::BinaryOp;

// =============================================================================
// Integer operations
// =============================================================================

mod int_ops {
    use super::*;

    #[test]
    fn arithmetic() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(3), BinaryOp::Add).unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            registry.evaluate(Value::Int(5), Value::Int(3), BinaryOp::Sub).unwrap(),
            Value::Int(2)
        );
        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(3), BinaryOp::Mul).unwrap(),
            Value::Int(6)
        );
        assert_eq!(
            registry.evaluate(Value::Int(10), Value::Int(3), BinaryOp::Div).unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            registry.evaluate(Value::Int(10), Value::Int(3), BinaryOp::Mod).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn floor_division() {
        let registry = OperatorRegistry::new();

        // Positive numbers
        assert_eq!(
            registry.evaluate(Value::Int(7), Value::Int(3), BinaryOp::FloorDiv).unwrap(),
            Value::Int(2)
        );
        // Negative dividend - should round toward negative infinity
        assert_eq!(
            registry.evaluate(Value::Int(-7), Value::Int(3), BinaryOp::FloorDiv).unwrap(),
            Value::Int(-3)
        );
        // Negative divisor
        assert_eq!(
            registry.evaluate(Value::Int(7), Value::Int(-3), BinaryOp::FloorDiv).unwrap(),
            Value::Int(-3)
        );
        // Both negative
        assert_eq!(
            registry.evaluate(Value::Int(-7), Value::Int(-3), BinaryOp::FloorDiv).unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn comparison() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(3), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Int(3), Value::Int(2), BinaryOp::Gt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(2), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(3), BinaryOp::NotEq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(2), BinaryOp::LtEq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Int(2), Value::Int(2), BinaryOp::GtEq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn bitwise() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Int(0b1100), Value::Int(0b1010), BinaryOp::BitAnd).unwrap(),
            Value::Int(0b1000)
        );
        assert_eq!(
            registry.evaluate(Value::Int(0b1100), Value::Int(0b1010), BinaryOp::BitOr).unwrap(),
            Value::Int(0b1110)
        );
        assert_eq!(
            registry.evaluate(Value::Int(0b1100), Value::Int(0b1010), BinaryOp::BitXor).unwrap(),
            Value::Int(0b0110)
        );
    }

    #[test]
    fn shift() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Int(1), Value::Int(4), BinaryOp::Shl).unwrap(),
            Value::Int(16)
        );
        assert_eq!(
            registry.evaluate(Value::Int(16), Value::Int(2), BinaryOp::Shr).unwrap(),
            Value::Int(4)
        );
    }

    #[test]
    fn shift_out_of_range() {
        let registry = OperatorRegistry::new();

        // Shift by negative amount should error
        assert!(registry.evaluate(Value::Int(1), Value::Int(-1), BinaryOp::Shl).is_err());
        // Shift by >= 64 should error
        assert!(registry.evaluate(Value::Int(1), Value::Int(64), BinaryOp::Shl).is_err());
        assert!(registry.evaluate(Value::Int(1), Value::Int(100), BinaryOp::Shr).is_err());
    }

    #[test]
    fn division_by_zero() {
        let registry = OperatorRegistry::new();

        assert!(registry.evaluate(Value::Int(5), Value::Int(0), BinaryOp::Div).is_err());
        assert!(registry.evaluate(Value::Int(5), Value::Int(0), BinaryOp::Mod).is_err());
        assert!(registry.evaluate(Value::Int(5), Value::Int(0), BinaryOp::FloorDiv).is_err());
    }

    #[test]
    fn range() {
        let registry = OperatorRegistry::new();

        let result = registry.evaluate(Value::Int(1), Value::Int(5), BinaryOp::Range).unwrap();
        if let Value::Range(r) = result {
            assert_eq!(r.start, 1);
            assert_eq!(r.end, 5);
            assert!(!r.inclusive);
        } else {
            panic!("Expected Range");
        }

        let result = registry.evaluate(Value::Int(1), Value::Int(5), BinaryOp::RangeInclusive).unwrap();
        if let Value::Range(r) = result {
            assert!(r.inclusive);
        } else {
            panic!("Expected Range");
        }
    }
}

// =============================================================================
// Float operations
// =============================================================================

mod float_ops {
    use super::*;

    #[test]
    fn arithmetic() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Float(2.0), Value::Float(3.0), BinaryOp::Add).unwrap(),
            Value::Float(5.0)
        );
        assert_eq!(
            registry.evaluate(Value::Float(5.0), Value::Float(3.0), BinaryOp::Sub).unwrap(),
            Value::Float(2.0)
        );
        assert_eq!(
            registry.evaluate(Value::Float(2.0), Value::Float(3.0), BinaryOp::Mul).unwrap(),
            Value::Float(6.0)
        );
        assert_eq!(
            registry.evaluate(Value::Float(6.0), Value::Float(3.0), BinaryOp::Div).unwrap(),
            Value::Float(2.0)
        );
    }

    #[test]
    fn comparison() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Float(2.0), Value::Float(3.0), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Float(2.0), Value::Float(2.0), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn nan_comparison() {
        let registry = OperatorRegistry::new();

        // NaN != NaN (IEEE 754)
        assert_eq!(
            registry.evaluate(Value::Float(f64::NAN), Value::Float(f64::NAN), BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            registry.evaluate(Value::Float(f64::NAN), Value::Float(f64::NAN), BinaryOp::NotEq).unwrap(),
            Value::Bool(true)
        );
        // NaN comparisons are all false
        assert_eq!(
            registry.evaluate(Value::Float(f64::NAN), Value::Float(1.0), BinaryOp::Lt).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            registry.evaluate(Value::Float(f64::NAN), Value::Float(1.0), BinaryOp::Gt).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn negative_zero() {
        let registry = OperatorRegistry::new();

        // -0.0 == 0.0 (IEEE 754)
        assert_eq!(
            registry.evaluate(Value::Float(-0.0), Value::Float(0.0), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn infinity() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Float(f64::INFINITY), Value::Float(f64::INFINITY), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Float(1.0), Value::Float(f64::INFINITY), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
    }
}

// =============================================================================
// String operations
// =============================================================================

mod string_ops {
    use super::*;

    #[test]
    fn concat() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::string("hello"), Value::string(" world"), BinaryOp::Add).unwrap(),
            Value::string("hello world")
        );
        assert_eq!(
            registry.evaluate(Value::string(""), Value::string("test"), BinaryOp::Add).unwrap(),
            Value::string("test")
        );
    }

    #[test]
    fn comparison() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::string("abc"), Value::string("abc"), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::string("abc"), Value::string("def"), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
    }
}

// =============================================================================
// Boolean operations
// =============================================================================

mod bool_ops {
    use super::*;

    #[test]
    fn equality() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Bool(true), Value::Bool(true), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::Bool(true), Value::Bool(false), BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            registry.evaluate(Value::Bool(true), Value::Bool(false), BinaryOp::NotEq).unwrap(),
            Value::Bool(true)
        );
    }
}

// =============================================================================
// List operations
// =============================================================================

mod list_ops {
    use super::*;

    #[test]
    fn concat() {
        let registry = OperatorRegistry::new();

        let result = registry.evaluate(
            Value::list(vec![Value::Int(1), Value::Int(2)]),
            Value::list(vec![Value::Int(3), Value::Int(4)]),
            BinaryOp::Add,
        ).unwrap();

        assert_eq!(result, Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]));
    }

    #[test]
    fn equality() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(
                Value::list(vec![Value::Int(1), Value::Int(2)]),
                Value::list(vec![Value::Int(1), Value::Int(2)]),
                BinaryOp::Eq
            ).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(
                Value::list(vec![Value::Int(1)]),
                Value::list(vec![Value::Int(2)]),
                BinaryOp::Eq
            ).unwrap(),
            Value::Bool(false)
        );
    }
}

// =============================================================================
// Type errors
// =============================================================================

mod type_errors {
    use super::*;

    #[test]
    fn mixed_int_float() {
        // Per spec: "No implicit conversions" - int + float is a type error
        let registry = OperatorRegistry::new();

        assert!(registry.evaluate(Value::Int(2), Value::Float(3.0), BinaryOp::Add).is_err());
        assert!(registry.evaluate(Value::Float(2.0), Value::Int(3), BinaryOp::Add).is_err());
    }

    #[test]
    fn incompatible_types() {
        let registry = OperatorRegistry::new();

        assert!(registry.evaluate(Value::Int(1), Value::Bool(true), BinaryOp::Add).is_err());
        assert!(registry.evaluate(Value::string("a"), Value::Int(1), BinaryOp::Add).is_err());
    }

    #[test]
    fn invalid_operations() {
        let registry = OperatorRegistry::new();

        // Booleans don't support arithmetic
        assert!(registry.evaluate(Value::Bool(true), Value::Bool(false), BinaryOp::Add).is_err());
        // Strings don't support subtraction
        assert!(registry.evaluate(Value::string("a"), Value::string("b"), BinaryOp::Sub).is_err());
    }
}

// =============================================================================
// Option and Result operations
// =============================================================================

mod option_result_ops {
    use super::*;

    #[test]
    fn option_equality() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::some(Value::Int(1)), Value::some(Value::Int(1)), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::None, Value::None, BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::some(Value::Int(1)), Value::None, BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn result_equality() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::ok(Value::Int(1)), Value::ok(Value::Int(1)), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::err(Value::string("e")), Value::err(Value::string("e")), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry.evaluate(Value::ok(Value::Int(1)), Value::err(Value::string("e")), BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
    }
}
