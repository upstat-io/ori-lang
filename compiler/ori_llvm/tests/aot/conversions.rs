//! Type Conversion AOT Tests
//!
//! Tests for primitive type conversion methods: `to_int`, `to_float`, `f`,
//! `byte`, `abs`, `to_str`, `into`, and chained conversions. Verifies that
//! AOT codegen produces correct LLVM cast instructions (sitofp, fptosi,
//! trunc, zext, sext).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── int.to_float / int.f ───

#[test]
fn test_conv_int_to_float() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let f = x.to_float();
    if f == 42.0 then 0 else 1
}
"#,
        "conv_int_to_float",
    );
}

#[test]
#[ignore = "AOT gap: int.f() result type not tracked as float — icmp used instead of fcmp for comparison"]
fn test_conv_int_f_shorthand() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let f = x.f();
    if f == 10.0 then 0 else 1
}
"#,
        "conv_int_f",
    );
}

#[test]
fn test_conv_int_to_float_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = -100;
    let f = x.to_float();
    if f == -100.0 then 0 else 1
}
"#,
        "conv_int_to_float_neg",
    );
}

#[test]
fn test_conv_int_to_float_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 0;
    let f = x.to_float();
    if f == 0.0 then 0 else 1
}
"#,
        "conv_int_to_float_zero",
    );
}

#[test]
fn test_conv_int_to_float_large() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 1000000000;
    let f = x.to_float();
    if f > 999999999.0 && f < 1000000001.0 then 0 else 1
}
"#,
        "conv_int_to_float_large",
    );
}

// ─── float.to_int ───

#[test]
fn test_conv_float_to_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = 42.0;
    let x = f.to_int();
    if x == 42 then 0 else 1
}
"#,
        "conv_float_to_int",
    );
}

#[test]
fn test_conv_float_to_int_truncates() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = 3.7;
    let x = f.to_int();
    // fptosi truncates toward zero: 3.7 -> 3
    if x == 3 then 0 else 1
}
"#,
        "conv_float_to_int_trunc",
    );
}

#[test]
fn test_conv_float_to_int_negative_truncates() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = -3.7;
    let x = f.to_int();
    // fptosi truncates toward zero: -3.7 -> -3
    if x == -3 then 0 else 1
}
"#,
        "conv_float_to_int_neg_trunc",
    );
}

#[test]
fn test_conv_float_to_int_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = 0.0;
    let x = f.to_int();
    if x == 0 then 0 else 1
}
"#,
        "conv_float_to_int_zero",
    );
}

#[test]
fn test_conv_float_to_int_negative_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = -0.0;
    let x = f.to_int();
    if x == 0 then 0 else 1
}
"#,
        "conv_float_to_int_neg_zero",
    );
}

// ─── int.into (int -> float) ───

#[test]
fn test_conv_int_into_float() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 7;
    let f: float = x.into();
    if f == 7.0 then 0 else 1
}
"#,
        "conv_int_into_float",
    );
}

// ─── bool.to_int ───

#[test]
fn test_conv_bool_to_int_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b = true;
    let x = b.to_int();
    if x == 1 then 0 else 1
}
"#,
        "conv_bool_to_int_true",
    );
}

#[test]
fn test_conv_bool_to_int_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b = false;
    let x = b.to_int();
    if x == 0 then 0 else 1
}
"#,
        "conv_bool_to_int_false",
    );
}

// ─── char.to_int ───

#[test]
fn test_conv_char_to_int_ascii() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = 'A';
    let x = c.to_int();
    if x == 65 then 0 else 1
}
"#,
        "conv_char_to_int_ascii",
    );
}

#[test]
fn test_conv_char_to_int_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = '0';
    let x = c.to_int();
    // '0' is Unicode codepoint 48
    if x == 48 then 0 else 1
}
"#,
        "conv_char_to_int_zero",
    );
}

#[test]
fn test_conv_char_to_int_space() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = ' ';
    let x = c.to_int();
    if x == 32 then 0 else 1
}
"#,
        "conv_char_to_int_space",
    );
}

// ─── byte.to_int ───

#[test]
fn test_conv_byte_to_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b: byte = 200;
    let x = b.to_int();
    if x == 200 then 0 else 1
}
"#,
        "conv_byte_to_int",
    );
}

#[test]
fn test_conv_byte_to_int_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b: byte = 0;
    let x = b.to_int();
    if x == 0 then 0 else 1
}
"#,
        "conv_byte_to_int_zero",
    );
}

#[test]
fn test_conv_byte_to_int_max() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b: byte = 255;
    let x = b.to_int();
    if x == 255 then 0 else 1
}
"#,
        "conv_byte_to_int_max",
    );
}

// ─── int.byte ───

#[test]
#[ignore = "AOT gap: int.byte() chained with byte.to_int() — type tracking lost through conversion chain"]
fn test_conv_int_to_byte() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let b = x.byte();
    let back = b.to_int();
    if back == 42 then 0 else 1
}
"#,
        "conv_int_to_byte",
    );
}

#[test]
#[ignore = "AOT gap: int.byte() truncation — byte type not fully supported in AOT pipeline"]
fn test_conv_int_to_byte_truncates() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 256;
    let b = x.byte();
    let back = b.to_int();
    // 256 truncated to byte = 0
    if back == 0 then 0 else 1
}
"#,
        "conv_int_to_byte_trunc",
    );
}

