//! Borrow-class independence matrix tests.
//!
//! Borrow-class cells from the SSA-alias dec-dedup TDD
//! matrix. Project field borrows refer to different RC slots than their
//! source root; independent `RcDecs` MUST continue to fire under the SSA
//! alias-class union-find fix. These cells PASS clean both pre-fix and
//! post-fix — they verify the fix preserves borrow exclusion (does not
//! over-deduplicate decs across distinct RC slots).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_borrow_disjoint_fields() {
    assert_aot_success(
        include_str!("fixtures/borrow_independence/borrow_disjoint_fields.ori"),
        "borrow_disjoint_fields",
    );
}

#[test]
fn test_borrow_nested_project() {
    assert_aot_success(
        include_str!("fixtures/borrow_independence/borrow_nested_project.ori"),
        "borrow_nested_project",
    );
}

// Borrow-class cells — additional borrow shapes that must NOT dedup decs

#[test]
fn test_borrow_then_let_alias() {
    assert_aot_success(
        include_str!("fixtures/borrow_independence/borrow_then_let_alias.ori"),
        "borrow_then_let_alias",
    );
}

#[test]
fn test_borrow_separate_heap_types() {
    assert_aot_success(
        include_str!("fixtures/borrow_independence/borrow_separate_heap_types.ori"),
        "borrow_separate_heap_types",
    );
}

#[test]
fn test_struct_owned_borrowed_mixed() {
    assert_aot_success(
        include_str!("fixtures/borrow_independence/struct_owned_borrowed_mixed.ori"),
        "struct_owned_borrowed_mixed",
    );
}
