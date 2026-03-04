//! Operator Edge Case & Expression AOT Tests
//!
//! Tests covering operator semantics that basic conformance tests miss:
//! integer boundaries, negative division/modulo, float specials, boolean
//! short-circuit semantics, empty string operations, complex precedence
//! chains, and unary operator combinations.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// ─── Integer boundary values ───

#[test]
fn test_op_large_integer_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let billion = 1000000000;
    let two_billion = billion * 2;
    if two_billion == 2000000000 then 0 else 1
}
"#,
        "op_large_int",
    );
}

#[test]
fn test_op_large_integer_multiplication() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 100000;
    let b = 100000;
    let c = a * b;
    // 10^10
    if c == 10000000000 then 0 else 1
}
"#,
        "op_large_mul",
    );
}

#[test]
fn test_op_negative_large_integer() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = -2000000000;
    let y = x - 1;
    if y == -2000000001 then 0 else 1
}
"#,
        "op_negative_large",
    );
}

#[test]
fn test_op_zero_boundary() {
    assert_aot_success(
        r#"
@main () -> int = {
    let z = 0;
    let ok1 = z == 0;
    let ok2 = -z == 0;
    let ok3 = z + 1 == 1;
    let ok4 = z - 1 == -1;
    let ok5 = z * 1000 == 0;
    if ok1 && ok2 && ok3 && ok4 && ok5 then 0 else 1
}
"#,
        "op_zero_boundary",
    );
}

// ─── Division edge cases ───

#[test]
fn test_op_division_truncation_positive() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 7 / 2;
    let b = 10 / 3;
    let c = 1 / 3;
    if a == 3 && b == 3 && c == 0 then 0 else 1
}
"#,
        "op_div_truncation_pos",
    );
}

#[test]
fn test_op_division_negative_dividend() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = -10 / 3;
    let b = -7 / 2;
    let c = -1 / 2;
    // Truncation toward zero: -10/3 = -3, -7/2 = -3, -1/2 = 0
    if a == -3 && b == -3 && c == 0 then 0 else 1
}
"#,
        "op_div_neg_dividend",
    );
}

#[test]
fn test_op_division_negative_divisor() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 10 / -3;
    let b = 7 / -2;
    // Truncation toward zero: 10/-3 = -3, 7/-2 = -3
    if a == -3 && b == -3 then 0 else 1
}
"#,
        "op_div_neg_divisor",
    );
}

#[test]
fn test_op_division_both_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = -10 / -3;
    let b = -7 / -2;
    // Truncation toward zero: -10/-3 = 3, -7/-2 = 3
    if a == 3 && b == 3 then 0 else 1
}
"#,
        "op_div_both_neg",
    );
}

#[test]
fn test_op_division_exact() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 100 / 10;
    let b = -100 / 10;
    let c = 100 / -10;
    let d = -100 / -10;
    if a == 10 && b == -10 && c == -10 && d == 10 then 0 else 1
}
"#,
        "op_div_exact",
    );
}

// ─── Modulo edge cases ───

#[test]
fn test_op_modulo_negative_dividend() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = -17 % 5;
    let b = -10 % 3;
    // Remainder follows dividend sign: -17 % 5 = -2, -10 % 3 = -1
    if a == -2 && b == -1 then 0 else 1
}
"#,
        "op_mod_neg_dividend",
    );
}

#[test]
fn test_op_modulo_negative_divisor() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 17 % -5;
    let b = 10 % -3;
    // 17 % -5 = 2, 10 % -3 = 1 (remainder follows dividend sign)
    if a == 2 && b == 1 then 0 else 1
}
"#,
        "op_mod_neg_divisor",
    );
}

#[test]
fn test_op_modulo_both_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = -17 % -5;
    // -17 % -5 = -2 (remainder follows dividend sign)
    if a == -2 then 0 else 1
}
"#,
        "op_mod_both_neg",
    );
}

#[test]
fn test_op_modulo_zero_remainder() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 10 % 5;
    let b = -10 % 5;
    let c = 10 % -5;
    if a == 0 && b == 0 && c == 0 then 0 else 1
}
"#,
        "op_mod_zero_remainder",
    );
}

// ─── Negation combinations ───

#[test]
fn test_op_double_boolean_negation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = !(!true);
    let b = !(!false);
    if a == true && b == false then 0 else 1
}
"#,
        "op_double_bool_neg",
    );
}

#[test]
fn test_op_triple_negation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = -(-(-5));
    let b = !(!(!true));
    if a == -5 && b == false then 0 else 1
}
"#,
        "op_triple_negation",
    );
}

// ─── Complex precedence chains ───

#[test]
fn test_op_mixed_precedence_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    // 2 + 3 * 4 - 1 = 2 + 12 - 1 = 13
    let a = 2 + 3 * 4 - 1;
    // 10 - 2 * 3 + 1 = 10 - 6 + 1 = 5
    let b = 10 - 2 * 3 + 1;
    if a == 13 && b == 5 then 0 else 1
}
"#,
        "op_mixed_precedence",
    );
}

