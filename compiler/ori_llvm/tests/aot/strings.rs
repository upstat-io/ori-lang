//! String Method AOT Tests
//!
//! Tests for string builtin methods and operations through the AOT pipeline.
//! Covers methods known to be in the builtin table (length, `is_empty`, concat,
//! iter, clone, `to_str`) and methods that may or may not be available yet
//! (contains, `starts_with`, split, trim, etc.) to build a gap inventory.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── length / len ───

#[test]
fn test_str_length_basic() {
    assert_aot_success(
        include_str!("fixtures/strings/str_length_basic.ori"),
        "str_length_basic",
    );
}

#[test]
fn test_str_length_empty() {
    assert_aot_success(
        include_str!("fixtures/strings/str_length_empty.ori"),
        "str_length_empty",
    );
}

#[test]
fn test_str_length_single_char() {
    assert_aot_success(
        include_str!("fixtures/strings/str_length_single_char.ori"),
        "str_length_single",
    );
}

#[test]
fn test_str_length_with_spaces() {
    assert_aot_success(
        include_str!("fixtures/strings/str_length_with_spaces.ori"),
        "str_length_spaces",
    );
}

#[test]
fn test_str_length_with_escapes() {
    assert_aot_success(
        include_str!("fixtures/strings/str_length_with_escapes.ori"),
        "str_length_escapes",
    );
}

#[test]
fn test_str_len_alias() {
    assert_aot_success(
        include_str!("fixtures/strings/str_len_alias.ori"),
        "str_len_alias",
    );
}

// ─── is_empty ───

#[test]
fn test_str_is_empty_true() {
    assert_aot_success(
        include_str!("fixtures/strings/str_is_empty_true.ori"),
        "str_is_empty_true",
    );
}

#[test]
fn test_str_is_empty_false() {
    assert_aot_success(
        include_str!("fixtures/strings/str_is_empty_false.ori"),
        "str_is_empty_false",
    );
}

#[test]
fn test_str_is_empty_space() {
    assert_aot_success(
        include_str!("fixtures/strings/str_is_empty_space.ori"),
        "str_is_empty_space",
    );
}

// ─── concat ───

#[test]
fn test_str_concat_basic() {
    assert_aot_success(
        include_str!("fixtures/strings/str_concat_basic.ori"),
        "str_concat_basic",
    );
}

#[test]
fn test_str_concat_empty_left() {
    assert_aot_success(
        include_str!("fixtures/strings/str_concat_empty_left.ori"),
        "str_concat_empty_left",
    );
}

#[test]
fn test_str_concat_empty_right() {
    assert_aot_success(
        include_str!("fixtures/strings/str_concat_empty_right.ori"),
        "str_concat_empty_right",
    );
}

#[test]
fn test_str_concat_chain() {
    assert_aot_success(
        include_str!("fixtures/strings/str_concat_chain.ori"),
        "str_concat_chain",
    );
}

// ─── to_str (identity for strings) ───

#[test]
fn test_str_to_str_identity() {
    assert_aot_success(
        include_str!("fixtures/strings/str_to_str_identity.ori"),
        "str_to_str_identity",
    );
}

// ─── clone ───

#[test]
fn test_str_clone() {
    assert_aot_success(include_str!("fixtures/strings/str_clone.ori"), "str_clone");
}

#[test]
fn test_str_clone_independence() {
    assert_aot_success(
        include_str!("fixtures/strings/str_clone_independence.ori"),
        "str_clone_indep",
    );
}

// ─── iter ───

#[test]
fn test_str_iter_count() {
    assert_aot_success(
        include_str!("fixtures/strings/str_iter_count.ori"),
        "str_iter_count",
    );
}

#[test]
fn test_str_iter_empty() {
    assert_aot_success(
        include_str!("fixtures/strings/str_iter_empty.ori"),
        "str_iter_empty",
    );
}

#[test]
fn test_str_iter_for_loop() {
    assert_aot_success(
        include_str!("fixtures/strings/str_iter_for_loop.ori"),
        "str_iter_for",
    );
}

// ─── String comparison ───

#[test]
fn test_str_compare_equal() {
    assert_aot_success(
        include_str!("fixtures/strings/str_compare_equal.ori"),
        "str_cmp_equal",
    );
}

#[test]
fn test_str_compare_not_equal() {
    assert_aot_success(
        include_str!("fixtures/strings/str_compare_not_equal.ori"),
        "str_cmp_not_equal",
    );
}

#[test]
fn test_str_compare_less() {
    assert_aot_success(
        include_str!("fixtures/strings/str_compare_less.ori"),
        "str_cmp_less",
    );
}

#[test]
fn test_str_compare_greater() {
    assert_aot_success(
        include_str!("fixtures/strings/str_compare_greater.ori"),
        "str_cmp_greater",
    );
}

#[test]
fn test_str_compare_prefix() {
    assert_aot_success(
        include_str!("fixtures/strings/str_compare_prefix.ori"),
        "str_cmp_prefix",
    );
}

// ─── String + type conversion concat ───

#[test]
fn test_str_concat_with_int_to_str() {
    assert_aot_success(
        include_str!("fixtures/strings/str_concat_with_int_to_str.ori"),
        "str_concat_int_to_str",
    );
}

#[test]
fn test_str_concat_with_bool_to_str() {
    assert_aot_success(
        include_str!("fixtures/strings/str_concat_with_bool_to_str.ori"),
        "str_concat_bool_to_str",
    );
}

// ─── String in data structures ───

#[test]
fn test_str_in_tuple() {
    assert_aot_success(
        include_str!("fixtures/strings/str_in_tuple.ori"),
        "str_in_tuple",
    );
}

#[test]
fn test_str_in_struct() {
    assert_aot_success(
        include_str!("fixtures/strings/str_in_struct.ori"),
        "str_in_struct",
    );
}

#[test]
fn test_str_struct_field_length() {
    assert_aot_success(
        include_str!("fixtures/strings/str_struct_field_length.ori"),
        "str_struct_field_len",
    );
}

// ─── String methods not in builtin table (expected gaps) ───

#[test]
fn test_str_contains() {
    assert_aot_success(
        include_str!("fixtures/strings/str_contains.ori"),
        "str_contains",
    );
}

#[test]
fn test_str_starts_with() {
    assert_aot_success(
        include_str!("fixtures/strings/str_starts_with.ori"),
        "str_starts_with",
    );
}

#[test]
fn test_str_ends_with() {
    assert_aot_success(
        include_str!("fixtures/strings/str_ends_with.ori"),
        "str_ends_with",
    );
}

#[test]
fn test_str_trim() {
    assert_aot_success(include_str!("fixtures/strings/str_trim.ori"), "str_trim");
}

#[test]
fn test_str_to_uppercase() {
    assert_aot_success(
        include_str!("fixtures/strings/str_to_uppercase.ori"),
        "str_to_uppercase",
    );
}

#[test]
fn test_str_to_lowercase() {
    assert_aot_success(
        include_str!("fixtures/strings/str_to_lowercase.ori"),
        "str_to_lowercase",
    );
}

#[test]
fn test_str_replace() {
    assert_aot_success(
        include_str!("fixtures/strings/str_replace.ori"),
        "str_replace",
    );
}

#[test]
fn test_str_split() {
    assert_aot_success(include_str!("fixtures/strings/str_split.ori"), "str_split");
}

#[test]
fn test_str_repeat() {
    assert_aot_success(
        include_str!("fixtures/strings/str_repeat.ori"),
        "str_repeat",
    );
}

#[test]
fn test_str_chars() {
    assert_aot_success(include_str!("fixtures/strings/str_chars.ori"), "str_chars");
}
