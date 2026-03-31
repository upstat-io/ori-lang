//! F05: Closure Parameter — fat pointer types passed through closure calls.
//!
//! Tests indirect call ABI, trampoline generation, and RC handling when fat
//! pointer values flow through closure parameters (as opposed to captures).

use crate::util::assert_aot_success;

// T4: SSO string passed as closure parameter
#[test]
fn test_fm_closure_param_str_sso() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_str_sso.ori"),
        "fm_closure_param_str_sso",
    );
}

// T5: Heap string passed as closure parameter
#[test]
fn test_fm_closure_param_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_str_heap.ori"),
        "fm_closure_param_str_heap",
    );
}

// T6: List of scalars passed as closure parameter
#[test]
fn test_fm_closure_param_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_list_scalar.ori"),
        "fm_closure_param_list_scalar",
    );
}

// T7: List of fat pointers passed as closure parameter
#[test]
fn test_fm_closure_param_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_list_fat.ori"),
        "fm_closure_param_list_fat",
    );
}

// T8: Struct with scalar fields passed as closure parameter
#[test]
fn test_fm_closure_param_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_struct_scalar.ori"),
        "fm_closure_param_struct_scalar",
    );
}

// T9: Struct with fat fields passed as closure parameter
#[test]
fn test_fm_closure_param_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_struct_fat.ori"),
        "fm_closure_param_struct_fat",
    );
}

// T15: Option<int> passed as closure parameter
#[test]
fn test_fm_closure_param_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_option_int.ori"),
        "fm_closure_param_option_int",
    );
}

// T17: Map passed as closure parameter
#[test]
fn test_fm_closure_param_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_map.ori"),
        "fm_closure_param_map",
    );
}

// T18: Tuple (mixed) passed as closure parameter
#[test]
fn test_fm_closure_param_tuple_mixed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_tuple_mixed.ori"),
        "fm_closure_param_tuple_mixed",
    );
}

// Closure with fat capture AND fat parameter
#[test]
fn test_fm_closure_param_with_fat_capture() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f05_closure_param/fm_closure_param_with_fat_capture.ori"
        ),
        "fm_closure_param_with_fat_capture",
    );
}

// Closure passed to higher-order function with fat parameter
#[test]
fn test_fm_closure_param_higher_order() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_higher_order.ori"),
        "fm_closure_param_higher_order",
    );
}

// Multiple fat parameters to closure
#[test]
fn test_fm_closure_param_multi_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f05_closure_param/fm_closure_param_multi_fat.ori"),
        "fm_closure_param_multi_fat",
    );
}
