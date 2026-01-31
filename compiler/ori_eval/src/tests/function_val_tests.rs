//! Tests for function value (type conversion) implementations.
//!
//! Relocated from `function_val.rs` per coding guidelines (>200 lines).

use crate::function_val::{
    function_val_byte, function_val_float, function_val_int, function_val_str,
};
use ori_patterns::Value;

mod int_conversion {
    use super::*;

    #[test]
    fn int_from_float_basic() {
        assert_eq!(
            function_val_int(&[Value::Float(3.7)]).unwrap(),
            Value::int(3)
        );
        assert_eq!(
            function_val_int(&[Value::Float(3.2)]).unwrap(),
            Value::int(3)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-3.7)]).unwrap(),
            Value::int(-3)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-3.2)]).unwrap(),
            Value::int(-3)
        );
    }

    #[test]
    fn int_from_float_whole_numbers() {
        assert_eq!(
            function_val_int(&[Value::Float(0.0)]).unwrap(),
            Value::int(0)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-0.0)]).unwrap(),
            Value::int(0)
        );
        assert_eq!(
            function_val_int(&[Value::Float(1.0)]).unwrap(),
            Value::int(1)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-1.0)]).unwrap(),
            Value::int(-1)
        );
        assert_eq!(
            function_val_int(&[Value::Float(42.0)]).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn int_from_float_near_zero() {
        assert_eq!(
            function_val_int(&[Value::Float(0.1)]).unwrap(),
            Value::int(0)
        );
        assert_eq!(
            function_val_int(&[Value::Float(0.9)]).unwrap(),
            Value::int(0)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-0.1)]).unwrap(),
            Value::int(0)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-0.9)]).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn int_from_float_nan_error() {
        let result = function_val_int(&[Value::Float(f64::NAN)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("NaN"));
    }

    #[test]
    fn int_from_float_infinity_error() {
        let result = function_val_int(&[Value::Float(f64::INFINITY)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("infinity"));

        let result = function_val_int(&[Value::Float(f64::NEG_INFINITY)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("infinity"));
    }

    #[test]
    fn int_from_float_overflow_error() {
        let result = function_val_int(&[Value::Float(1e19)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    #[test]
    fn int_from_string_basic() {
        assert_eq!(
            function_val_int(&[Value::string("0")]).unwrap(),
            Value::int(0)
        );
        assert_eq!(
            function_val_int(&[Value::string("1")]).unwrap(),
            Value::int(1)
        );
        assert_eq!(
            function_val_int(&[Value::string("-1")]).unwrap(),
            Value::int(-1)
        );
        assert_eq!(
            function_val_int(&[Value::string("12345")]).unwrap(),
            Value::int(12345)
        );
    }

    #[test]
    fn int_from_string_boundaries() {
        assert_eq!(
            function_val_int(&[Value::string("9223372036854775807")]).unwrap(),
            Value::int(i64::MAX)
        );
        assert_eq!(
            function_val_int(&[Value::string("-9223372036854775808")]).unwrap(),
            Value::int(i64::MIN)
        );
    }

    #[test]
    fn int_from_string_invalid_syntax() {
        assert!(function_val_int(&[Value::string("")]).is_err());
        assert!(function_val_int(&[Value::string("abc")]).is_err());
        assert!(function_val_int(&[Value::string("123abc")]).is_err());
        assert!(function_val_int(&[Value::string("12.34")]).is_err());
    }

    #[test]
    fn int_from_bool() {
        assert_eq!(
            function_val_int(&[Value::Bool(true)]).unwrap(),
            Value::int(1)
        );
        assert_eq!(
            function_val_int(&[Value::Bool(false)]).unwrap(),
            Value::int(0)
        );
    }

    #[test]
    fn int_from_int_identity() {
        assert_eq!(function_val_int(&[Value::int(0)]).unwrap(), Value::int(0));
        assert_eq!(function_val_int(&[Value::int(42)]).unwrap(), Value::int(42));
        assert_eq!(
            function_val_int(&[Value::int(i64::MAX)]).unwrap(),
            Value::int(i64::MAX)
        );
        assert_eq!(
            function_val_int(&[Value::int(i64::MIN)]).unwrap(),
            Value::int(i64::MIN)
        );
    }

    #[test]
    fn int_wrong_arg_count() {
        assert!(function_val_int(&[]).is_err());
        assert!(function_val_int(&[Value::int(1), Value::int(2)]).is_err());
    }

    #[test]
    fn int_from_invalid_type() {
        assert!(function_val_int(&[Value::list(vec![])]).is_err());
        assert!(function_val_int(&[Value::Void]).is_err());
    }
}

mod float_conversion {
    use super::*;

    #[test]
    fn float_from_int_basic() {
        assert_eq!(
            function_val_float(&[Value::int(0)]).unwrap(),
            Value::Float(0.0)
        );
        assert_eq!(
            function_val_float(&[Value::int(1)]).unwrap(),
            Value::Float(1.0)
        );
        assert_eq!(
            function_val_float(&[Value::int(-1)]).unwrap(),
            Value::Float(-1.0)
        );
        assert_eq!(
            function_val_float(&[Value::int(42)]).unwrap(),
            Value::Float(42.0)
        );
    }

    #[test]
    fn float_from_int_i32_boundaries() {
        assert_eq!(
            function_val_float(&[Value::int(i64::from(i32::MAX))]).unwrap(),
            Value::Float(f64::from(i32::MAX))
        );
        assert_eq!(
            function_val_float(&[Value::int(i64::from(i32::MIN))]).unwrap(),
            Value::Float(f64::from(i32::MIN))
        );
    }

    #[test]
    fn float_from_string_basic() {
        assert_eq!(
            function_val_float(&[Value::string("0")]).unwrap(),
            Value::Float(0.0)
        );
        assert_eq!(
            function_val_float(&[Value::string("1")]).unwrap(),
            Value::Float(1.0)
        );
        assert_eq!(
            function_val_float(&[Value::string("1.5")]).unwrap(),
            Value::Float(1.5)
        );
    }

    #[test]
    fn float_from_string_scientific() {
        assert_eq!(
            function_val_float(&[Value::string("1e10")]).unwrap(),
            Value::Float(1e10)
        );
        assert_eq!(
            function_val_float(&[Value::string("1E10")]).unwrap(),
            Value::Float(1e10)
        );
        assert_eq!(
            function_val_float(&[Value::string("1e-10")]).unwrap(),
            Value::Float(1e-10)
        );
    }

    #[test]
    fn float_from_string_invalid_syntax() {
        assert!(function_val_float(&[Value::string("")]).is_err());
        assert!(function_val_float(&[Value::string("abc")]).is_err());
        assert!(function_val_float(&[Value::string("1.2.3")]).is_err());
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "Testing float operations, not using PI"
    )]
    fn float_from_float_identity() {
        assert_eq!(
            function_val_float(&[Value::Float(0.0)]).unwrap(),
            Value::Float(0.0)
        );
        assert_eq!(
            function_val_float(&[Value::Float(3.14)]).unwrap(),
            Value::Float(3.14)
        );
    }

    #[test]
    fn float_wrong_arg_count() {
        assert!(function_val_float(&[]).is_err());
        assert!(function_val_float(&[Value::int(1), Value::int(2)]).is_err());
    }

    #[test]
    fn float_from_invalid_type() {
        assert!(function_val_float(&[Value::Bool(true)]).is_err());
        assert!(function_val_float(&[Value::list(vec![])]).is_err());
    }
}

mod str_conversion {
    use super::*;

    #[test]
    fn str_from_int() {
        assert_eq!(
            function_val_str(&[Value::int(0)]).unwrap(),
            Value::string("0")
        );
        assert_eq!(
            function_val_str(&[Value::int(1)]).unwrap(),
            Value::string("1")
        );
        assert_eq!(
            function_val_str(&[Value::int(-1)]).unwrap(),
            Value::string("-1")
        );
        assert_eq!(
            function_val_str(&[Value::int(42)]).unwrap(),
            Value::string("42")
        );
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "Testing float operations, not using PI"
    )]
    fn str_from_float() {
        assert_eq!(
            function_val_str(&[Value::Float(0.0)]).unwrap(),
            Value::string("0")
        );
        assert_eq!(
            function_val_str(&[Value::Float(3.14)]).unwrap(),
            Value::string("3.14")
        );
    }

    #[test]
    fn str_from_bool() {
        assert_eq!(
            function_val_str(&[Value::Bool(true)]).unwrap(),
            Value::string("true")
        );
        assert_eq!(
            function_val_str(&[Value::Bool(false)]).unwrap(),
            Value::string("false")
        );
    }

    #[test]
    fn str_wrong_arg_count() {
        assert!(function_val_str(&[]).is_err());
        assert!(function_val_str(&[Value::int(1), Value::int(2)]).is_err());
    }
}

mod byte_conversion {
    use super::*;

    #[test]
    fn byte_from_int_valid_range() {
        assert_eq!(function_val_byte(&[Value::int(0)]).unwrap(), Value::Byte(0));
        assert_eq!(function_val_byte(&[Value::int(1)]).unwrap(), Value::Byte(1));
        assert_eq!(
            function_val_byte(&[Value::int(127)]).unwrap(),
            Value::Byte(127)
        );
        assert_eq!(
            function_val_byte(&[Value::int(255)]).unwrap(),
            Value::Byte(255)
        );
    }

    #[test]
    fn byte_from_int_out_of_range() {
        assert!(function_val_byte(&[Value::int(-1)]).is_err());
        assert!(function_val_byte(&[Value::int(256)]).is_err());
        assert!(function_val_byte(&[Value::int(1000)]).is_err());
    }

    #[test]
    fn byte_from_byte_identity() {
        assert_eq!(
            function_val_byte(&[Value::Byte(0)]).unwrap(),
            Value::Byte(0)
        );
        assert_eq!(
            function_val_byte(&[Value::Byte(255)]).unwrap(),
            Value::Byte(255)
        );
    }

    #[test]
    fn byte_from_char_ascii() {
        assert_eq!(
            function_val_byte(&[Value::Char('a')]).unwrap(),
            Value::Byte(97)
        );
        assert_eq!(
            function_val_byte(&[Value::Char('A')]).unwrap(),
            Value::Byte(65)
        );
        assert_eq!(
            function_val_byte(&[Value::Char('0')]).unwrap(),
            Value::Byte(48)
        );
    }

    #[test]
    fn byte_from_char_non_ascii_error() {
        assert!(function_val_byte(&[Value::Char('λ')]).is_err());
        assert!(function_val_byte(&[Value::Char('中')]).is_err());
    }

    #[test]
    fn byte_wrong_arg_count() {
        assert!(function_val_byte(&[]).is_err());
        assert!(function_val_byte(&[Value::int(1), Value::int(2)]).is_err());
    }

    #[test]
    fn byte_from_invalid_type() {
        assert!(function_val_byte(&[Value::Float(1.0)]).is_err());
        assert!(function_val_byte(&[Value::string("1")]).is_err());
        assert!(function_val_byte(&[Value::Bool(true)]).is_err());
    }
}
