//! `[str]` iteration tests — T1 (heap strings) and T1b (mixed SSO/heap).

use crate::util::assert_aot_success;

#[test]
fn test_str_list_full_iteration() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/str_list_full_iteration.ori"),
        "str_list_full_iteration",
    );
}

#[test]
fn test_str_list_partial_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/str_list_partial_break.ori"),
        "str_list_partial_break",
    );
}

#[test]
fn test_str_list_yield_lengths() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/str_list_yield_lengths.ori"),
        "str_list_yield_lengths",
    );
}

#[test]
fn test_str_list_passed_to_two_functions() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/str_list_passed_to_two_functions.ori"),
        "str_list_passed_to_two_functions",
    );
}

// T1b: mixed SSO/heap strings — semantic pin for SSO check in elem_dec_fn

#[test]
fn test_str_list_mixed_sso_heap() {
    // Mix of short strings (<= 23 bytes, SSO inline) and long strings (> 23 bytes, heap).
    // The elem_dec_fn thunk must correctly skip SSO strings (no RC to dec) and
    // only dec heap strings. If the SSO check is broken, this leaks or double-frees.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/str_list_mixed_sso_heap.ori"),
        "str_list_mixed_sso_heap",
    );
}

// Semantic pin — slice-backed string through string methods

/// `substring(...).to_uppercase()` must not crash on slice-backed strings.
/// Without the `is_slice_cap` guard, `ori_rc_is_unique` dereferences an
/// interior pointer → misaligned access → abort.
#[test]
fn test_substring_to_uppercase() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/substring_to_uppercase.ori"),
        "substring_to_uppercase",
    );
}

/// `split(...)[0].to_lowercase()` — slice from split through a string method.
#[test]
fn test_split_first_to_lowercase() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/split_first_to_lowercase.ori"),
        "split_first_to_lowercase",
    );
}

// Semantic pin — repeat(1) must not double-free

/// `s.repeat(count: 1)` must return an owned string, not alias the original.
/// Without the fix, RC double-free when both original and result are live.
#[test]
fn test_repeat_one_no_double_free() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/repeat_one_no_double_free.ori"),
        "repeat_one_no_double_free",
    );
}

/// `substring(...).repeat(count: 1)` — slice through repeat.
#[test]
fn test_substring_repeat_one() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/substring_repeat_one.ori"),
        "substring_repeat_one",
    );
}

// Semantic pin — concat("") must not double-free

/// `heap_str + ""` must return an owned result, not alias the operand.
#[test]
fn test_concat_empty_right_no_double_free() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/concat_empty_right_no_double_free.ori"),
        "concat_empty_right_no_double_free",
    );
}

/// `"" + heap_str` — empty left operand.
#[test]
fn test_concat_empty_left_no_double_free() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/str_list/concat_empty_left_no_double_free.ori"),
        "concat_empty_left_no_double_free",
    );
}
