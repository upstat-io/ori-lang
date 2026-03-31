//! Struct/sum/tuple iteration tests — elements with Drop fields.

use crate::util::assert_aot_success;

// Struct with string fields

#[test]
fn test_struct_with_str_field_iteration() {
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/struct_sum_tuple/struct_with_str_field_iteration.ori"
        ),
        "struct_with_str_field_iteration",
    );
}

// T6: [Result<str, str>] — fat pointer in both Ok and Err variants

#[test]
fn test_result_str_list_iteration() {
    // Both Ok and Err payloads are heap strings needing elem_dec_fn cleanup.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/result_str_list_iteration.ori"),
        "result_str_list_iteration",
    );
}

// T7: [(str, int)] — tuple with a string component

#[test]
fn test_tuple_str_list_iteration() {
    // Tuple elements with a fat pointer component need elem_dec_fn for the str field.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/tuple_str_list_iteration.ori"),
        "tuple_str_list_iteration",
    );
}

// F2: Break — partial iteration with un-consumed element cleanup

#[test]
fn test_option_str_list_break() {
    // T5-F2: Option<str> partial iteration — un-consumed Some(str) must be cleaned up.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/option_str_list_break.ori"),
        "option_str_list_break",
    );
}

#[test]
fn test_result_str_list_break() {
    // T6-F2: Result<str, str> partial iteration — un-consumed variants cleaned up.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/result_str_list_break.ori"),
        "result_str_list_break",
    );
}

#[test]
fn test_tuple_str_list_break() {
    // T7-F2: (str, int) partial iteration — un-consumed tuple elements cleaned up.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/tuple_str_list_break.ori"),
        "tuple_str_list_break",
    );
}

// F5: Function parameter — pass collection to function, call twice

#[test]
fn test_option_str_list_two_calls() {
    // T5-F5: [Option<str>] passed to function twice.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/option_str_list_two_calls.ori"),
        "option_str_list_two_calls",
    );
}

#[test]
fn test_result_str_list_two_calls() {
    // T6-F5: [Result<str, str>] passed to function twice.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/result_str_list_two_calls.ori"),
        "result_str_list_two_calls",
    );
}

#[test]
fn test_tuple_str_list_two_calls() {
    // T7-F5: [(str, int)] passed to function twice.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/struct_sum_tuple/tuple_str_list_two_calls.ori"),
        "tuple_str_list_two_calls",
    );
}
