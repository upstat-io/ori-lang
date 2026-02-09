//! Tests for binary operator implementations.
//!
//! Relocated from `operators.rs` per coding guidelines (>200 lines).

use crate::operators::evaluate_binary;
use ori_ir::BinaryOp;
use ori_patterns::Value;

#[test]
fn test_int_operations() {
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
        evaluate_binary(Value::int(7), Value::int(2), BinaryOp::Div).unwrap(),
        Value::int(3)
    );
    assert_eq!(
        evaluate_binary(Value::int(7), Value::int(2), BinaryOp::Mod).unwrap(),
        Value::int(1)
    );
}

#[test]
fn test_division_by_zero() {
    assert!(evaluate_binary(Value::int(1), Value::int(0), BinaryOp::Div).is_err());
    assert!(evaluate_binary(Value::int(1), Value::int(0), BinaryOp::Mod).is_err());
}

#[test]
fn test_comparisons() {
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
}

#[test]
fn test_string_concatenation() {
    let result = evaluate_binary(
        Value::string("hello".to_string()),
        Value::string(" world".to_string()),
        BinaryOp::Add,
    )
    .unwrap();
    assert_eq!(result, Value::string("hello world".to_string()));
}

#[test]
fn test_list_concatenation() {
    let result = evaluate_binary(
        Value::list(vec![Value::int(1)]),
        Value::list(vec![Value::int(2)]),
        BinaryOp::Add,
    )
    .unwrap();
    assert_eq!(result, Value::list(vec![Value::int(1), Value::int(2)]));
}

#[test]
fn test_type_mismatch() {
    assert!(evaluate_binary(Value::int(1), Value::Bool(true), BinaryOp::Add).is_err());
}

#[test]
fn test_shift_amount_validation() {
    // Valid shift
    assert_eq!(
        evaluate_binary(Value::int(1), Value::int(3), BinaryOp::Shl).unwrap(),
        Value::int(8)
    );

    // Invalid shift (negative)
    assert!(evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shl).is_err());

    // Invalid shift (too large)
    assert!(evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shl).is_err());
}

#[test]
fn test_addition_overflow() {
    let result = evaluate_binary(Value::int(i64::MAX), Value::int(1), BinaryOp::Add);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("integer overflow"));
}

#[test]
fn test_subtraction_overflow() {
    let result = evaluate_binary(Value::int(i64::MIN), Value::int(1), BinaryOp::Sub);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("integer overflow"));
}

#[test]
fn test_multiplication_overflow() {
    let result = evaluate_binary(Value::int(i64::MAX), Value::int(2), BinaryOp::Mul);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("integer overflow"));
}

#[test]
fn test_division_overflow() {
    // i64::MIN / -1 overflows because |i64::MIN| > i64::MAX
    let result = evaluate_binary(Value::int(i64::MIN), Value::int(-1), BinaryOp::Div);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("integer overflow"));
}

#[test]
fn test_option_equality() {
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
        evaluate_binary(Value::some(Value::int(1)), Value::None, BinaryOp::Eq).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        evaluate_binary(Value::None, Value::None, BinaryOp::Eq).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn test_result_equality() {
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
            Value::ok(Value::int(1)),
            Value::err(Value::string("e")),
            BinaryOp::Eq
        )
        .unwrap(),
        Value::Bool(false)
    );
}

fn assert_overflow(left: i64, right: i64, op: BinaryOp) {
    let result = evaluate_binary(Value::int(left), Value::int(right), op);
    assert!(
        result.is_err(),
        "expected overflow for {left} {op:?} {right}"
    );
    assert!(
        result
            .unwrap_err()
            .into_eval_error()
            .message
            .contains("integer overflow"),
        "expected 'integer overflow' message"
    );
}

#[test]
fn overflow_add_max_plus_1() {
    assert_overflow(i64::MAX, 1, BinaryOp::Add);
}

#[test]
fn overflow_add_max_plus_max() {
    assert_overflow(i64::MAX, i64::MAX, BinaryOp::Add);
}

#[test]
fn overflow_add_min_plus_neg1() {
    assert_overflow(i64::MIN, -1, BinaryOp::Add);
}

#[test]
fn overflow_sub_min_minus_1() {
    assert_overflow(i64::MIN, 1, BinaryOp::Sub);
}

#[test]
fn overflow_sub_max_minus_neg1() {
    assert_overflow(i64::MAX, -1, BinaryOp::Sub);
}

#[test]
fn overflow_mul_max_times_2() {
    assert_overflow(i64::MAX, 2, BinaryOp::Mul);
}

#[test]
fn overflow_mul_min_times_neg1() {
    assert_overflow(i64::MIN, -1, BinaryOp::Mul);
}

#[test]
fn overflow_mul_min_times_2() {
    assert_overflow(i64::MIN, 2, BinaryOp::Mul);
}

#[test]
fn overflow_div_min_by_neg1() {
    assert_overflow(i64::MIN, -1, BinaryOp::Div);
}

