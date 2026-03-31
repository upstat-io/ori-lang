//! Generalize tests — verify `elem_dec_fn` works for all `[T]` where T has Drop.
//!
//! Covers: [str], [[int]], closures, structs with str fields, Option<str>,
//! string iteration, and multi-call patterns.

use crate::util::assert_aot_success;

#[test]
fn test_generalize_str_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_str_list.ori"),
        "generalize_str_list",
    );
}

/// [[int]] iteration — nested list elements are heap-allocated.
#[test]
fn test_generalize_nested_int_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_nested_int_list.ori"),
        "generalize_nested_int_list",
    );
}

/// [(int) -> int] iteration — closures that capture heap values.
#[test]
fn test_generalize_closure_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_closure_list.ori"),
        "generalize_closure_list",
    );
}

/// [{name: str, age: int}] iteration — structs with string fields.
#[test]
fn test_generalize_struct_with_str_fields() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_struct_with_str_fields.ori"),
        "generalize_struct_with_str_fields",
    );
}

/// [Option<str>] iteration — sum types with fat pointer payloads.
#[test]
fn test_generalize_option_str_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_option_str_list.ori"),
        "generalize_option_str_list",
    );
}

/// Partially consumed [str] with break — consumed and unconsumed elements
/// must both be correctly cleaned up.
#[test]
fn test_generalize_partial_break_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_partial_break_str.ori"),
        "generalize_partial_break_str",
    );
}

/// `for w in words yield w.len()` — yield consumes each element value.
#[test]
fn test_generalize_yield_str_lengths() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_yield_str_lengths.ori"),
        "generalize_yield_str_lengths",
    );
}

/// [str] passed to TWO functions — verifies list RC increment on second
/// call preserves elements for both iteration passes.
#[test]
fn test_generalize_str_list_two_calls() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_str_list_two_calls.ori"),
        "generalize_str_list_two_calls",
    );
}

/// String iteration — `for c in s` where `s: str`. `IterState::Str` owns
/// its data via `owns_data` flag.
#[test]
fn test_generalize_string_iteration() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/generalize/generalize_string_iteration.ori"),
        "generalize_string_iteration",
    );
}
