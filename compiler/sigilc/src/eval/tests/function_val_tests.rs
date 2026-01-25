//! Comprehensive tests for type conversion functions (`function_val`).

use sigil_eval::{
    function_val_byte, function_val_float, function_val_int, function_val_str,
};
use crate::eval::Value;

// =============================================================================
// int() conversion tests
// =============================================================================

mod int_conversion {
    use super::*;

    // -------------------------------------------------------------------------
    // From float
    // -------------------------------------------------------------------------

    #[test]
    fn int_from_float_basic() {
        // Basic truncation
        assert_eq!(function_val_int(&[Value::Float(3.7)]).unwrap(), Value::Int(3));
        assert_eq!(function_val_int(&[Value::Float(3.2)]).unwrap(), Value::Int(3));
        assert_eq!(function_val_int(&[Value::Float(-3.7)]).unwrap(), Value::Int(-3));
        assert_eq!(function_val_int(&[Value::Float(-3.2)]).unwrap(), Value::Int(-3));
    }

    #[test]
    fn int_from_float_whole_numbers() {
        assert_eq!(function_val_int(&[Value::Float(0.0)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(-0.0)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(1.0)]).unwrap(), Value::Int(1));
        assert_eq!(function_val_int(&[Value::Float(-1.0)]).unwrap(), Value::Int(-1));
        assert_eq!(function_val_int(&[Value::Float(42.0)]).unwrap(), Value::Int(42));
    }

    #[test]
    fn int_from_float_near_zero() {
        assert_eq!(function_val_int(&[Value::Float(0.1)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(0.9)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(-0.1)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(-0.9)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(0.5)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Float(-0.5)]).unwrap(), Value::Int(0));
    }

    #[test]
    fn int_from_float_large_values() {
        // Values that fit in i64 (within f64 precision)
        assert_eq!(
            function_val_int(&[Value::Float(1e15)]).unwrap(),
            Value::Int(1_000_000_000_000_000)
        );
        assert_eq!(
            function_val_int(&[Value::Float(-1e15)]).unwrap(),
            Value::Int(-1_000_000_000_000_000)
        );
    }

