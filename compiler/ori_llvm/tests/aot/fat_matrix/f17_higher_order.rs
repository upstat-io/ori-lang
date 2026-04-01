//! F17: Higher-Order — fat pointer types passed through function-typed parameters.
//!
//! Tests indirect call codegen, type erasure, and RC handling when fat pointer
//! values flow through higher-order function parameters.

use crate::util::assert_aot_success;

// Higher-order: function taking str -> int
#[test]
fn test_fm_higher_order_str_fn() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_str_fn.ori"),
        "fm_higher_order_str_fn",
    );
}

// Higher-order: function taking [int] -> int
#[test]
fn test_fm_higher_order_list_fn() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_list_fn.ori"),
        "fm_higher_order_list_fn",
    );
}

// Higher-order: lambda with fat capture passed as argument
#[test]
fn test_fm_higher_order_lambda_fat_capture() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f17_higher_order/fm_higher_order_lambda_fat_capture.ori"
        ),
        "fm_higher_order_lambda_fat_capture",
    );
}

// Higher-order: function returning int called twice (RC correctness)
#[test]
fn test_fm_higher_order_called_twice() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_called_twice.ori"),
        "fm_higher_order_called_twice",
    );
}

// Higher-order: compose two functions on str
#[test]
fn test_fm_higher_order_compose() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_compose.ori"),
        "fm_higher_order_compose",
    );
}

// Higher-order: function operating on struct with fat field
#[test]
fn test_fm_higher_order_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_struct_fat.ori"),
        "fm_higher_order_struct_fat",
    );
}

// Higher-order: function operating on map
#[test]
fn test_fm_higher_order_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_map.ori"),
        "fm_higher_order_map",
    );
}

// Higher-order: two different functions on same fat type
#[test]
fn test_fm_higher_order_different_fns() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f17_higher_order/fm_higher_order_different_fns.ori"),
        "fm_higher_order_different_fns",
    );
}
