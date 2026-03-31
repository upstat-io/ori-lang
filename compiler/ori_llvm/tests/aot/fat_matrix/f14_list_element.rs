//! F14: List Element — fat pointer types stored as list elements.
//!
//! Tests element-level RC (inc on insertion, dec on removal/list drop),
//! correct `elem_dec_fn` generation, and interaction with iteration.

use crate::util::assert_aot_success;

// T5: [str] — list of heap strings
#[test]
fn test_fm_list_elem_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_str.ori"),
        "fm_list_elem_str",
    );
}

// T7: [[int]] — nested list
#[test]
fn test_fm_list_elem_nested() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_nested.ori"),
        "fm_list_elem_nested",
    );
}

// T9: [Named] — list of structs with fat fields
#[test]
fn test_fm_list_elem_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_struct_fat.ori"),
        "fm_list_elem_struct_fat",
    );
}

// T16: [Option<str>] — list of optional strings
#[test]
fn test_fm_list_elem_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_option_str.ori"),
        "fm_list_elem_option_str",
    );
}

// Multiple list accesses — iterate twice (RC)
#[test]
fn test_fm_list_elem_str_two_iterations() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_str_two_iterations.ori"),
        "fm_list_elem_str_two_iterations",
    );
}

// List of heap strings iterated with yield
#[test]
fn test_fm_list_elem_str_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_str_yield.ori"),
        "fm_list_elem_str_yield",
    );
}

// T17: [Result<str, str>] — list of Result with heap string payloads (RC-02-001)
#[test]
fn test_fm_list_elem_result_str_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_result_str_str.ori"),
        "fm_list_elem_result_str_str",
    );
}

// T18: [Result<str, int>] — Ok variant has fat pointer, Err is scalar
#[test]
fn test_fm_list_elem_result_str_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_result_str_int.ori"),
        "fm_list_elem_result_str_int",
    );
}

// T19: [Result<int, str>] — Err variant has fat pointer, Ok is scalar
#[test]
fn test_fm_list_elem_result_int_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_result_int_str.ori"),
        "fm_list_elem_result_int_str",
    );
}

// T20: [Result<[int], str>] — Ok variant has list, Err has str
#[test]
fn test_fm_list_elem_result_list_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_result_list_str.ori"),
        "fm_list_elem_result_list_str",
    );
}

// T21: Single-element [Result<str, str>] — just Ok with heap string
#[test]
fn test_fm_list_elem_result_single_ok() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_result_single_ok.ori"),
        "fm_list_elem_result_single_ok",
    );
}

// T22: Single-element [Result<str, str>] — just Err with heap string
#[test]
fn test_fm_list_elem_result_single_err() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f14_list_element/fm_list_elem_result_single_err.ori"),
        "fm_list_elem_result_single_err",
    );
}