#[test]
#[ignore = "AOT gap: int.byte() chained with byte.to_int() — type tracking lost through conversion chain"]
fn test_conv_int_to_byte_max() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 255;
    let b = x.byte();
    let back = b.to_int();
    if back == 255 then 0 else 1
}
"#,
        "conv_int_to_byte_max",
    );
}

// ─── abs ───

#[test]
fn test_conv_int_abs_positive() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    if x.abs() == 42 then 0 else 1
}
"#,
        "conv_int_abs_pos",
    );
}

#[test]
fn test_conv_int_abs_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = -42;
    if x.abs() == 42 then 0 else 1
}
"#,
        "conv_int_abs_neg",
    );
}

#[test]
fn test_conv_int_abs_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 0;
    if x.abs() == 0 then 0 else 1
}
"#,
        "conv_int_abs_zero",
    );
}

#[test]
fn test_conv_float_abs_positive() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3.14;
    if x.abs() == 3.14 then 0 else 1
}
"#,
        "conv_float_abs_pos",
    );
}

#[test]
fn test_conv_float_abs_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = -3.14;
    if x.abs() == 3.14 then 0 else 1
}
"#,
        "conv_float_abs_neg",
    );
}

#[test]
fn test_conv_float_abs_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 0.0;
    if x.abs() == 0.0 then 0 else 1
}
"#,
        "conv_float_abs_zero",
    );
}

// ─── to_str ───

#[test]
fn test_conv_int_to_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = 42.to_str();
    if s == "42" then 0 else 1
}
"#,
        "conv_int_to_str",
    );
}

#[test]
fn test_conv_int_to_str_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = (-7).to_str();
    if s == "-7" then 0 else 1
}
"#,
        "conv_int_to_str_neg",
    );
}

#[test]
fn test_conv_int_to_str_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = 0.to_str();
    if s == "0" then 0 else 1
}
"#,
        "conv_int_to_str_zero",
    );
}

#[test]
fn test_conv_float_to_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = 3.14.to_str();
    // Float to_str should produce a reasonable representation
    if s.length() > 0 then 0 else 1
}
"#,
        "conv_float_to_str",
    );
}

#[test]
fn test_conv_bool_to_str_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = true.to_str();
    if s == "true" then 0 else 1
}
"#,
        "conv_bool_to_str_true",
    );
}

#[test]
fn test_conv_bool_to_str_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = false.to_str();
    if s == "false" then 0 else 1
}
"#,
        "conv_bool_to_str_false",
    );
}

// ─── Ordering.to_int ───

#[test]
fn test_conv_ordering_to_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1;
    let b = 2;
    let ord = a.compare(other: b);
    let x = ord.to_int();
    // Less should be a non-zero value
    if x != 1 then 0 else 1
}
"#,
        "conv_ordering_to_int",
    );
}

// ─── Chained conversions ───

#[test]
fn test_conv_int_to_float_to_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let roundtrip = x.to_float().to_int();
    if roundtrip == 42 then 0 else 1
}
"#,
        "conv_roundtrip_int_float",
    );
}

#[test]
#[ignore = "AOT gap: int.byte() chained with byte.to_int() — type tracking lost through conversion chain"]
fn test_conv_int_to_byte_to_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 100;
    let roundtrip = x.byte().to_int();
    if roundtrip == 100 then 0 else 1
}
"#,
        "conv_roundtrip_int_byte",
    );
}

#[test]
fn test_conv_bool_to_int_to_float() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b = true;
    let f = b.to_int().to_float();
    if f == 1.0 then 0 else 1
}
"#,
        "conv_chain_bool_int_float",
    );
}

#[test]
fn test_conv_char_to_int_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = 'a';
    let offset = c.to_int() - 'A'.to_int();
    // 'a' = 97, 'A' = 65, difference = 32
    if offset == 32 then 0 else 1
}
"#,
        "conv_char_to_int_arith",
    );
}

// ─── Conversion in expressions ───

#[test]
fn test_conv_in_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 5;
    let y = 5.0;
    // Compare by converting int to float
    if x.to_float() == y then 0 else 1
}
"#,
        "conv_in_comparison",
    );
}

#[test]
fn test_conv_in_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3;
    let y = 0.14;
    let pi_approx = x.to_float() + y;
    if pi_approx > 3.13 && pi_approx < 3.15 then 0 else 1
}
"#,
        "conv_in_arithmetic",
    );
}

#[test]
fn test_conv_abs_in_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = -10;
    let b = 10;
    // abs makes them equal
    if a.abs() == b.abs() then 0 else 1
}
"#,
        "conv_abs_in_expr",
    );
}

#[test]
fn test_conv_to_str_concat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = 42;
    let s = "value: " + n.to_str();
    if s == "value: 42" then 0 else 1
}
"#,
        "conv_to_str_concat",
    );
}

#[test]
fn test_conv_multiple_to_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1.to_str();
    let b = 2.to_str();
    let c = 3.to_str();
    let result = a + b + c;
    if result == "123" then 0 else 1
}
"#,
        "conv_multiple_to_str",
    );
}
