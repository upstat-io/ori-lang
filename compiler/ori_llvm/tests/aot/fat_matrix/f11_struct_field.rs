//! F11: Struct Field — fat pointer types stored in struct fields.
//!
//! Tests GEP-based field access, aggregate construction, and RC handling
//! when fat pointer values are stored in and extracted from struct fields.

use crate::util::assert_aot_success;

// T4/T5: Struct with str field
#[test]
fn test_fm_struct_field_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_str.ori"),
        "fm_struct_field_str",
    );
}

// T5: Struct with heap str field
#[test]
fn test_fm_struct_field_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_str_heap.ori"),
        "fm_struct_field_str_heap",
    );
}

// T6: Struct with list of scalars field
#[test]
fn test_fm_struct_field_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_list_scalar.ori"),
        "fm_struct_field_list_scalar",
    );
}

// T7: Struct with list of fat pointers field
#[test]
fn test_fm_struct_field_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_list_fat.ori"),
        "fm_struct_field_list_fat",
    );
}

// T9: Nested struct with fat fields
#[test]
fn test_fm_struct_field_nested_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_nested_fat.ori"),
        "fm_struct_field_nested_fat",
    );
}

// Multiple fat fields in same struct
#[test]
fn test_fm_struct_field_multi_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_multi_fat.ori"),
        "fm_struct_field_multi_fat",
    );
}

// Struct field passed to function
#[test]
fn test_fm_struct_field_passed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_passed.ori"),
        "fm_struct_field_passed",
    );
}

// Struct with map field
#[test]
fn test_fm_struct_field_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f11_struct_field/fm_struct_field_map.ori"),
        "fm_struct_field_map",
    );
}
