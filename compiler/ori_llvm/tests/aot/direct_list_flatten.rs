//! Direct `List.flatten()` AOT tests.
//!
//! `List.flatten()` called directly (no `.iter()` prefix) has no codegen
//! dispatch arm; the master dispatcher falls through to auto-iter promotion,
//! which mis-shapes the receiver for the Iterator-only `emit_iter_flatten`
//! emitter and aborts. These tests pin the direct-list-method call path
//! across nested, triple-nested, and non-nested receiver shapes, independent
//! of the sibling typeck `ReturnTag` fix — each fixture uses an
//! unannotated call + a `.length()`-based assertion, so no element-type
//! comparison is involved.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// Positive pins: L8 AOT direct-list-flatten builds + runs across all three
// shapes. Each fails pre-fix (the `flatten_inner_elem_size` assert aborts
// the build — `assert_aot_success` surfaces that as a compile-failure
// panic) and passes post-fix.

#[test]
fn test_direct_list_flatten_nested() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_nested.ori"),
        "direct_list_flatten_nested",
    );
}

#[test]
fn test_direct_list_flatten_triple_nested() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_triple_nested.ori"),
        "direct_list_flatten_triple_nested",
    );
}

#[test]
fn test_direct_list_flatten_non_nested() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_non_nested.ori"),
        "direct_list_flatten_non_nested",
    );
}

// L10 leak pins: `compile_and_run_capture` always enables `ORI_CHECK_LEAKS=1`
// on the run step; `assert_aot_success` already treats exit `2` as the
// leak-detected failure mode, so the positive pins above ARE the leak pins.
// Named separately per the TDD matrix for traceability to layer-coverage.md L10.

#[test]
fn test_direct_list_flatten_leak() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_nested.ori"),
        "direct_list_flatten_leak",
    );
}

#[test]
fn test_direct_list_flatten_triple_nested_leak() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_triple_nested.ori"),
        "direct_list_flatten_triple_nested_leak",
    );
}

#[test]
fn test_direct_list_flatten_non_nested_leak() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_non_nested.ori"),
        "direct_list_flatten_non_nested_leak",
    );
}

// Additional shapes: a for-yield-constructed receiver (not a list literal)
// and a heap-typed element, exercising RC discipline beyond scalar `int`.

#[test]
fn test_direct_list_flatten_for_yield_source() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_for_yield_source.ori"),
        "direct_list_flatten_for_yield_source",
    );
}

#[test]
fn test_direct_list_flatten_heap_element() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_heap_element.ori"),
        "direct_list_flatten_heap_element",
    );
}

// Edge-case pins: an empty inner list and uneven inner-list lengths, both
// exercising the two-pass sum-then-copy primitive's per-element length
// accumulation beyond the uniform-length shapes above.

#[test]
fn test_direct_list_flatten_empty_inner() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_empty_inner.ori"),
        "direct_list_flatten_empty_inner",
    );
}

#[test]
fn test_direct_list_flatten_uneven_inner() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_flatten_uneven_inner.ori"),
        "direct_list_flatten_uneven_inner",
    );
}
