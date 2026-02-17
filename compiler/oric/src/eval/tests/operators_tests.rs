//! Tests for binary operators.
//!
//! Tests operator evaluation including arithmetic, comparison, bitwise,
//! and type checking.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::Value;
use crate::ir::BinaryOp;
use ori_eval::evaluate_binary;

// Integer operations

mod int_ops {
    use super::*;

    #[test]
    fn arithmetic() {
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Add).unwrap(),
            Value::int(5)
        );
        assert_eq!(
            evaluate_binary(Value::int(5), Value::int(3), BinaryOp::Sub).unwrap(),
            Value::int(2)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Mul).unwrap(),
            Value::int(6)
        );
        assert_eq!(
            evaluate_binary(Value::int(10), Value::int(3), BinaryOp::Div).unwrap(),
            Value::int(3)
        );
        assert_eq!(
            evaluate_binary(Value::int(10), Value::int(3), BinaryOp::Mod).unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn floor_division() {
        // Positive numbers
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(3), BinaryOp::FloorDiv).unwrap(),
            Value::int(2)
        );
        // Negative dividend - should round toward negative infinity
        assert_eq!(
            evaluate_binary(Value::int(-7), Value::int(3), BinaryOp::FloorDiv).unwrap(),
            Value::int(-3)
        );
        // Negative divisor
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(-3), BinaryOp::FloorDiv).unwrap(),
            Value::int(-3)
        );
        // Both negative
        assert_eq!(
            evaluate_binary(Value::int(-7), Value::int(-3), BinaryOp::FloorDiv).unwrap(),
            Value::int(2)
        );
    }

    #[test]
    fn comparison() {
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(3), Value::int(2), BinaryOp::Gt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(2), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::NotEq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(2), BinaryOp::LtEq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(2), BinaryOp::GtEq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn bitwise() {
        assert_eq!(
            evaluate_binary(Value::int(0b1100), Value::int(0b1010), BinaryOp::BitAnd).unwrap(),
            Value::int(0b1000)
        );
        assert_eq!(
            evaluate_binary(Value::int(0b1100), Value::int(0b1010), BinaryOp::BitOr).unwrap(),
            Value::int(0b1110)
        );
        assert_eq!(
            evaluate_binary(Value::int(0b1100), Value::int(0b1010), BinaryOp::BitXor).unwrap(),
            Value::int(0b0110)
        );
    }

    #[test]
    fn shift() {
        assert_eq!(
            evaluate_binary(Value::int(1), Value::int(4), BinaryOp::Shl).unwrap(),
            Value::int(16)
        );
        assert_eq!(
            evaluate_binary(Value::int(16), Value::int(2), BinaryOp::Shr).unwrap(),
            Value::int(4)
        );
    }

    #[test]
    fn shift_out_of_range() {
        // Shift by negative amount should error
        assert!(evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shl).is_err());
        // Shift by >= 64 should error
        assert!(evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shl).is_err());
        assert!(evaluate_binary(Value::int(1), Value::int(100), BinaryOp::Shr).is_err());
    }

    #[test]
    fn division_by_zero() {
        assert!(evaluate_binary(Value::int(5), Value::int(0), BinaryOp::Div).is_err());
        assert!(evaluate_binary(Value::int(5), Value::int(0), BinaryOp::Mod).is_err());
        assert!(evaluate_binary(Value::int(5), Value::int(0), BinaryOp::FloorDiv).is_err());
    }

    #[test]
    fn range() {
        let result = evaluate_binary(Value::int(1), Value::int(5), BinaryOp::Range).unwrap();
        if let Value::Range(r) = result {
            assert_eq!(r.start, 1);
            assert_eq!(r.end, Some(5));
            assert!(!r.inclusive);
        } else {
            panic!("Expected Range");
        }

        let result =
            evaluate_binary(Value::int(1), Value::int(5), BinaryOp::RangeInclusive).unwrap();
        if let Value::Range(r) = result {
            assert!(r.inclusive);
        } else {
            panic!("Expected Range");
        }
    }
}

// Float operations

mod float_ops {
    use super::*;

    #[test]
    fn arithmetic() {
        assert_eq!(
            evaluate_binary(Value::Float(2.0), Value::Float(3.0), BinaryOp::Add).unwrap(),
            Value::Float(5.0)
        );
        assert_eq!(
            evaluate_binary(Value::Float(5.0), Value::Float(3.0), BinaryOp::Sub).unwrap(),
            Value::Float(2.0)
        );
        assert_eq!(
            evaluate_binary(Value::Float(2.0), Value::Float(3.0), BinaryOp::Mul).unwrap(),
            Value::Float(6.0)
        );
        assert_eq!(
            evaluate_binary(Value::Float(6.0), Value::Float(3.0), BinaryOp::Div).unwrap(),
            Value::Float(2.0)
        );
    }

