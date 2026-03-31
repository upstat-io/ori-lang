//! F01: Let Binding — fat pointer types bound via `let` and used in expressions.
//!
//! Tests that each fat pointer type category can be constructed, bound to a
//! variable, and used without leaks or double-frees.

use crate::util::assert_aot_success;

// T4: String (SSO — ≤23 bytes)
#[test]
fn test_fm_let_str_sso() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_str_sso.ori"),
        "fm_let_str_sso",
    );
}

// T5: String (heap — >23 bytes)
#[test]
fn test_fm_let_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_str_heap.ori"),
        "fm_let_str_heap",
    );
}

// T6: List of scalars
#[test]
fn test_fm_let_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_list_scalar.ori"),
        "fm_let_list_scalar",
    );
}

// T7: List of fat pointers
#[test]
fn test_fm_let_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_list_fat.ori"),
        "fm_let_list_fat",
    );
}

// T8: Struct (scalar fields only)
#[test]
fn test_fm_let_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_struct_scalar.ori"),
        "fm_let_struct_scalar",
    );
}

// T9: Struct (fat fields)
#[test]
fn test_fm_let_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_struct_fat.ori"),
        "fm_let_struct_fat",
    );
}

// T15: Option<int>
#[test]
fn test_fm_let_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_option_int.ori"),
        "fm_let_option_int",
    );
}

// T16: Option<str>
#[test]
fn test_fm_let_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_option_str.ori"),
        "fm_let_option_str",
    );
}

// T17: Map (str keys)
#[test]
fn test_fm_let_map_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_map_str.ori"),
        "fm_let_map_str",
    );
}

// T18: Tuple (mixed — str + int)
#[test]
fn test_fm_let_tuple_mixed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_tuple_mixed.ori"),
        "fm_let_tuple_mixed",
    );
}

// T12: Closure (no capture)
#[test]
fn test_fm_let_closure_no_capture() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_closure_no_capture.ori"),
        "fm_let_closure_no_capture",
    );
}

// T13: Closure (scalar capture)
#[test]
fn test_fm_let_closure_scalar_capture() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_closure_scalar_capture.ori"),
        "fm_let_closure_scalar_capture",
    );
}

// T14: Closure (fat capture — str)
#[test]
fn test_fm_let_closure_fat_capture() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_closure_fat_capture.ori"),
        "fm_let_closure_fat_capture",
    );
}

// Multiple bindings — several fat values in scope at once
#[test]
fn test_fm_let_multiple_fat_values() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_multiple_fat_values.ori"),
        "fm_let_multiple_fat_values",
    );
}

// Rebinding — fat value rebound should not leak
#[test]
fn test_fm_let_rebind_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f01_let_binding/fm_let_rebind_fat.ori"),
        "fm_let_rebind_fat",
    );
}