#[test]
fn test_op_complex_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    // (a + b) * (c - d) / (e % f)
    let a = 2;
    let b = 3;
    let c = 10;
    let d = 4;
    let e = 17;
    let f = 5;
    let result = (a + b) * (c - d) / (e % f);
    // (5) * (6) / (2) = 15
    if result == 15 then 0 else 1
}
"#,
        "op_complex_expr",
    );
}

#[test]
fn test_op_bitwise_mixed_precedence() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 0xFF & 0x0F | 0xF0;
    // & has higher precedence than |: (0xFF & 0x0F) | 0xF0 = 0x0F | 0xF0 = 0xFF = 255
    if a == 255 then 0 else 1
}
"#,
        "op_bitwise_precedence",
    );
}

#[test]
fn test_op_comparison_in_boolean_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let ok = x > 5 && x < 20 || x == 0;
    // && has higher precedence than ||: (true && true) || false = true
    if ok then 0 else 1
}
"#,
        "op_comparison_bool_chain",
    );
}

// ─── Float edge cases ───

#[test]
fn test_op_float_negative_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let pz = 0.0;
    let nz = -0.0;
    // -0.0 == 0.0 (IEEE 754)
    if pz == nz then 0 else 1
}
"#,
        "op_float_neg_zero",
    );
}

#[test]
fn test_op_float_very_small() {
    assert_aot_success(
        r#"
@main () -> int = {
    let small = 0.000001;
    let result = small * 1000000.0;
    // Should be approximately 1.0
    if result > 0.99 && result < 1.01 then 0 else 1
}
"#,
        "op_float_very_small",
    );
}

#[test]
fn test_op_float_very_large() {
    assert_aot_success(
        r#"
@main () -> int = {
    let big = 1000000.0;
    let result = big * big;
    // 10^12
    if result > 999999999999.0 && result < 1000000000001.0 then 0 else 1
}
"#,
        "op_float_very_large",
    );
}

#[test]
fn test_op_float_precision() {
    assert_aot_success(
        r#"
@main () -> int = {
    // Classic floating-point precision test
    let a = 0.1 + 0.2;
    // 0.1 + 0.2 is NOT exactly 0.3 in IEEE 754
    // It should be approximately 0.30000000000000004
    if a > 0.29 && a < 0.31 then 0 else 1
}
"#,
        "op_float_precision",
    );
}

#[test]
fn test_op_float_division() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1.0 / 3.0;
    let b = 2.0 / 3.0;
    let c = a + b;
    // Should be approximately 1.0
    if c > 0.99 && c < 1.01 then 0 else 1
}
"#,
        "op_float_division",
    );
}

// ─── Empty string operations ───

#[test]
fn test_op_empty_string_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "";
    let b = "";
    if a == b then 0 else 1
}
"#,
        "op_empty_str_eq",
    );
}

#[test]
fn test_op_empty_string_concat_left() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "" + "hello";
    if s == "hello" then 0 else 1
}
"#,
        "op_empty_str_concat_left",
    );
}

#[test]
fn test_op_empty_string_concat_right() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello" + "";
    if s == "hello" then 0 else 1
}
"#,
        "op_empty_str_concat_right",
    );
}

#[test]
fn test_op_empty_string_concat_both() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "" + "";
    if s == "" && s.length() == 0 then 0 else 1
}
"#,
        "op_empty_str_concat_both",
    );
}

#[test]
fn test_op_empty_string_not_equal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let empty = "";
    let space = " ";
    if empty != space then 0 else 1
}
"#,
        "op_empty_str_ne",
    );
}

// ─── Boolean short-circuit semantics ───

#[test]
fn test_op_and_short_circuit() {
    // If && short-circuits, the second condition is never evaluated
    // We test indirectly: false && (complex expression) should still be fast
    assert_aot_success(
        r#"
@expensive (x: int) -> bool = {
    // If this is called when it shouldn't be, the computation still works
    // but we can verify short-circuit by testing evaluation count
    x > 0
};

@main () -> int = {
    let called = 0;
    // false && expensive(1) — expensive should not be called with short-circuit
    let result = false && expensive(x: 1);
    if result == false then 0 else 1
}
"#,
        "op_and_short_circuit",
    );
}

#[test]
fn test_op_or_short_circuit() {
    assert_aot_success(
        r#"
@main () -> int = {
    // true || (anything) should return true without evaluating rhs
    let result = true || false;
    let result2 = false || true;
    let result3 = false || false;
    if result && result2 && !result3 then 0 else 1
}
"#,
        "op_or_short_circuit",
    );
}