    #[test]
    fn comparison() {
        assert_eq!(
            evaluate_binary(Value::Float(2.0), Value::Float(3.0), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::Float(2.0), Value::Float(2.0), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn nan_comparison() {
        // NaN != NaN (IEEE 754)
        assert_eq!(
            evaluate_binary(Value::Float(f64::NAN), Value::Float(f64::NAN), BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            evaluate_binary(
                Value::Float(f64::NAN),
                Value::Float(f64::NAN),
                BinaryOp::NotEq
            )
            .unwrap(),
            Value::Bool(true)
        );
        // NaN comparisons are all false
        assert_eq!(
            evaluate_binary(Value::Float(f64::NAN), Value::Float(1.0), BinaryOp::Lt).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            evaluate_binary(Value::Float(f64::NAN), Value::Float(1.0), BinaryOp::Gt).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn negative_zero() {
        // -0.0 == 0.0 (IEEE 754)
        assert_eq!(
            evaluate_binary(Value::Float(-0.0), Value::Float(0.0), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn infinity() {
        assert_eq!(
            evaluate_binary(
                Value::Float(f64::INFINITY),
                Value::Float(f64::INFINITY),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::Float(1.0), Value::Float(f64::INFINITY), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
    }
}

// String operations

mod string_ops {
    use super::*;

    #[test]
    fn concat() {
        assert_eq!(
            evaluate_binary(
                Value::string("hello"),
                Value::string(" world"),
                BinaryOp::Add
            )
            .unwrap(),
            Value::string("hello world")
        );
        assert_eq!(
            evaluate_binary(Value::string(""), Value::string("test"), BinaryOp::Add).unwrap(),
            Value::string("test")
        );
    }

    #[test]
    fn comparison() {
        assert_eq!(
            evaluate_binary(Value::string("abc"), Value::string("abc"), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::string("abc"), Value::string("def"), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
    }
}

// Boolean operations

mod bool_ops {
    use super::*;

    #[test]
    fn equality() {
        assert_eq!(
            evaluate_binary(Value::Bool(true), Value::Bool(true), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::Bool(true), Value::Bool(false), BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            evaluate_binary(Value::Bool(true), Value::Bool(false), BinaryOp::NotEq).unwrap(),
            Value::Bool(true)
        );
    }
}

// List operations

mod list_ops {
    use super::*;

    #[test]
    fn concat() {
        let result = evaluate_binary(
            Value::list(vec![Value::int(1), Value::int(2)]),
            Value::list(vec![Value::int(3), Value::int(4)]),
            BinaryOp::Add,
        )
        .unwrap();

        assert_eq!(
            result,
            Value::list(vec![
                Value::int(1),
                Value::int(2),
                Value::int(3),
                Value::int(4)
            ])
        );
    }

    #[test]
    fn equality() {
        assert_eq!(
            evaluate_binary(
                Value::list(vec![Value::int(1), Value::int(2)]),
                Value::list(vec![Value::int(1), Value::int(2)]),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(
                Value::list(vec![Value::int(1)]),
                Value::list(vec![Value::int(2)]),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(false)
        );
    }
}

// Type errors

mod type_errors {
    use super::*;

    #[test]
    fn mixed_int_float() {
        // Per spec: "No implicit conversions" - int + float is a type error
        assert!(evaluate_binary(Value::int(2), Value::Float(3.0), BinaryOp::Add).is_err());
        assert!(evaluate_binary(Value::Float(2.0), Value::int(3), BinaryOp::Add).is_err());
    }

    #[test]
    fn incompatible_types() {
        assert!(evaluate_binary(Value::int(1), Value::Bool(true), BinaryOp::Add).is_err());
        assert!(evaluate_binary(Value::string("a"), Value::int(1), BinaryOp::Add).is_err());
    }

    #[test]
    fn invalid_operations() {
        // Booleans don't support arithmetic
        assert!(evaluate_binary(Value::Bool(true), Value::Bool(false), BinaryOp::Add).is_err());
        // Strings don't support subtraction
        assert!(evaluate_binary(Value::string("a"), Value::string("b"), BinaryOp::Sub).is_err());
    }
}

// Option and Result operations

mod option_result_ops {
    use super::*;

    #[test]
    fn option_equality() {
        assert_eq!(
            evaluate_binary(
                Value::some(Value::int(1)),
                Value::some(Value::int(1)),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::None, Value::None, BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::some(Value::int(1)), Value::None, BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn result_equality() {
        assert_eq!(
            evaluate_binary(
                Value::ok(Value::int(1)),
                Value::ok(Value::int(1)),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(
                Value::err(Value::string("e")),
                Value::err(Value::string("e")),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(
                Value::ok(Value::int(1)),
                Value::err(Value::string("e")),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(false)
        );
    }
}
