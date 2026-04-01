//! F04: Closure Capture — fat pointer types captured in closure environments.
//!
//! This is the J17 bug area: closures capturing non-scalar values had missing
//! AIMS param ownership annotations, causing spurious `RcDec` on borrowed aliases.
//! Tests that each fat pointer type can be captured and used correctly.

use crate::util::assert_aot_success;

// T4: Closure captures SSO string
#[test]
fn test_fm_capture_str_sso() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_str_sso.ori"),
        "fm_capture_str_sso",
    );
}

// T5: Closure captures heap string
#[test]
fn test_fm_capture_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_str_heap.ori"),
        "fm_capture_str_heap",
    );
}

// T5: Closure captures heap string, called twice (RC must be correct)
#[test]
fn test_fm_capture_str_heap_two_calls() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f04_closure_capture/fm_capture_str_heap_two_calls.ori"
        ),
        "fm_capture_str_heap_two_calls",
    );
}

// T6: Closure captures list of scalars
#[test]
fn test_fm_capture_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_list_scalar.ori"),
        "fm_capture_list_scalar",
    );
}

// T7: Closure captures list of strings
#[test]
fn test_fm_capture_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_list_fat.ori"),
        "fm_capture_list_fat",
    );
}

// T8: Closure captures struct with scalar fields
#[test]
fn test_fm_capture_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_struct_scalar.ori"),
        "fm_capture_struct_scalar",
    );
}

// T9: Closure captures struct with fat fields
#[test]
fn test_fm_capture_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_struct_fat.ori"),
        "fm_capture_struct_fat",
    );
}

// T15: Closure captures Option<int>
#[test]
fn test_fm_capture_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_option_int.ori"),
        "fm_capture_option_int",
    );
}

// T17: Closure captures map
#[test]
fn test_fm_capture_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_map.ori"),
        "fm_capture_map",
    );
}

// Multiple captures: str + [int]
#[test]
fn test_fm_capture_multi_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_multi_fat.ori"),
        "fm_capture_multi_fat",
    );
}

// Closure passed as argument with fat capture
#[test]
fn test_fm_capture_passed_as_arg() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_passed_as_arg.ori"),
        "fm_capture_passed_as_arg",
    );
}

// Closure used in for loop body with fat capture
#[test]
fn test_fm_capture_in_loop() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f04_closure_capture/fm_capture_in_loop.ori"),
        "fm_capture_in_loop",
    );
}
