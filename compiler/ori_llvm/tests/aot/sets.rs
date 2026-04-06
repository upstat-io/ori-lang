//! Set Method AOT Tests
//!
//! Tests for Set<T> method coverage in AOT compilation.
//! Set construction via `[...].iter().collect()` → `__collect_set` is
//! wired in the LLVM backend.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_aot_set_length() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_length.ori"),
        "set_length",
    );
}

#[test]
fn test_aot_set_is_empty() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_is_empty.ori"),
        "set_is_empty",
    );
}

#[test]
fn test_aot_set_contains() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_contains.ori"),
        "set_contains",
    );
}

#[test]
fn test_aot_set_insert() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_insert.ori"),
        "set_insert",
    );
}

#[test]
fn test_aot_set_remove() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_remove.ori"),
        "set_remove",
    );
}

#[test]
fn test_aot_set_union() {
    assert_aot_success(include_str!("fixtures/sets/aot_set_union.ori"), "set_union");
}

#[test]
fn test_aot_set_intersection() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_intersection.ori"),
        "set_intersection",
    );
}

#[test]
fn test_aot_set_difference() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_difference.ori"),
        "set_difference",
    );
}

#[test]
fn test_aot_set_to_list() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_to_list.ori"),
        "set_to_list",
    );
}

#[test]
fn test_aot_set_iter_count() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_iter_count.ori"),
        "set_iter_count",
    );
}

// Auto-promoted set iterator methods (no explicit .iter())
// Semantic pins for TPR-03-001: emit_auto_iter must route Set through emit_set_iter

#[test]
#[ignore = "codegen gap: set fold hits unresolved type variable (pre-existing)"]
fn test_aot_set_auto_fold() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_auto_fold.ori"),
        "set_auto_fold",
    );
}

#[test]
fn test_aot_set_auto_count() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_auto_count.ori"),
        "set_auto_count",
    );
}

#[test]
fn test_aot_set_auto_any() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_auto_any.ori"),
        "set_auto_any",
    );
}

#[test]
fn test_aot_set_auto_all() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_auto_all.ori"),
        "set_auto_all",
    );
}

#[test]
fn test_aot_set_str_auto_fold() {
    assert_aot_success(
        include_str!("fixtures/sets/aot_set_str_auto_fold.ori"),
        "set_str_auto_fold",
    );
}
