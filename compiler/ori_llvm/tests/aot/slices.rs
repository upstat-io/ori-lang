//! AOT tests for seamless slice operations.
//!
//! Exercises the LLVM codegen paths for `list.slice()`, `list.take()`,
//! `list.drop()`, `str.substring()`, and `str.slice()`. These methods
//! produce slices that share the underlying buffer via the sign-bit
//! encoding in the `cap` field.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── list.slice(start:, end:) ───

#[test]
fn test_list_slice_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30, 40, 50];
    let ys = xs.slice(start: 1, end: 4);
    if ys.length() == 3 && ys.first().unwrap() == 20 && ys.last().unwrap() == 40
        then 0
        else 1
}
"#,
        "list_slice_basic",
    );
}

#[test]
fn test_list_slice_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.slice(start: 0, end: 3);
    if ys.length() == 3 && ys.first().unwrap() == 1 && ys.last().unwrap() == 3
        then 0
        else 1
}
"#,
        "list_slice_full",
    );
}

#[test]
fn test_list_slice_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.slice(start: 1, end: 1);
    if ys.length() == 0 then 0 else 1
}
"#,
        "list_slice_empty",
    );
}

#[test]
fn test_list_slice_single_element() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let ys = xs.slice(start: 1, end: 2);
    if ys.length() == 1 && ys.first().unwrap() == 20
        then 0
        else 1
}
"#,
        "list_slice_single_element",
    );
}

#[test]
fn test_list_slice_from_start() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let ys = xs.slice(start: 0, end: 3);
    if ys.length() == 3 && ys.first().unwrap() == 1 && ys.last().unwrap() == 3
        then 0
        else 1
}
"#,
        "list_slice_from_start",
    );
}

#[test]
fn test_list_slice_to_end() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let ys = xs.slice(start: 3, end: 5);
    if ys.length() == 2 && ys.first().unwrap() == 4 && ys.last().unwrap() == 5
        then 0
        else 1
}
"#,
        "list_slice_to_end",
    );
}

// ─── list.take(count:) ───

#[test]
fn test_list_take_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30, 40, 50];
    let ys = xs.take(count: 3);
    if ys.length() == 3 && ys.first().unwrap() == 10 && ys.last().unwrap() == 30
        then 0
        else 1
}
"#,
        "list_take_basic",
    );
}

#[test]
fn test_list_take_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.take(count: 0);
    if ys.length() == 0 then 0 else 1
}
"#,
        "list_take_zero",
    );
}

#[test]
fn test_list_take_all() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.take(count: 3);
    if ys.length() == 3 && ys.first().unwrap() == 1 && ys.last().unwrap() == 3
        then 0
        else 1
}
"#,
        "list_take_all",
    );
}

// ─── list.drop(count:) ───

#[test]
fn test_list_drop_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30, 40, 50];
    let ys = xs.drop(count: 2);
    if ys.length() == 3 && ys.first().unwrap() == 30 && ys.last().unwrap() == 50
        then 0
        else 1
}
"#,
        "list_drop_basic",
    );
}

#[test]
fn test_list_drop_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.drop(count: 0);
    if ys.length() == 3 && ys.first().unwrap() == 1
        then 0
        else 1
}
"#,
        "list_drop_zero",
    );
}

#[test]
fn test_list_drop_all() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.drop(count: 3);
    if ys.length() == 0 then 0 else 1
}
"#,
        "list_drop_all",
    );
}

// ─── str.substring(start:, end:) ───

#[test]
fn test_str_substring_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "hello world";
    let $sub = s.substring(start: 0, end: 5);
    if sub == "hello" then 0 else 1
}
"#,
        "str_substring_basic",
    );
}

#[test]
fn test_str_substring_middle() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "hello world";
    let $sub = s.substring(start: 6, end: 11);
    if sub == "world" then 0 else 1
}
"#,
        "str_substring_middle",
    );
}

#[test]
fn test_str_substring_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "hello";
    let $sub = s.substring(start: 2, end: 2);
    if sub == "" then 0 else 1
}
"#,
        "str_substring_empty",
    );
}

// ─── str.slice(start:, end:) — alias for substring ───

#[test]
fn test_str_slice_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "abcdef";
    let $sub = s.slice(start: 1, end: 4);
    if sub == "bcd" then 0 else 1
}
"#,
        "str_slice_basic",
    );
}

// ─── Slice chaining ───

#[test]
fn test_list_take_then_drop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let ys = xs.take(count: 4).drop(count: 1);
    if ys.length() == 3 && ys.first().unwrap() == 2 && ys.last().unwrap() == 4
        then 0
        else 1
}
"#,
        "list_take_then_drop",
    );
}

#[test]
fn test_list_slice_then_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let ys = xs.slice(start: 2, end: 8);
    if ys.length() == 6 then 0 else 1
}
"#,
        "list_slice_then_length",
    );
}

// ─── Slice RC safety (original list should survive) ───

#[test]
fn test_list_slice_preserves_original() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let ys = xs.slice(start: 1, end: 4);
    // Both xs and ys should be valid — slice shares the buffer
    if xs.length() == 5 && ys.length() == 3
        && xs.first().unwrap() == 1 && ys.first().unwrap() == 2
        then 0
        else 1
}
"#,
        "list_slice_preserves_original",
    );
}
