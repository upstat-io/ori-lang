//! F14: List Element — fat pointer types stored as list elements.
//!
//! Tests element-level RC (inc on insertion, dec on removal/list drop),
//! correct `elem_dec_fn` generation, and interaction with iteration.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T5: [str] — list of heap strings
#[test]
fn test_fm_list_elem_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = ["abcdefghijklmnopqrstuvwxyz1234", "another_heap_string_here_now!"];
    if words.length() == 2 then 0 else 1
}
"#,
        "fm_list_elem_str",
    );
}

// T7: [[int]] — nested list
#[test]
fn test_fm_list_elem_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[1, 2], [3, 4, 5], [6]];
    if nested.length() == 3 then 0 else 1
}
"#,
        "fm_list_elem_nested",
    );
}

// T9: [Named] — list of structs with fat fields
#[test]
fn test_fm_list_elem_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let items = [Named { name: "alice", id: 1 }, Named { name: "bob", id: 2 }];
    if items.length() == 2 then 0 else 1
}
"#,
        "fm_list_elem_struct_fat",
    );
}

// T16: [Option<str>] — list of optional strings
#[test]
fn test_fm_list_elem_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [Option<str>] = [Some("hello"), None, Some("world")];
    if items.length() == 3 then 0 else 1
}
"#,
        "fm_list_elem_option_str",
    );
}

// Multiple list accesses — iterate twice (RC)
#[test]
fn test_fm_list_elem_str_two_iterations() {
    assert_aot_success(
        r#"
@sum_lengths (words: [str]) -> int = {
    let total = 0;
    for s in words do {
        total = total + s.length();
    };
    total
}

@main () -> int = {
    let words = ["hello", "world"];
    let a = sum_lengths(words: words);
    let b = sum_lengths(words: words);
    if a + b == 20 then 0 else 1
}
"#,
        "fm_list_elem_str_two_iterations",
    );
}

// List of heap strings iterated with yield
#[test]
fn test_fm_list_elem_str_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = ["hello", "world", "test"];
    let lengths = for s in words yield s.length();
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    if total == 14 then 0 else 1
}
"#,
        "fm_list_elem_str_yield",
    );
}

// T17: [Result<str, str>] — list of Result with heap string payloads (RC-02-001)
#[test]
fn test_fm_list_elem_result_str_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [Ok("success long heap string here"), Err("failure long heap string here")];
    if a.length() == 2 then 0 else 1
}
"#,
        "fm_list_elem_result_str_str",
    );
}

// T18: [Result<str, int>] — Ok variant has fat pointer, Err is scalar
#[test]
fn test_fm_list_elem_result_str_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [Ok("success long heap string here"), Err(42)];
    if a.length() == 2 then 0 else 1
}
"#,
        "fm_list_elem_result_str_int",
    );
}

// T19: [Result<int, str>] — Err variant has fat pointer, Ok is scalar
#[test]
fn test_fm_list_elem_result_int_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [Ok(42), Err("failure long heap string here")];
    if a.length() == 2 then 0 else 1
}
"#,
        "fm_list_elem_result_int_str",
    );
}

// T20: [Result<[int], str>] — Ok variant has list, Err has str
#[test]
fn test_fm_list_elem_result_list_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [Ok([1, 2, 3]), Err("failure long heap string here")];
    if a.length() == 2 then 0 else 1
}
"#,
        "fm_list_elem_result_list_str",
    );
}

// T21: Single-element [Result<str, str>] — just Ok with heap string
#[test]
fn test_fm_list_elem_result_single_ok() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: [Result<str, str>] = [Ok("success long heap string here")];
    if a.length() == 1 then 0 else 1
}
"#,
        "fm_list_elem_result_single_ok",
    );
}

// T22: Single-element [Result<str, str>] — just Err with heap string
#[test]
fn test_fm_list_elem_result_single_err() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: [Result<str, str>] = [Err("failure long heap string here")];
    if a.length() == 1 then 0 else 1
}
"#,
        "fm_list_elem_result_single_err",
    );
}