#[test]
fn overflow_floor_div_min_by_neg1() {
    assert_overflow(i64::MIN, -1, BinaryOp::FloorDiv);
}

#[test]
fn overflow_rem_min_mod_neg1() {
    let result = evaluate_binary(Value::int(i64::MIN), Value::int(-1), BinaryOp::Mod);
    assert!(result.is_err());
}

#[test]
fn div_zero_message() {
    let result = evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Div);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("division by zero"));
}

#[test]
fn mod_zero_message() {
    let result = evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Mod);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("modulo by zero"));
}

#[test]
fn floor_div_zero_message() {
    let result = evaluate_binary(Value::int(42), Value::int(0), BinaryOp::FloorDiv);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("division by zero"));
}

#[test]
fn near_boundary_add_valid() {
    // MAX - 1 + 1 = MAX
    assert_eq!(
        evaluate_binary(Value::int(i64::MAX - 1), Value::int(1), BinaryOp::Add).unwrap(),
        Value::int(i64::MAX)
    );
}

#[test]
fn near_boundary_add_overflow() {
    // MAX - 1 + 2 overflows
    assert_overflow(i64::MAX - 1, 2, BinaryOp::Add);
}

#[test]
fn near_boundary_sub_valid() {
    // MIN + 1 - 1 = MIN
    assert_eq!(
        evaluate_binary(Value::int(i64::MIN + 1), Value::int(1), BinaryOp::Sub).unwrap(),
        Value::int(i64::MIN)
    );
}

#[test]
fn near_boundary_sub_overflow() {
    // MIN + 1 - 2 overflows
    assert_overflow(i64::MIN + 1, 2, BinaryOp::Sub);
}

#[test]
fn near_boundary_mul_valid() {
    // (MAX / 2) * 2 should not overflow
    let half = i64::MAX / 2;
    assert_eq!(
        evaluate_binary(Value::int(half), Value::int(2), BinaryOp::Mul).unwrap(),
        Value::int(half * 2)
    );
}

#[test]
fn floor_div_positive() {
    assert_eq!(
        evaluate_binary(Value::int(7), Value::int(2), BinaryOp::FloorDiv).unwrap(),
        Value::int(3)
    );
}

#[test]
fn floor_div_negative_numerator() {
    assert_eq!(
        evaluate_binary(Value::int(-7), Value::int(2), BinaryOp::FloorDiv).unwrap(),
        Value::int(-4)
    );
}

#[test]
fn floor_div_negative_denominator() {
    assert_eq!(
        evaluate_binary(Value::int(7), Value::int(-2), BinaryOp::FloorDiv).unwrap(),
        Value::int(-4)
    );
}

#[test]
fn floor_div_both_negative() {
    assert_eq!(
        evaluate_binary(Value::int(-7), Value::int(-2), BinaryOp::FloorDiv).unwrap(),
        Value::int(3)
    );
}

#[test]
fn rem_negative_numerator() {
    // -7 % 3 = -1 (sign follows numerator)
    assert_eq!(
        evaluate_binary(Value::int(-7), Value::int(3), BinaryOp::Mod).unwrap(),
        Value::int(-1)
    );
}

#[test]
fn rem_negative_denominator() {
    // 7 % -3 = 1 (sign follows numerator)
    assert_eq!(
        evaluate_binary(Value::int(7), Value::int(-3), BinaryOp::Mod).unwrap(),
        Value::int(1)
    );
}

#[test]
fn rem_both_negative() {
    // -7 % -3 = -1
    assert_eq!(
        evaluate_binary(Value::int(-7), Value::int(-3), BinaryOp::Mod).unwrap(),
        Value::int(-1)
    );
}

#[test]
fn shl_boundary_63_overflows() {
    // Per spec: `1 << 63` should panic due to signed overflow.
    // Shifting 1 left by 63 positions would set only the sign bit,
    // which is an overflow from positive to negative.
    let result = evaluate_binary(Value::int(1), Value::int(63), BinaryOp::Shl);
    assert!(result.is_err());
}

#[test]
fn shr_boundary_63() {
    assert_eq!(
        evaluate_binary(Value::int(i64::MIN), Value::int(63), BinaryOp::Shr).unwrap(),
        Value::int(-1)
    );
}

#[test]
fn shl_zero_shift() {
    assert_eq!(
        evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Shl).unwrap(),
        Value::int(42)
    );
}

#[test]
fn shr_zero_shift() {
    assert_eq!(
        evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Shr).unwrap(),
        Value::int(42)
    );
}

#[test]
fn shl_64_out_of_range() {
    let result = evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shl);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("out of range"));
}

#[test]
fn shr_64_out_of_range() {
    let result = evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shr);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("out of range"));
}

#[test]
fn shl_negative_out_of_range() {
    let result = evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shl);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("out of range"));
}

#[test]
fn shr_negative_out_of_range() {
    let result = evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shr);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("out of range"));
}
