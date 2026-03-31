//! F06: Pattern Matching — fat pointer types used in match expressions.
//!
//! Tests decision tree codegen, extractvalue for payloads, and RC handling
//! when fat pointer values are destructured or compared in patterns.

use crate::util::assert_aot_success;

// T11/T16: Match on Option<str> — Some vs None
#[test]
fn test_fm_match_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_option_str.ori"),
        "fm_match_option_str",
    );
}

// T15: Match on Option<int>
#[test]
fn test_fm_match_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_option_int.ori"),
        "fm_match_option_int",
    );
}

// T10: Match on unit-variant sum type (no fat payload)
#[test]
fn test_fm_match_unit_sum() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_unit_sum.ori"),
        "fm_match_unit_sum",
    );
}

// T11: Match on sum type with fat payload
#[test]
fn test_fm_match_fat_payload() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_fat_payload.ori"),
        "fm_match_fat_payload",
    );
}

// T9: Match destructuring struct with fat fields
#[test]
fn test_fm_match_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_struct_fat.ori"),
        "fm_match_struct_fat",
    );
}

// T18: Match destructuring tuple with fat element
#[test]
fn test_fm_match_tuple_mixed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_tuple_mixed.ori"),
        "fm_match_tuple_mixed",
    );
}

// Match with multiple arms using fat values (multi-field variant offset)
#[test]
fn test_fm_match_multi_arm_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_multi_arm_fat.ori"),
        "fm_match_multi_arm_fat",
    );
}

// Multi-field variant with fat first field, scalars after
// Semantic pin: would crash (misaligned pointer) without byte-offset GEP fix
#[test]
fn test_fm_match_multi_field_str_then_ints() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f06_pattern_matching/fm_match_multi_field_str_then_ints.ori"
        ),
        "fm_match_multi_str_then_ints",
    );
}

// Fat field in middle position
#[test]
fn test_fm_match_fat_field_middle() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_fat_field_middle.ori"),
        "fm_match_fat_middle",
    );
}

// Fat field in last position
#[test]
fn test_fm_match_fat_field_last() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_fat_field_last.ori"),
        "fm_match_fat_last",
    );
}

// Multiple fat fields in same variant
#[test]
fn test_fm_match_multiple_fat_fields() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f06_pattern_matching/fm_match_multiple_fat_fields.ori"
        ),
        "fm_match_multi_fat_fields",
    );
}

// str + int + str (fat-scalar-fat interleave)
#[test]
fn test_fm_match_fat_scalar_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_fat_scalar_fat.ori"),
        "fm_match_fat_scalar_fat",
    );
}

// Heap string (>23 bytes, non-SSO) in multi-field variant
#[test]
fn test_fm_match_heap_str_multi_field() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_matrix/f06_pattern_matching/fm_match_heap_str_multi_field.ori"
        ),
        "fm_match_heap_str_multi",
    );
}

// Nested match on fat values
#[test]
fn test_fm_match_nested_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f06_pattern_matching/fm_match_nested_option_str.ori"),
        "fm_match_nested_option_str",
    );
}
