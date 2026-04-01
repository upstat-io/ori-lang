//! F03: Function Return — fat pointer types returned from functions.
//!
//! Tests return ABI (sret vs register), RC ownership transfer, and
//! correct cleanup when returned values go out of scope.

use crate::util::assert_aot_success;

// T4: String (SSO) returned from function
#[test]
fn test_fm_return_str_sso() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_str_sso.ori"),
        "fm_return_str_sso",
    );
}

// T5: String (heap) returned from function
#[test]
fn test_fm_return_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_str_heap.ori"),
        "fm_return_str_heap",
    );
}

// T6: List of scalars returned from function
#[test]
fn test_fm_return_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_list_scalar.ori"),
        "fm_return_list_scalar",
    );
}

// T7: List of fat pointers returned from function
#[test]
fn test_fm_return_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_list_fat.ori"),
        "fm_return_list_fat",
    );
}

// T8: Struct (scalar fields) returned from function
#[test]
fn test_fm_return_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_struct_scalar.ori"),
        "fm_return_struct_scalar",
    );
}

// T9: Struct (fat fields) returned from function
#[test]
fn test_fm_return_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_struct_fat.ori"),
        "fm_return_struct_fat",
    );
}

// T15: Option<int> returned from function
#[test]
fn test_fm_return_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_option_int.ori"),
        "fm_return_option_int",
    );
}

// T16: Option<str> returned from function
#[test]
fn test_fm_return_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_option_str.ori"),
        "fm_return_option_str",
    );
}

// T17: Map returned from function
#[test]
fn test_fm_return_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_map.ori"),
        "fm_return_map",
    );
}

// T18: Tuple (mixed) returned from function
#[test]
fn test_fm_return_tuple_mixed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_tuple_mixed.ori"),
        "fm_return_tuple_mixed",
    );
}

// Chained returns — returned fat value passed to another function
#[test]
fn test_fm_return_chained() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f03_function_return/fm_return_chained.ori"),
        "fm_return_chained",
    );
}
