//! F12: Sum Type Payload — fat pointer types as enum variant payloads.
//!
//! Tests tag + payload layout, extractvalue for payloads, and RC handling
//! when fat pointer values are wrapped in sum type variants.

use crate::util::assert_aot_success;

// T4/T5: Sum type with str payload
#[test]
fn test_fm_sum_str_payload() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_str_payload.ori"),
        "fm_sum_str_payload",
    );
}

// T5: Sum type with heap str payload
#[test]
fn test_fm_sum_str_heap_payload() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_str_heap_payload.ori"),
        "fm_sum_str_heap_payload",
    );
}

// T6: Sum type with list of scalars payload
#[test]
fn test_fm_sum_list_scalar_payload() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_list_scalar_payload.ori"),
        "fm_sum_list_scalar_payload",
    );
}

// T7: Sum type with list of fat pointers payload
#[test]
fn test_fm_sum_list_fat_payload() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_list_fat_payload.ori"),
        "fm_sum_list_fat_payload",
    );
}

// T9: Sum type with struct (fat fields) payload
#[test]
fn test_fm_sum_struct_fat_payload() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_struct_fat_payload.ori"),
        "fm_sum_struct_fat_payload",
    );
}

// Sum type with multiple fat-payload variants
#[test]
fn test_fm_sum_multi_fat_variant() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_multi_fat_variant.ori"),
        "fm_sum_multi_fat_variant",
    );
}

// None variant matching (Option<str> with no payload)
#[test]
fn test_fm_sum_none_variant() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_none_variant.ori"),
        "fm_sum_none_variant",
    );
}

// Sum type payload passed to function
#[test]
fn test_fm_sum_payload_passed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f12_sum_payload/fm_sum_payload_passed.ori"),
        "fm_sum_payload_passed",
    );
}
