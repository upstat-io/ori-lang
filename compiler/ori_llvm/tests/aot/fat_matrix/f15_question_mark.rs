//! F15: ? Propagation — fat pointer types used in Option/Result with ? operator.
//!
//! Tests early return codegen, cleanup on error path, and RC handling when fat
//! pointer values are wrapped in Option/Result and propagated with ?.

use crate::util::assert_aot_success;

// T15: ? on Option<int> — Some path
#[test]
fn test_fm_question_option_int_some() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_option_int_some.ori"),
        "fm_question_option_int_some",
    );
}

// T15: ? on Option<int> — None path
#[test]
fn test_fm_question_option_int_none() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_option_int_none.ori"),
        "fm_question_option_int_none",
    );
}

// T16: ? on Option<str> — Some path
#[test]
fn test_fm_question_option_str_some() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_option_str_some.ori"),
        "fm_question_option_str_some",
    );
}

// T16: ? on Option<str> — None path
#[test]
fn test_fm_question_option_str_none() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_option_str_none.ori"),
        "fm_question_option_str_none",
    );
}

// ? with fat value in scope that must be cleaned up on early return
#[test]
fn test_fm_question_cleanup_fat_scope() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_cleanup_fat_scope.ori"),
        "fm_question_cleanup_fat_scope",
    );
}

// ? with fat value produced after successful extraction
#[test]
fn test_fm_question_fat_after_extract() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_fat_after_extract.ori"),
        "fm_question_fat_after_extract",
    );
}

// Multiple ? in same function with fat values
#[test]
fn test_fm_question_multiple() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f15_question_mark/fm_question_multiple.ori"),
        "fm_question_multiple",
    );
}
