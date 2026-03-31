//! F16: Recursion — fat pointer types passed through recursive function calls.
//!
//! Tests stack frame RC handling, ensuring that fat pointer values are correctly
//! incremented/decremented across recursive call boundaries.

use crate::util::assert_aot_success;

// Recursive countdown with str in scope
#[test]
fn test_fm_recursion_str_in_scope() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f16_recursion/fm_recursion_str_in_scope.ori"),
        "fm_recursion_str_in_scope",
    );
}

// Recursive function with str parameter
#[test]
fn test_fm_recursion_str_param() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f16_recursion/fm_recursion_str_param.ori"),
        "fm_recursion_str_param",
    );
}

// Recursive function with list parameter
#[test]
fn test_fm_recursion_list_param() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f16_recursion/fm_recursion_list_param.ori"),
        "fm_recursion_list_param",
    );
}

// Recursive function returning struct with fat field
#[test]
fn test_fm_recursion_struct_fat_return() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f16_recursion/fm_recursion_struct_fat_return.ori"),
        "fm_recursion_struct_fat_return",
    );
}

// Recursive function with Option<int> return
#[test]
fn test_fm_recursion_option_return() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f16_recursion/fm_recursion_option_return.ori"),
        "fm_recursion_option_return",
    );
}

// Mutual recursion with fat values
#[test]
fn test_fm_recursion_mutual_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f16_recursion/fm_recursion_mutual_fat.ori"),
        "fm_recursion_mutual_fat",
    );
}
