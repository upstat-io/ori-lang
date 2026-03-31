//! F13: Derived Eq — equality comparison on types containing fat pointer fields.
//!
//! Tests the `$eq` derived method codegen for structs and sum types whose fields
//! include strings, lists, and other fat pointer types.

use crate::util::assert_aot_success;

// T4/T5: Eq on struct with str field
#[test]
fn test_fm_eq_struct_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_struct_str.ori"),
        "fm_eq_struct_str",
    );
}

// T6: Eq on struct with list of scalars
#[test]
fn test_fm_eq_struct_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_struct_list_scalar.ori"),
        "fm_eq_struct_list_scalar",
    );
}

// T7: Eq on struct with list of strings
#[test]
fn test_fm_eq_struct_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_struct_list_fat.ori"),
        "fm_eq_struct_list_fat",
    );
}

// T9: Eq on struct with nested fat fields
#[test]
fn test_fm_eq_nested_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_nested_fat.ori"),
        "fm_eq_nested_fat",
    );
}

// Sum type: Eq on Option<str>
#[test]
fn test_fm_eq_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_option_str.ori"),
        "fm_eq_option_str",
    );
}

// Direct str comparison
#[test]
fn test_fm_eq_str_direct() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_str_direct.ori"),
        "fm_eq_str_direct",
    );
}

// Eq on struct with multiple fat fields
#[test]
fn test_fm_eq_multi_fat_fields() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_multi_fat_fields.ori"),
        "fm_eq_multi_fat_fields",
    );
}

// Eq with heap strings (>23 bytes)
#[test]
fn test_fm_eq_heap_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_heap_str.ori"),
        "fm_eq_heap_str",
    );
}

// Derived Eq on struct with [str] using heap-backed strings
// Regression: ori_list_eq_scalar did byte-level memcmp which fails for heap
// strings (different data pointers but identical content).
#[test]
fn test_fm_eq_list_heap_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_heap_str.ori"),
        "fm_eq_list_heap_str",
    );
}

// Multiple heap strings in a list
#[test]
fn test_fm_eq_list_multiple_heap_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_multiple_heap_str.ori"),
        "fm_eq_list_multi_heap_str",
    );
}

// Empty list equality
#[test]
fn test_fm_eq_list_empty() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_empty.ori"),
        "fm_eq_list_empty",
    );
}

// Mixed SSO and heap strings in a list
#[test]
fn test_fm_eq_list_mixed_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_mixed_str.ori"),
        "fm_eq_list_mixed_str",
    );
}

// Map equality with composite value type {str: [int]}
#[test]
fn test_fm_eq_map_composite_list_val() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_map_composite_list_val.ori"),
        "fm_eq_map_composite_list_val",
    );
}

// Map equality with str value type (non-primitive)
#[test]
fn test_fm_eq_map_str_val() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_map_str_val.ori"),
        "fm_eq_map_str_val",
    );
}

// Map equality — str keys with int values (base case, should still work)
#[test]
fn test_fm_eq_map_primitive_val() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_map_primitive_val.ori"),
        "fm_eq_map_primitive_val",
    );
}

// Derived Eq on struct with [Option<str>] — wrapper elements
// require deep comparison but were missed by needs_deep_comparison().
#[test]
fn test_fm_eq_list_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_option_str.ori"),
        "fm_eq_list_option_str",
    );
}

// Derived Eq on struct with [Option<str>] — both None
#[test]
fn test_fm_eq_list_option_str_both_none() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_option_str_both_none.ori"),
        "fm_eq_list_option_none",
    );
}

// Derived Eq on struct with {str: Option<str>} — wrapper map values
#[test]
fn test_fm_eq_map_option_str_val() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_map_option_str_val.ori"),
        "fm_eq_map_option_str_val",
    );
}

// Derived Eq with Option field directly (not nested in list/map)
#[test]
fn test_fm_eq_option_field_direct() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_option_field_direct.ori"),
        "fm_eq_option_field_direct",
    );
}

// Derived Eq with Result<str, str> in list
// Uses SSO-length strings to avoid [Result<str,str>] RC leak
// (heap strings in Result list elements leak — tracked in rc-integrity plan).
#[test]
fn test_fm_eq_list_result_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_result_str.ori"),
        "fm_eq_list_result_str",
    );
}

// Derived Eq with (int, str) tuple in list
#[test]
fn test_fm_eq_list_tuple() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_tuple.ori"),
        "fm_eq_list_tuple",
    );
}

// Direct [Named] list equality where Named has fat fields
#[test]
fn test_fm_eq_list_fat_struct() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_fat_struct.ori"),
        "fm_eq_list_fat_struct",
    );
}

// Struct with [Named] field (compute_elem_size must use LLVM layout)
#[test]
fn test_fm_eq_struct_with_list_of_fat_struct() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f13_derived_eq/fm_eq_struct_with_list_of_fat_struct.ori"
        ),
        "fm_eq_struct_with_list_of_fat_struct",
    );
}

// Equality on empty [Named] list (edge case)
#[test]
fn test_fm_eq_list_fat_struct_empty() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_list_fat_struct_empty.ori"),
        "fm_eq_list_fat_struct_empty",
    );
}

// Map with fat-struct values
#[test]
fn test_fm_eq_map_fat_struct_value() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f13_derived_eq/fm_eq_map_fat_struct_value.ori"),
        "fm_eq_map_fat_struct_value",
    );
}
