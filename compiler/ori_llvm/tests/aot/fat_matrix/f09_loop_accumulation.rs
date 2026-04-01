//! F09: Loop Accumulation — accumulating fat pointer values across iterations.
//!
//! Tests phi nodes for mutable bindings, RC correctness when values are
//! reassigned in loops, and cleanup of replaced values.

use crate::util::assert_aot_success;

// Accumulate int sum in loop (scalar baseline)
#[test]
fn test_fm_loop_acc_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f09_loop_accumulation/fm_loop_acc_scalar.ori"),
        "fm_loop_acc_scalar",
    );
}

// Accumulate list length in loop
#[test]
fn test_fm_loop_acc_list_len() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f09_loop_accumulation/fm_loop_acc_list_len.ori"),
        "fm_loop_acc_list_len",
    );
}

// Accumulate map sizes
#[test]
fn test_fm_loop_acc_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f09_loop_accumulation/fm_loop_acc_map.ori"),
        "fm_loop_acc_map",
    );
}

// Accumulate through function calls on fat values
#[test]
fn test_fm_loop_acc_fn_call() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f09_loop_accumulation/fm_loop_acc_fn_call.ori"),
        "fm_loop_acc_fn_call",
    );
}
