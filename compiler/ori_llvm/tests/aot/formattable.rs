//! AOT Formattable Trait Codegen Tests (§3.16)
//!
//! End-to-end tests verifying that format spec expressions (`{value:spec}`)
//! generate correct native code through the LLVM backend. Each test compiles
//! Ori source to a native binary, runs it, and checks exit code 0.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// =============================================================================
// Integer Formatting
// =============================================================================

#[test]
fn test_aot_format_int_hex() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{255:x}` == "ff" && `{255:X}` == "FF" then 0 else 1
"#,
        "format_int_hex",
    );
}

#[test]
fn test_aot_format_int_binary() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{10:b}` == "1010" && `{10:#b}` == "0b1010" then 0 else 1
"#,
        "format_int_binary",
    );
}

#[test]
fn test_aot_format_int_octal() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{8:o}` == "10" && `{8:#o}` == "0o10" then 0 else 1
"#,
        "format_int_octal",
    );
}

#[test]
fn test_aot_format_int_sign() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{42:+}` == "+42" && `{-42:+}` == "-42" then 0 else 1
"#,
        "format_int_sign",
    );
}

#[test]
fn test_aot_format_int_zero_pad() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{42:08}` == "00000042" then 0 else 1
"#,
        "format_int_zero_pad",
    );
}

#[test]
fn test_aot_format_int_width_align() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let right = `{42:>8}`,
    let left = `{42:<8}`,
    let center = `{42:^8}`,
    if right == "      42" && left == "42      " && center == "   42   " then 0 else 1
)
"#,
        "format_int_width_align",
    );
}

// =============================================================================
// Float Formatting
// =============================================================================

#[test]
fn test_aot_format_float_fixed() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{3.14159:.2f}` == "3.14" then 0 else 1
"#,
        "format_float_fixed",
    );
}

#[test]
fn test_aot_format_float_precision() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{3.14159:.4}` == "3.1416" then 0 else 1
"#,
        "format_float_precision",
    );
}

#[test]
fn test_aot_format_float_percent() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{0.75:.1%}` == "75.0%" then 0 else 1
"#,
        "format_float_percent",
    );
}

#[test]
fn test_aot_format_float_sign() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{3.14:+.2f}` == "+3.14" && `{-3.14:+.2f}` == "-3.14" then 0 else 1
"#,
        "format_float_sign",
    );
}

// =============================================================================
// String Formatting
// =============================================================================

#[test]
fn test_aot_format_str_width() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let right = `{"hi":>10}`,
    let left = `{"hi":<10}`,
    if right == "        hi" && left == "hi        " then 0 else 1
)
"#,
        "format_str_width",
    );
}

#[test]
fn test_aot_format_str_fill() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{"hi":*>10}` == "********hi" then 0 else 1
"#,
        "format_str_fill",
    );
}

#[test]
fn test_aot_format_str_precision() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{"hello world":.5}` == "hello" then 0 else 1
"#,
        "format_str_precision",
    );
}

// =============================================================================
// Bool and Char Formatting
// =============================================================================

#[test]
fn test_aot_format_bool_width() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{true:>10}` == "      true" && `{false:<10}` == "false     " then 0 else 1
"#,
        "format_bool_width",
    );
}

#[test]
fn test_aot_format_char_width() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{'A':>5}` == "    A" then 0 else 1
"#,
        "format_char_width",
    );
}

// =============================================================================
// Custom Fill Characters
// =============================================================================

#[test]
fn test_aot_format_fill_center() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{"hi":*^10}` == "****hi****" then 0 else 1
"#,
        "format_fill_center",
    );
}

#[test]
fn test_aot_format_negative_hex() {
    assert_aot_success(
        r#"
@main () -> int =
    if `{-255:x}` == "-ff" && `{-42:#x}` == "-0x2a" then 0 else 1
"#,
        "format_negative_hex",
    );
}
