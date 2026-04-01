//! F10: Generic Instantiation — fat pointer types used as generic type parameters.
//!
//! Tests monomorphization of generic functions when instantiated with fat pointer
//! types. The compiler must correctly specialize ABI, RC handling, and type layout
//! for each concrete instantiation.

use crate::util::assert_aot_success;

// T4: Generic identity with SSO string
#[test]
fn test_fm_generic_str_sso() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_str_sso.ori"),
        "fm_generic_str_sso",
    );
}

// T5: Generic identity with heap string
#[test]
fn test_fm_generic_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_str_heap.ori"),
        "fm_generic_str_heap",
    );
}

// T6: Generic with list of scalars
#[test]
fn test_fm_generic_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_list_scalar.ori"),
        "fm_generic_list_scalar",
    );
}

// T7: Generic with list of fat pointers
#[test]
fn test_fm_generic_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_list_fat.ori"),
        "fm_generic_list_fat",
    );
}

// T8: Generic with struct (scalar fields)
#[test]
fn test_fm_generic_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_struct_scalar.ori"),
        "fm_generic_struct_scalar",
    );
}

// T9: Generic with struct (fat fields)
#[test]
fn test_fm_generic_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_struct_fat.ori"),
        "fm_generic_struct_fat",
    );
}

// T15: Generic with Option<int>
#[test]
fn test_fm_generic_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_option_int.ori"),
        "fm_generic_option_int",
    );
}

// T17: Generic with map
#[test]
fn test_fm_generic_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_map.ori"),
        "fm_generic_map",
    );
}

// Generic function called with multiple fat types (monomorphized separately)
#[test]
fn test_fm_generic_multi_instantiation() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_multi_instantiation.ori"),
        "fm_generic_multi_instantiation",
    );
}

// Generic function with fat constraint (uses length)
#[test]
fn test_fm_generic_with_operation() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f10_generics/fm_generic_with_operation.ori"),
        "fm_generic_with_operation",
    );
}