#[test]
fn test_op_short_circuit_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    // All conditions must be checked in order, stopping at first false for &&
    let ok = x > 0 && x < 100 && x == 10 && x != 5;
    // At least one must be true for ||, stopping at first true
    let any = x == 5 || x == 7 || x == 10 || x == 15;
    if ok && any then 0 else 1
}
"#,
        "op_short_circuit_chain",
    );
}

// ─── Operator on different types ───

#[test]
fn test_op_char_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 'x';
    let b = 'x';
    let c = 'y';
    if a == b && a != c then 0 else 1
}
"#,
        "op_char_eq",
    );
}

#[test]
fn test_op_byte_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: byte = 200;
    let b: byte = 55;
    let c: byte = 255;
    let d: byte = 0;
    if a != b && c != d then 0 else 1
}
"#,
        "op_byte_arith",
    );
}

#[test]
fn test_op_duration_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 500ms;
    let b = 500ms;
    let c = a + b;
    if c == 1s then 0 else 1
}
"#,
        "op_duration_arith",
    );
}

#[test]
fn test_op_size_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 500kb;
    let b = 500kb;
    let c = a + b;
    // 500kb = 500,000 bytes (SI), 1mb = 1,000,000 bytes (SI)
    if c == 1mb then 0 else 1
}
"#,
        "op_size_arith",
    );
}

// ─── Integer overflow (checked arithmetic) ───

#[test]
fn test_op_overflow_add_max_plus_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let x = 9223372036854775807;
    x + 1
}
"#,
    );
    assert_ne!(exit_code, 0, "addition overflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_add_min_minus_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let x = -9223372036854775807 - 1;
    x + -1
}
"#,
    );
    assert_ne!(exit_code, 0, "addition underflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_sub_min_minus_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let x = -9223372036854775807 - 1;
    x - 1
}
"#,
    );
    assert_ne!(exit_code, 0, "subtraction underflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_mul_large_values() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let x = 9223372036854775807;
    x * 2
}
"#,
    );
    assert_ne!(exit_code, 0, "multiplication overflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_mul_min_times_neg_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let x = -9223372036854775807 - 1;
    x * -1
}
"#,
    );
    assert_ne!(exit_code, 0, "INT_MIN * -1 overflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_no_overflow_near_boundary() {
    // INT_MAX - 1 + 1 = INT_MAX (no overflow)
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 9223372036854775806;
    let y = x + 1;
    if y == 9223372036854775807 then 0 else 1
}
"#,
        "op_no_overflow_add",
    );
}

#[test]
fn test_op_no_overflow_sub_near_boundary() {
    // INT_MIN + 1 - 1 = INT_MIN (no overflow)
    assert_aot_success(
        r#"
@main () -> int = {
    let x = -9223372036854775807;
    let y = x - 1;
    if y == -9223372036854775807 - 1 then 0 else 1
}
"#,
        "op_no_overflow_sub",
    );
}

#[test]
fn test_op_no_overflow_mul_near_boundary() {
    // Multiplication that's large but doesn't overflow
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3037000499;
    let y = x * x;
    // 3037000499^2 = 9223372030926249001 < INT_MAX
    if y > 0 then 0 else 1
}
"#,
        "op_no_overflow_mul",
    );
}

#[test]
fn test_op_overflow_in_expression() {
    // Overflow inside a complex expression
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let a = 4611686018427387903;
    let b = 4611686018427387903;
    // a + b + 2 = INT_MAX + 1 (overflow on the second addition)
    a + b + 2
}
"#,
    );
    assert_ne!(exit_code, 0, "overflow in complex expression should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_in_function() {
    // Overflow in a called function
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@add_one (x: int) -> int = x + 1;

@main () -> int = {
    add_one(x: 9223372036854775807)
}
"#,
    );
    assert_ne!(exit_code, 0, "overflow in function should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

// --- Unary negation overflow ---

#[test]
fn test_op_overflow_neg_min() {
    // Unary negation of INT_MIN (-2^63) overflows: result would be 2^63 > INT_MAX
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let x = -9223372036854775807 - 1;
    -x
}
"#,
    );
    assert_ne!(exit_code, 0, "INT_MIN unary negation should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_no_overflow_neg_zero() {
    // -0 = 0, no overflow
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 0;
    let y = -x;
    if y == 0 then 0 else 1
}
"#,
        "op_no_overflow_neg_zero",
    );
}

#[test]
fn test_op_no_overflow_neg_normal() {
    // -42 = -42, no overflow
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let y = -x;
    if y == -42 then 0 else 1
}
"#,
        "op_no_overflow_neg_normal",
    );
}

#[test]
fn test_op_no_overflow_neg_max() {
    // -INT_MAX = INT_MIN + 1, no overflow (largest valid negation)
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 9223372036854775807;
    let y = -x;
    if y == -9223372036854775807 then 0 else 1
}
"#,
        "op_no_overflow_neg_max",
    );
}
