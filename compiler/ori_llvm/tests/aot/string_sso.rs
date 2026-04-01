//! SSO (Small String Optimization) AOT Tests
//!
//! Tests that specifically exercise the SSO boundary (23 bytes) and ensure
//! correct behavior for both inline (SSO) and heap-allocated strings through
//! the AOT pipeline. Covers:
//! - SSO strings (≤23 bytes inline, no heap)
//! - Heap strings (>23 bytes, RC-managed)
//! - Boundary transitions (concat crossing 23-byte threshold)
//! - RC operations (clone, drop, sharing) on both representations
//! - String methods dispatching correctly for both representations

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── SSO: basic inline strings ───

#[test]
fn test_sso_empty_string() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_empty_string.ori"),
        "sso_empty",
    );
}

#[test]
fn test_sso_short_string() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_short_string.ori"),
        "sso_short",
    );
}

#[test]
fn test_sso_max_length_23_bytes() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_max_length_23_bytes.ori"),
        "sso_max_23",
    );
}

#[test]
fn test_sso_max_equality() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_max_equality.ori"),
        "sso_max_eq",
    );
}

// ─── Heap: strings exceeding SSO ───

#[test]
fn test_heap_24_bytes() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_24_bytes.ori"),
        "heap_24",
    );
}

#[test]
fn test_heap_long_string() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_long_string.ori"),
        "heap_long",
    );
}

#[test]
fn test_heap_equality() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_equality.ori"),
        "heap_eq",
    );
}

#[test]
fn test_heap_inequality() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_inequality.ori"),
        "heap_neq",
    );
}

// ─── SSO ↔ heap transitions via concatenation ───

#[test]
fn test_sso_concat_stays_sso() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_concat_stays_sso.ori"),
        "sso_concat_stays",
    );
}

#[test]
fn test_sso_concat_to_max_sso() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_concat_to_max_sso.ori"),
        "sso_concat_max",
    );
}

#[test]
fn test_sso_concat_crosses_to_heap() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_concat_crosses_to_heap.ori"),
        "sso_concat_to_heap",
    );
}

#[test]
fn test_sso_chain_concat_to_heap() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_chain_concat_to_heap.ori"),
        "sso_chain_to_heap",
    );
}

#[test]
fn test_heap_concat_heap() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_concat_heap.ori"),
        "heap_concat_heap",
    );
}

// ─── RC operations: clone/drop on both representations ───

#[test]
fn test_sso_clone() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_clone.ori"),
        "sso_clone",
    );
}

#[test]
fn test_heap_clone() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_clone.ori"),
        "heap_clone",
    );
}

#[test]
fn test_sso_clone_independence() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_clone_independence.ori"),
        "sso_clone_indep",
    );
}

#[test]
fn test_heap_clone_independence() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_clone_independence.ori"),
        "heap_clone_indep",
    );
}

#[test]
fn test_sso_multiple_references() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_multiple_references.ori"),
        "sso_multi_ref",
    );
}

#[test]
fn test_heap_multiple_references() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_multiple_references.ori"),
        "heap_multi_ref",
    );
}

// ─── String methods on SSO vs heap ───

#[test]
fn test_sso_contains() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_contains.ori"),
        "sso_contains",
    );
}

#[test]
fn test_heap_contains() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_contains.ori"),
        "heap_contains",
    );
}

#[test]
fn test_sso_starts_ends_with() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_starts_ends_with.ori"),
        "sso_starts_ends",
    );
}

#[test]
fn test_heap_starts_ends_with() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_starts_ends_with.ori"),
        "heap_starts_ends",
    );
}

#[test]
fn test_sso_comparison() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_comparison.ori"),
        "sso_compare",
    );
}

#[test]
fn test_heap_comparison() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_comparison.ori"),
        "heap_compare",
    );
}

#[test]
fn test_sso_vs_heap_comparison() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_vs_heap_comparison.ori"),
        "sso_vs_heap_cmp",
    );
}

// ─── Strings in data structures ───

#[test]
fn test_sso_in_list() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_in_list.ori"),
        "sso_in_list",
    );
}

#[test]
fn test_heap_in_list() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_in_list.ori"),
        "heap_in_list",
    );
}

#[test]
fn test_mixed_sso_heap_in_list() {
    assert_aot_success(
        include_str!("fixtures/string_sso/mixed_sso_heap_in_list.ori"),
        "mixed_sso_heap_list",
    );
}

#[test]
fn test_sso_in_struct() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_in_struct.ori"),
        "sso_in_struct",
    );
}

#[test]
fn test_heap_in_struct() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_in_struct.ori"),
        "heap_in_struct",
    );
}

#[test]
fn test_sso_in_tuple() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_in_tuple.ori"),
        "sso_in_tuple",
    );
}

// ─── String iteration on SSO vs heap ───

#[test]
fn test_sso_iteration() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_iteration.ori"),
        "sso_iter",
    );
}

#[test]
fn test_heap_iteration() {
    assert_aot_success(
        include_str!("fixtures/string_sso/heap_iteration.ori"),
        "heap_iter",
    );
}

// ─── to_str conversions producing SSO vs heap ───

#[test]
fn test_int_to_str_sso() {
    assert_aot_success(
        include_str!("fixtures/string_sso/int_to_str_sso.ori"),
        "int_to_str_sso",
    );
}

#[test]
fn test_bool_to_str_sso() {
    assert_aot_success(
        include_str!("fixtures/string_sso/bool_to_str_sso.ori"),
        "bool_to_str_sso",
    );
}

// ─── format strings with SSO and heap ───

#[test]
fn test_format_sso_result() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let s = `{x}`;
    if s == "42" then 0 else 1
}
"#,
        "fmt_sso_result",
    );
}

#[test]
fn test_format_heap_result() {
    assert_aot_success(
        r#"
@main () -> int = {
    let name = "world";
    let s = `hello {name}, this greeting is going to be quite long indeed!`;
    if s.length() > 23 then 0 else 1
}
"#,
        "fmt_heap_result",
    );
}

// ─── SSO boundary stress: repeated concat ───

#[test]
fn test_sso_repeated_concat_loop() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_repeated_concat_loop.ori"),
        "sso_repeated_concat",
    );
}

#[test]
fn test_sso_boundary_exact_transition() {
    assert_aot_success(
        include_str!("fixtures/string_sso/sso_boundary_exact_transition.ori"),
        "sso_boundary_exact",
    );
}

// ─── Catch/error handling with SSO strings ───

#[test]
fn test_catch_returns_sso_string() {
    assert_aot_success(
        include_str!("fixtures/string_sso/catch_returns_sso_string.ori"),
        "catch_sso_str",
    );
}

#[test]
fn test_catch_returns_heap_string() {
    assert_aot_success(
        include_str!("fixtures/string_sso/catch_returns_heap_string.ori"),
        "catch_heap_str",
    );
}

// Immortal empty string: codegen calls ori_str_empty() (no heap, no RC).

#[test]
fn test_immortal_empty_string_no_leak() {
    assert_aot_success(
        include_str!("fixtures/string_sso/immortal_empty_string_no_leak.ori"),
        "immortal_empty_str_no_leak",
    );
}

#[test]
fn test_immortal_empty_string_passed_to_function() {
    assert_aot_success(
        include_str!("fixtures/string_sso/immortal_empty_string_passed_to_function.ori"),
        "immortal_empty_str_passed",
    );
}

#[test]
fn test_immortal_empty_string_in_collection() {
    assert_aot_success(
        include_str!("fixtures/string_sso/immortal_empty_string_in_collection.ori"),
        "immortal_empty_str_collection",
    );
}
