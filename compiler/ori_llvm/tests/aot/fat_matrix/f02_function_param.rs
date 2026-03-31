//! F02: Function Parameter — fat pointer types passed as arguments to functions.
//!
//! Tests ABI correctness (direct vs indirect passing), borrow elision, and
//! RC inc/dec at call boundaries.

use crate::util::assert_aot_success;

// T4: String (SSO) as parameter
#[test]
fn test_fm_param_str_sso() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_str_sso.ori"),
        "fm_param_str_sso",
    );
}

// T5: String (heap) as parameter
#[test]
fn test_fm_param_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_str_heap.ori"),
        "fm_param_str_heap",
    );
}

// T5: String used after passing to function (RC inc required)
#[test]
fn test_fm_param_str_heap_reuse() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_str_heap_reuse.ori"),
        "fm_param_str_heap_reuse",
    );
}

// T6: List of scalars as parameter
#[test]
fn test_fm_param_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_list_scalar.ori"),
        "fm_param_list_scalar",
    );
}

// T7: List of fat pointers as parameter
#[test]
fn test_fm_param_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_list_fat.ori"),
        "fm_param_list_fat",
    );
}

// T8: Struct (scalar fields) as parameter
#[test]
fn test_fm_param_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_struct_scalar.ori"),
        "fm_param_struct_scalar",
    );
}

// T9: Struct (fat fields) as parameter
#[test]
fn test_fm_param_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_struct_fat.ori"),
        "fm_param_struct_fat",
    );
}

// T15: Option<int> as parameter
#[test]
fn test_fm_param_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_option_int.ori"),
        "fm_param_option_int",
    );
}

// T16: Option<str> as parameter
#[test]
fn test_fm_param_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_option_str.ori"),
        "fm_param_option_str",
    );
}

// T17: Map as parameter
#[test]
fn test_fm_param_map_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_map_str.ori"),
        "fm_param_map_str",
    );
}

// T18: Tuple (mixed) as parameter
#[test]
fn test_fm_param_tuple_mixed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_tuple_mixed.ori"),
        "fm_param_tuple_mixed",
    );
}

// Multiple fat params — two fat values passed simultaneously
#[test]
fn test_fm_param_multiple_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f02_function_param/fm_param_multiple_fat.ori"),
        "fm_param_multiple_fat",
    );
}