    #[test]
    fn int_from_float_boundary_i32() {
        // i32 boundaries (exact in f64)
        let i32_max = i32::MAX as f64;
        let i32_min = i32::MIN as f64;
        assert_eq!(
            function_val_int(&[Value::Float(i32_max)]).unwrap(),
            Value::Int(i32::MAX as i64)
        );
        assert_eq!(
            function_val_int(&[Value::Float(i32_min)]).unwrap(),
            Value::Int(i32::MIN as i64)
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
        // Values larger than i64::MAX
        let result = function_val_int(&[Value::Float(1e19)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));

        let result = function_val_int(&[Value::Float(-1e19)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    // -------------------------------------------------------------------------
    // From string
    // -------------------------------------------------------------------------

    #[test]
    fn int_from_string_basic() {
        assert_eq!(function_val_int(&[Value::string("0")]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::string("1")]).unwrap(), Value::Int(1));
        assert_eq!(function_val_int(&[Value::string("-1")]).unwrap(), Value::Int(-1));
        assert_eq!(function_val_int(&[Value::string("12345")]).unwrap(), Value::Int(12345));
        assert_eq!(function_val_int(&[Value::string("-12345")]).unwrap(), Value::Int(-12345));
    }

    #[test]
    fn int_from_string_boundaries() {
        // i64 boundaries
        assert_eq!(
            function_val_int(&[Value::string("9223372036854775807")]).unwrap(),
            Value::Int(i64::MAX)
        );
        assert_eq!(
            function_val_int(&[Value::string("-9223372036854775808")]).unwrap(),
            Value::Int(i64::MIN)
        );
    }

    #[test]
    fn int_from_string_overflow_error() {
        // One past i64::MAX
        let result = function_val_int(&[Value::string("9223372036854775808")]);
        assert!(result.is_err());

        // One past i64::MIN
        let result = function_val_int(&[Value::string("-9223372036854775809")]);
        assert!(result.is_err());
    }

    #[test]
    fn int_from_string_invalid_syntax() {
        assert!(function_val_int(&[Value::string("")]).is_err());
        assert!(function_val_int(&[Value::string("abc")]).is_err());
        assert!(function_val_int(&[Value::string("123abc")]).is_err());
        assert!(function_val_int(&[Value::string("12.34")]).is_err());
        assert!(function_val_int(&[Value::string("1e10")]).is_err());
    }

    #[test]
    fn int_from_string_whitespace() {
        // Whitespace handling (should fail - no trimming)
        assert!(function_val_int(&[Value::string(" 123")]).is_err());
        assert!(function_val_int(&[Value::string("123 ")]).is_err());
        assert!(function_val_int(&[Value::string(" 123 ")]).is_err());
    }

    // -------------------------------------------------------------------------
    // From bool
    // -------------------------------------------------------------------------

    #[test]
    fn int_from_bool() {
        assert_eq!(function_val_int(&[Value::Bool(true)]).unwrap(), Value::Int(1));
        assert_eq!(function_val_int(&[Value::Bool(false)]).unwrap(), Value::Int(0));
    }

    // -------------------------------------------------------------------------
    // From int (identity)
    // -------------------------------------------------------------------------

    #[test]
    fn int_from_int_identity() {
        assert_eq!(function_val_int(&[Value::Int(0)]).unwrap(), Value::Int(0));
        assert_eq!(function_val_int(&[Value::Int(42)]).unwrap(), Value::Int(42));
        assert_eq!(function_val_int(&[Value::Int(-42)]).unwrap(), Value::Int(-42));
        assert_eq!(function_val_int(&[Value::Int(i64::MAX)]).unwrap(), Value::Int(i64::MAX));
        assert_eq!(function_val_int(&[Value::Int(i64::MIN)]).unwrap(), Value::Int(i64::MIN));
    }

    // -------------------------------------------------------------------------
    // Argument validation
    // -------------------------------------------------------------------------

    #[test]
    fn int_wrong_arg_count() {
        assert!(function_val_int(&[]).is_err());
        assert!(function_val_int(&[Value::Int(1), Value::Int(2)]).is_err());
    }

    #[test]
    fn int_from_invalid_type() {
        assert!(function_val_int(&[Value::list(vec![])]).is_err());
        assert!(function_val_int(&[Value::Void]).is_err());
    }
}

// =============================================================================
// float() conversion tests
// =============================================================================

mod float_conversion {
    use super::*;

    // -------------------------------------------------------------------------
    // From int
    // -------------------------------------------------------------------------

    #[test]
    fn float_from_int_basic() {
        assert_eq!(function_val_float(&[Value::Int(0)]).unwrap(), Value::Float(0.0));
        assert_eq!(function_val_float(&[Value::Int(1)]).unwrap(), Value::Float(1.0));
        assert_eq!(function_val_float(&[Value::Int(-1)]).unwrap(), Value::Float(-1.0));
        assert_eq!(function_val_float(&[Value::Int(42)]).unwrap(), Value::Float(42.0));
        assert_eq!(function_val_float(&[Value::Int(-42)]).unwrap(), Value::Float(-42.0));
    }

    #[test]
    fn float_from_int_exact_range() {
        // Values that convert exactly to f64 (within 2^53)
        assert_eq!(
            function_val_float(&[Value::Int(9007199254740992)]).unwrap(), // 2^53
            Value::Float(9007199254740992.0)
        );
        assert_eq!(
            function_val_float(&[Value::Int(-9007199254740992)]).unwrap(),
            Value::Float(-9007199254740992.0)
        );
    }

    #[test]
    fn float_from_int_i32_boundaries() {
        assert_eq!(
            function_val_float(&[Value::Int(i32::MAX as i64)]).unwrap(),
            Value::Float(i32::MAX as f64)
        );
        assert_eq!(
            function_val_float(&[Value::Int(i32::MIN as i64)]).unwrap(),
            Value::Float(i32::MIN as f64)
        );
    }

    #[test]
    fn float_from_int_large_values() {
        // Large values may lose precision but should still convert
        let large = i64::MAX;
        let result = function_val_float(&[Value::Int(large)]).unwrap();
        if let Value::Float(f) = result {
            // The value should be close to i64::MAX (precision loss expected)
            assert!(f > 9e18);
        } else {
            panic!("Expected Float");
        }
    }

    // -------------------------------------------------------------------------
    // From string
    // -------------------------------------------------------------------------

    #[test]
    fn float_from_string_basic() {
        assert_eq!(function_val_float(&[Value::string("0")]).unwrap(), Value::Float(0.0));
        assert_eq!(function_val_float(&[Value::string("1")]).unwrap(), Value::Float(1.0));
        assert_eq!(function_val_float(&[Value::string("-1")]).unwrap(), Value::Float(-1.0));
        assert_eq!(function_val_float(&[Value::string("1.5")]).unwrap(), Value::Float(1.5));
        assert_eq!(function_val_float(&[Value::string("-1.5")]).unwrap(), Value::Float(-1.5));
    }

    #[test]
    fn float_from_string_scientific() {
        assert_eq!(function_val_float(&[Value::string("1e10")]).unwrap(), Value::Float(1e10));
        assert_eq!(function_val_float(&[Value::string("1E10")]).unwrap(), Value::Float(1e10));
        assert_eq!(function_val_float(&[Value::string("1e-10")]).unwrap(), Value::Float(1e-10));
        assert_eq!(function_val_float(&[Value::string("1.5e10")]).unwrap(), Value::Float(1.5e10));
        assert_eq!(function_val_float(&[Value::string("-1.5e-10")]).unwrap(), Value::Float(-1.5e-10));
    }

    #[test]
    fn float_from_string_special_values() {
        // Infinity
        if let Value::Float(f) = function_val_float(&[Value::string("inf")]).unwrap() {
            assert!(f.is_infinite() && f.is_sign_positive());
        }
        if let Value::Float(f) = function_val_float(&[Value::string("-inf")]).unwrap() {
            assert!(f.is_infinite() && f.is_sign_negative());
        }
        if let Value::Float(f) = function_val_float(&[Value::string("Infinity")]).unwrap() {
            assert!(f.is_infinite());
        }

        // NaN
        if let Value::Float(f) = function_val_float(&[Value::string("NaN")]).unwrap() {
            assert!(f.is_nan());
        }
    }

    #[test]
    fn float_from_string_zero_variants() {
        assert_eq!(function_val_float(&[Value::string("0")]).unwrap(), Value::Float(0.0));
        assert_eq!(function_val_float(&[Value::string("0.0")]).unwrap(), Value::Float(0.0));
        assert_eq!(function_val_float(&[Value::string("-0")]).unwrap(), Value::Float(-0.0));
        assert_eq!(function_val_float(&[Value::string("0e0")]).unwrap(), Value::Float(0.0));
    }

    #[test]
    fn float_from_string_max_values() {
        // Maximum finite f64
        let max_str = "1.7976931348623157e308";
        if let Value::Float(f) = function_val_float(&[Value::string(max_str)]).unwrap() {
            assert!(f.is_finite());
            assert!(f > 1e308);
        }
    }

    #[test]
    fn float_from_string_min_denormal() {
        // Smallest positive denormal
        let min_str = "5e-324";
        if let Value::Float(f) = function_val_float(&[Value::string(min_str)]).unwrap() {
            assert!(f > 0.0);
            assert!(f < 1e-300);
        }
    }

    #[test]
    fn float_from_string_invalid_syntax() {
        assert!(function_val_float(&[Value::string("")]).is_err());
        assert!(function_val_float(&[Value::string("abc")]).is_err());
        assert!(function_val_float(&[Value::string("1.2.3")]).is_err());
        assert!(function_val_float(&[Value::string("1e")]).is_err());
    }

    // -------------------------------------------------------------------------
    // From float (identity)
    // -------------------------------------------------------------------------

    #[test]
    fn float_from_float_identity() {
        assert_eq!(function_val_float(&[Value::Float(0.0)]).unwrap(), Value::Float(0.0));
        assert_eq!(function_val_float(&[Value::Float(3.14)]).unwrap(), Value::Float(3.14));
        assert_eq!(function_val_float(&[Value::Float(-3.14)]).unwrap(), Value::Float(-3.14));

        // Special values
        if let Value::Float(f) = function_val_float(&[Value::Float(f64::INFINITY)]).unwrap() {
            assert!(f.is_infinite());
        }
        if let Value::Float(f) = function_val_float(&[Value::Float(f64::NAN)]).unwrap() {
            assert!(f.is_nan());
        }
    }

    // -------------------------------------------------------------------------
    // Argument validation
    // -------------------------------------------------------------------------

    #[test]
    fn float_wrong_arg_count() {
        assert!(function_val_float(&[]).is_err());
        assert!(function_val_float(&[Value::Int(1), Value::Int(2)]).is_err());
    }

    #[test]
    fn float_from_invalid_type() {
        assert!(function_val_float(&[Value::Bool(true)]).is_err());
        assert!(function_val_float(&[Value::list(vec![])]).is_err());
    }
}

// =============================================================================
// str() conversion tests
// =============================================================================

mod str_conversion {
    use super::*;

    #[test]
    fn str_from_int() {
        assert_eq!(function_val_str(&[Value::Int(0)]).unwrap(), Value::string("0"));
        assert_eq!(function_val_str(&[Value::Int(1)]).unwrap(), Value::string("1"));
        assert_eq!(function_val_str(&[Value::Int(-1)]).unwrap(), Value::string("-1"));
        assert_eq!(function_val_str(&[Value::Int(42)]).unwrap(), Value::string("42"));
        assert_eq!(function_val_str(&[Value::Int(i64::MAX)]).unwrap(), Value::string("9223372036854775807"));
        assert_eq!(function_val_str(&[Value::Int(i64::MIN)]).unwrap(), Value::string("-9223372036854775808"));
    }

    #[test]
    fn str_from_float() {
        assert_eq!(function_val_str(&[Value::Float(0.0)]).unwrap(), Value::string("0"));
        assert_eq!(function_val_str(&[Value::Float(3.14)]).unwrap(), Value::string("3.14"));
        assert_eq!(function_val_str(&[Value::Float(-3.14)]).unwrap(), Value::string("-3.14"));
    }

    #[test]
    fn str_from_float_special() {
        if let Value::Str(s) = function_val_str(&[Value::Float(f64::INFINITY)]).unwrap() {
            assert!(s.contains("inf") || s.contains("Inf"));
        }
        if let Value::Str(s) = function_val_str(&[Value::Float(f64::NAN)]).unwrap() {
            assert!(s.contains("NaN") || s.contains("nan"));
        }
    }

    #[test]
    fn str_from_bool() {
        assert_eq!(function_val_str(&[Value::Bool(true)]).unwrap(), Value::string("true"));
        assert_eq!(function_val_str(&[Value::Bool(false)]).unwrap(), Value::string("false"));
    }

    #[test]
    fn str_from_string() {
        // str() uses Display format, which quotes strings
        assert_eq!(function_val_str(&[Value::string("")]).unwrap(), Value::string("\"\""));
        assert_eq!(function_val_str(&[Value::string("hello")]).unwrap(), Value::string("\"hello\""));
    }

    #[test]
    fn str_wrong_arg_count() {
        assert!(function_val_str(&[]).is_err());
        assert!(function_val_str(&[Value::Int(1), Value::Int(2)]).is_err());
    }
}

// =============================================================================
// byte() conversion tests
// =============================================================================

mod byte_conversion {
    use super::*;

    #[test]
    fn byte_from_int_valid_range() {
        assert_eq!(function_val_byte(&[Value::Int(0)]).unwrap(), Value::Byte(0));
        assert_eq!(function_val_byte(&[Value::Int(1)]).unwrap(), Value::Byte(1));
        assert_eq!(function_val_byte(&[Value::Int(127)]).unwrap(), Value::Byte(127));
        assert_eq!(function_val_byte(&[Value::Int(128)]).unwrap(), Value::Byte(128));
        assert_eq!(function_val_byte(&[Value::Int(255)]).unwrap(), Value::Byte(255));
    }

    #[test]
    fn byte_from_int_out_of_range() {
        // Negative values
        assert!(function_val_byte(&[Value::Int(-1)]).is_err());
        assert!(function_val_byte(&[Value::Int(-128)]).is_err());

        // Values > 255
        assert!(function_val_byte(&[Value::Int(256)]).is_err());
        assert!(function_val_byte(&[Value::Int(1000)]).is_err());
    }

    #[test]
    fn byte_from_byte_identity() {
        assert_eq!(function_val_byte(&[Value::Byte(0)]).unwrap(), Value::Byte(0));
        assert_eq!(function_val_byte(&[Value::Byte(255)]).unwrap(), Value::Byte(255));
    }

    #[test]
    fn byte_from_char_ascii() {
        assert_eq!(function_val_byte(&[Value::Char('a')]).unwrap(), Value::Byte(97));
        assert_eq!(function_val_byte(&[Value::Char('A')]).unwrap(), Value::Byte(65));
        assert_eq!(function_val_byte(&[Value::Char('0')]).unwrap(), Value::Byte(48));
        assert_eq!(function_val_byte(&[Value::Char('\n')]).unwrap(), Value::Byte(10));
        assert_eq!(function_val_byte(&[Value::Char('\0')]).unwrap(), Value::Byte(0));
    }

    #[test]
    fn byte_from_char_non_ascii_error() {
        // Non-ASCII characters should fail
        assert!(function_val_byte(&[Value::Char('λ')]).is_err());
        assert!(function_val_byte(&[Value::Char('中')]).is_err());
        assert!(function_val_byte(&[Value::Char('🎉')]).is_err());
    }

    #[test]
    fn byte_wrong_arg_count() {
        assert!(function_val_byte(&[]).is_err());
        assert!(function_val_byte(&[Value::Int(1), Value::Int(2)]).is_err());
    }

    #[test]
    fn byte_from_invalid_type() {
        assert!(function_val_byte(&[Value::Float(1.0)]).is_err());
        assert!(function_val_byte(&[Value::string("1")]).is_err());
        assert!(function_val_byte(&[Value::Bool(true)]).is_err());
    }
}

// =============================================================================
// Edge cases and stress tests
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn int_from_float_exactly_representable() {
        // All integers up to 2^53 are exactly representable in f64
        let exact_max = (1_i64 << 53) as f64;
        assert_eq!(
            function_val_int(&[Value::Float(exact_max)]).unwrap(),
            Value::Int(1_i64 << 53)
        );
    }

    #[test]
    fn float_from_int_round_trip_small() {
        // Small integers should round-trip exactly
        for i in -1000..=1000 {
            let as_float = function_val_float(&[Value::Int(i)]).unwrap();
            if let Value::Float(f) = as_float {
                let back = function_val_int(&[Value::Float(f)]).unwrap();
                assert_eq!(back, Value::Int(i), "Round-trip failed for {i}");
            }
        }
    }

    #[test]
    fn str_int_round_trip() {
        // String -> Int -> String should preserve value
        for &s in &["0", "1", "-1", "12345", "-12345", "9223372036854775807", "-9223372036854775808"] {
            let as_int = function_val_int(&[Value::string(s)]).unwrap();
            let back = function_val_str(&[as_int]).unwrap();
            assert_eq!(back, Value::string(s), "Round-trip failed for {s}");
        }
    }

    #[test]
    fn str_float_round_trip_simple() {
        // String -> Float -> String for simple values
        for &s in &["0", "1", "1.5", "-1.5", "100", "0.125"] {
            let as_float = function_val_float(&[Value::string(s)]).unwrap();
            if let Value::Float(f) = as_float {
                // Check the parsed value is correct
                let expected: f64 = s.parse().unwrap();
                assert!((f - expected).abs() < 1e-10, "Parse failed for {s}");
            }
        }
    }
}
