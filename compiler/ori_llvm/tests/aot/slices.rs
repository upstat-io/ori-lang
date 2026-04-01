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
        include_str!("fixtures/slices/list_slice_basic.ori"),
        "list_slice_basic",
    );
}

#[test]
fn test_list_slice_full() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_full.ori"),
        "list_slice_full",
    );
}

#[test]
fn test_list_slice_empty() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_empty.ori"),
        "list_slice_empty",
    );
}

#[test]
fn test_list_slice_single_element() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_single_element.ori"),
        "list_slice_single_element",
    );
}

#[test]
fn test_list_slice_from_start() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_from_start.ori"),
        "list_slice_from_start",
    );
}

#[test]
fn test_list_slice_to_end() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_to_end.ori"),
        "list_slice_to_end",
    );
}

// ─── list.take(count:) ───

#[test]
fn test_list_take_basic() {
    assert_aot_success(
        include_str!("fixtures/slices/list_take_basic.ori"),
        "list_take_basic",
    );
}

#[test]
fn test_list_take_zero() {
    assert_aot_success(
        include_str!("fixtures/slices/list_take_zero.ori"),
        "list_take_zero",
    );
}

#[test]
fn test_list_take_all() {
    assert_aot_success(
        include_str!("fixtures/slices/list_take_all.ori"),
        "list_take_all",
    );
}

// ─── list.drop(count:) ───

#[test]
fn test_list_drop_basic() {
    assert_aot_success(
        include_str!("fixtures/slices/list_drop_basic.ori"),
        "list_drop_basic",
    );
}

#[test]
fn test_list_drop_zero() {
    assert_aot_success(
        include_str!("fixtures/slices/list_drop_zero.ori"),
        "list_drop_zero",
    );
}

#[test]
fn test_list_drop_all() {
    assert_aot_success(
        include_str!("fixtures/slices/list_drop_all.ori"),
        "list_drop_all",
    );
}

// ─── str.substring(start:, end:) ───

#[test]
fn test_str_substring_basic() {
    assert_aot_success(
        include_str!("fixtures/slices/str_substring_basic.ori"),
        "str_substring_basic",
    );
}

#[test]
fn test_str_substring_middle() {
    assert_aot_success(
        include_str!("fixtures/slices/str_substring_middle.ori"),
        "str_substring_middle",
    );
}

#[test]
fn test_str_substring_empty() {
    assert_aot_success(
        include_str!("fixtures/slices/str_substring_empty.ori"),
        "str_substring_empty",
    );
}

// ─── str.slice(start:, end:) — alias for substring ───

#[test]
fn test_str_slice_basic() {
    assert_aot_success(
        include_str!("fixtures/slices/str_slice_basic.ori"),
        "str_slice_basic",
    );
}

// ─── Slice chaining ───

#[test]
fn test_list_take_then_drop() {
    assert_aot_success(
        include_str!("fixtures/slices/list_take_then_drop.ori"),
        "list_take_then_drop",
    );
}

#[test]
fn test_list_slice_then_length() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_then_length.ori"),
        "list_slice_then_length",
    );
}

// ─── Slice RC safety (original list should survive) ───

#[test]
fn test_list_slice_preserves_original() {
    assert_aot_success(
        include_str!("fixtures/slices/list_slice_preserves_original.ori"),
        "list_slice_preserves_original",
    );
}
