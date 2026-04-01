//! Map & Set COW (Copy-on-Write) AOT Integration Tests
//!
//! Verifies that map and set mutations follow value semantics in AOT:
//! - Unique owner: mutate in-place (fast path, O(1) amortized)
//! - Shared owner: copy-then-mutate (slow path, original unchanged)
//! - All paths are leak-free (`ORI_CHECK_LEAKS=1` enforced by harness)
//!
//! Set tests are currently ignored because Set construction in AOT is
//! blocked on `__collect_set` / `Set.new()` / `to_set()` not being
//! implemented in the LLVM backend. The COW runtime functions exist
//! (`ori_set_insert_cow`, etc.) but are untestable until Sets can be
//! created.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Map COW: insert sharing ───

#[test]
fn test_cow_map_insert_shared_preserves_original() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_insert_shared_preserves_original.ori"),
        "cow_map_insert_shared_preserves_original",
    );
}

#[test]
fn test_cow_map_insert_shared_values_correct() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_insert_shared_values_correct.ori"),
        "cow_map_insert_shared_values_correct",
    );
}

#[test]
fn test_cow_map_insert_overwrite_shared() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_insert_overwrite_shared.ori"),
        "cow_map_insert_overwrite_shared",
    );
}

// ─── Map COW: remove sharing ───

#[test]
fn test_cow_map_remove_shared_preserves_original() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_remove_shared_preserves_original.ori"),
        "cow_map_remove_shared_preserves_original",
    );
}

#[test]
fn test_cow_map_remove_shared_values_correct() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_remove_shared_values_correct.ori"),
        "cow_map_remove_shared_values_correct",
    );
}

#[test]
fn test_cow_map_remove_nonexistent_key() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_remove_nonexistent_key.ori"),
        "cow_map_remove_nonexistent_key",
    );
}

// ─── Map COW: unique owner fast path ───

#[test]
fn test_cow_map_insert_unique_no_leak() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_insert_unique_no_leak.ori"),
        "cow_map_insert_unique_no_leak",
    );
}

#[test]
fn test_cow_map_remove_unique_no_leak() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_remove_unique_no_leak.ori"),
        "cow_map_remove_unique_no_leak",
    );
}

// ─── Map COW: chained operations ───

#[test]
fn test_cow_map_chain_insert_remove() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_chain_insert_remove.ori"),
        "cow_map_chain_insert_remove",
    );
}

#[test]
fn test_cow_map_chain_overwrite_then_remove() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_chain_overwrite_then_remove.ori"),
        "cow_map_chain_overwrite_then_remove",
    );
}

// ─── Map COW: int keys ───

#[test]
fn test_cow_map_int_keys_insert_shared() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_int_keys_insert_shared.ori"),
        "cow_map_int_keys_insert_shared",
    );
}

#[test]
fn test_cow_map_int_keys_remove_shared() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_int_keys_remove_shared.ori"),
        "cow_map_int_keys_remove_shared",
    );
}

// ─── Map COW: edge cases ───

#[test]
fn test_cow_map_empty_insert() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_empty_insert.ori"),
        "cow_map_empty_insert",
    );
}

#[test]
fn test_cow_map_single_entry_remove() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_single_entry_remove.ori"),
        "cow_map_single_entry_remove",
    );
}

#[test]
fn test_cow_map_remove_all_entries() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_remove_all_entries.ori"),
        "cow_map_remove_all_entries",
    );
}

// ─── Map COW: stress test ───

#[test]
fn test_cow_map_insert_loop_100() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_insert_loop_100.ori"),
        "cow_map_insert_loop_100",
    );
}

#[test]
fn test_cow_map_insert_loop_values_correct() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_insert_loop_values_correct.ori"),
        "cow_map_insert_loop_values_correct",
    );
}

// ─── Map COW: multiple sharing branches ───

#[test]
fn test_cow_map_multiple_forks() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_multiple_forks.ori"),
        "cow_map_multiple_forks",
    );
}

// ─── Map COW: keys() and values() after COW ───

#[test]
fn test_cow_map_keys_after_insert() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_keys_after_insert.ori"),
        "cow_map_keys_after_insert",
    );
}

#[test]
fn test_cow_map_values_after_remove() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_values_after_remove.ori"),
        "cow_map_values_after_remove",
    );
}

// ─── Map COW: iteration after mutation ───

#[test]
fn test_cow_map_iter_after_insert() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_map_iter_after_insert.ori"),
        "cow_map_iter_after_insert",
    );
}

// ─── Set COW ───
// Set construction via `[...].iter().collect()` → `__collect_set` is now
// wired in the LLVM backend. These tests verify COW semantics for sets.

#[test]
fn test_cow_set_insert_shared_preserves_original() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_insert_shared_preserves_original.ori"),
        "cow_set_insert_shared_preserves_original",
    );
}

#[test]
fn test_cow_set_remove_shared_preserves_original() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_remove_shared_preserves_original.ori"),
        "cow_set_remove_shared_preserves_original",
    );
}

#[test]
fn test_cow_set_insert_duplicate_no_change() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_insert_duplicate_no_change.ori"),
        "cow_set_insert_duplicate_no_change",
    );
}

#[test]
fn test_cow_set_union_shared() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_union_shared.ori"),
        "cow_set_union_shared",
    );
}

#[test]
fn test_cow_set_intersection_shared() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_intersection_shared.ori"),
        "cow_set_intersection_shared",
    );
}

#[test]
fn test_cow_set_difference_shared() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_difference_shared.ori"),
        "cow_set_difference_shared",
    );
}

#[test]
fn test_cow_set_union_identity_empty() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_union_identity_empty.ori"),
        "cow_set_union_identity_empty",
    );
}

#[test]
fn test_cow_set_intersection_disjoint() {
    assert_aot_success(
        include_str!("fixtures/cow_map_set/cow_set_intersection_disjoint.ori"),
        "cow_set_intersection_disjoint",
    );
}
