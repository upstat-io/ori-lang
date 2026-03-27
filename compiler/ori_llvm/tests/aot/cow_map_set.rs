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
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let m2 = m.insert("c", 3);
    // Original must be unchanged, copy must have the new entry
    if m.len() == 2 && m2.len() == 3 then 0 else 1
}
"#,
        "cow_map_insert_shared_preserves_original",
    );
}

#[test]
fn test_cow_map_insert_shared_values_correct() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let m2 = m.insert("c", 3);
    let a_orig = m.get("a").unwrap();
    let b_orig = m.get("b").unwrap();
    let c_new = m2.get("c").unwrap();
    if a_orig == 1 && b_orig == 2 && c_new == 3 then 0 else 1
}
"#,
        "cow_map_insert_shared_values_correct",
    );
}

#[test]
fn test_cow_map_insert_overwrite_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let m2 = m.insert("a", 99);
    // Original key "a" must still be 1, copy must have 99
    let orig_val = m.get("a").unwrap();
    let new_val = m2.get("a").unwrap();
    if orig_val == 1 && new_val == 99 && m.len() == 2 && m2.len() == 2 then 0 else 1
}
"#,
        "cow_map_insert_overwrite_shared",
    );
}

// ─── Map COW: remove sharing ───

#[test]
fn test_cow_map_remove_shared_preserves_original() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    let m2 = m.remove("b");
    // Original must keep all 3, copy must have 2
    if m.len() == 3 && m2.len() == 2 then 0 else 1
}
"#,
        "cow_map_remove_shared_preserves_original",
    );
}

#[test]
fn test_cow_map_remove_shared_values_correct() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    let m2 = m.remove("b");
    // Original still has "b", copy does not
    let orig_has_b = m.contains_key("b");
    let copy_no_b = !m2.contains_key("b");
    let copy_has_a = m2.contains_key("a");
    let copy_has_c = m2.contains_key("c");
    if orig_has_b && copy_no_b && copy_has_a && copy_has_c then 0 else 1
}
"#,
        "cow_map_remove_shared_values_correct",
    );
}

#[test]
fn test_cow_map_remove_nonexistent_key() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let m2 = m.remove("z");
    // Removing nonexistent key should return same contents
    if m2.len() == 2 && m2.contains_key("a") && m2.contains_key("b") then 0 else 1
}
"#,
        "cow_map_remove_nonexistent_key",
    );
}

// ─── Map COW: unique owner fast path ───

#[test]
fn test_cow_map_insert_unique_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    // No sharing: m is consumed by insert, triggers fast path
    let m = {"a": 1}.insert("b", 2).insert("c", 3);
    if m.len() == 3 then 0 else 1
}
"#,
        "cow_map_insert_unique_no_leak",
    );
}

#[test]
fn test_cow_map_remove_unique_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3}.remove("b");
    if m.len() == 2 && !m.contains_key("b") then 0 else 1
}
"#,
        "cow_map_remove_unique_no_leak",
    );
}

// ─── Map COW: chained operations ───

#[test]
fn test_cow_map_chain_insert_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1}.insert("b", 2).insert("c", 3).remove("a");
    if m.len() == 2 && !m.contains_key("a") && m.get("b").unwrap() == 2 then 0 else 1
}
"#,
        "cow_map_chain_insert_remove",
    );
}

#[test]
fn test_cow_map_chain_overwrite_then_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2}.insert("a", 10).remove("b");
    if m.len() == 1 && m.get("a").unwrap() == 10 then 0 else 1
}
"#,
        "cow_map_chain_overwrite_then_remove",
    );
}

// ─── Map COW: int keys ───

#[test]
fn test_cow_map_int_keys_insert_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {1: "one", 2: "two"};
    let m2 = m.insert(3, "three");
    if m.len() == 2 && m2.len() == 3 then 0 else 1
}
"#,
        "cow_map_int_keys_insert_shared",
    );
}

#[test]
fn test_cow_map_int_keys_remove_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {10: "a", 20: "b", 30: "c"};
    let m2 = m.remove(20);
    if m.len() == 3 && m2.len() == 2 && !m2.contains_key(20) then 0 else 1
}
"#,
        "cow_map_int_keys_remove_shared",
    );
}

// ─── Map COW: edge cases ───

#[test]
fn test_cow_map_empty_insert() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {str: int} = {};
    let m2 = m.insert("first", 1);
    if m.is_empty() && m2.len() == 1 then 0 else 1
}
"#,
        "cow_map_empty_insert",
    );
}

#[test]
fn test_cow_map_single_entry_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"only": 42};
    let m2 = m.remove("only");
    if m.len() == 1 && m2.is_empty() then 0 else 1
}
"#,
        "cow_map_single_entry_remove",
    );
}

#[test]
fn test_cow_map_remove_all_entries() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let m2 = m.remove("a").remove("b");
    if m.len() == 2 && m2.is_empty() then 0 else 1
}
"#,
        "cow_map_remove_all_entries",
    );
}

// ─── Map COW: stress test ───

#[test]
fn test_cow_map_insert_loop_100() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: int} = {};
    for i in 0..100 do {
        m = m.insert(i, i * 10);
    };
    if m.len() == 100 then 0 else 1
}
"#,
        "cow_map_insert_loop_100",
    );
}

#[test]
fn test_cow_map_insert_loop_values_correct() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: int} = {};
    for i in 0..10 do {
        m = m.insert(i, i * 100);
    };
    let v0 = m.get(0).unwrap();
    let v5 = m.get(5).unwrap();
    let v9 = m.get(9).unwrap();
    if v0 == 0 && v5 == 500 && v9 == 900 then 0 else 1
}
"#,
        "cow_map_insert_loop_values_correct",
    );
}

// ─── Map COW: multiple sharing branches ───

#[test]
fn test_cow_map_multiple_forks() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base = {"a": 1, "b": 2};
    let fork1 = base.insert("c", 3);
    let fork2 = base.insert("d", 4);
    let fork3 = base.remove("a");
    // All forks diverge; base unchanged
    if base.len() == 2
        && fork1.len() == 3
        && fork2.len() == 3
        && fork3.len() == 1
        && fork1.contains_key("c")
        && fork2.contains_key("d")
        && !fork3.contains_key("a")
    then 0 else 1
}
"#,
        "cow_map_multiple_forks",
    );
}

// ─── Map COW: keys() and values() after COW ───

#[test]
fn test_cow_map_keys_after_insert() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1};
    let m2 = m.insert("b", 2);
    let ks = m2.keys();
    if ks.len() == 2 then 0 else 1
}
"#,
        "cow_map_keys_after_insert",
    );
}

#[test]
fn test_cow_map_values_after_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 10, "b": 20, "c": 30};
    let m2 = m.remove("b");
    let vs = m2.values();
    if vs.len() == 2 then 0 else 1
}
"#,
        "cow_map_values_after_remove",
    );
}

// ─── Map COW: iteration after mutation ───

#[test]
fn test_cow_map_iter_after_insert() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"x": 1}.insert("y", 2).insert("z", 3);
    let count = 0;
    for entry in m do {
        count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "cow_map_iter_after_insert",
    );
}

// ─── Set COW ───
// Set construction via `[...].iter().collect()` → `__collect_set` is now
// wired in the LLVM backend. These tests verify COW semantics for sets.

#[test]
fn test_cow_set_insert_shared_preserves_original() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    let s2 = s.insert(4);
    if s.len() == 3 && s2.len() == 4 then 0 else 1
}
"#,
        "cow_set_insert_shared_preserves_original",
    );
}

#[test]
fn test_cow_set_remove_shared_preserves_original() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    let s2 = s.remove(2);
    if s.len() == 3 && s2.len() == 2 then 0 else 1
}
"#,
        "cow_set_remove_shared_preserves_original",
    );
}

#[test]
fn test_cow_set_insert_duplicate_no_change() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    let s2 = s.insert(2);
    if s2.len() == 3 then 0 else 1
}
"#,
        "cow_set_insert_duplicate_no_change",
    );
}

#[test]
fn test_cow_set_union_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2].iter().collect();
    let b: Set<int> = [2, 3].iter().collect();
    let c = a.union(b);
    // a and b unchanged, c has union
    if a.len() == 2 && b.len() == 2 && c.len() == 3 then 0 else 1
}
"#,
        "cow_set_union_shared",
    );
}

#[test]
fn test_cow_set_intersection_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [2, 3, 4].iter().collect();
    let c = a.intersection(b);
    if a.len() == 3 && c.len() == 2 then 0 else 1
}
"#,
        "cow_set_intersection_shared",
    );
}

#[test]
fn test_cow_set_difference_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [2].iter().collect();
    let c = a.difference(b);
    if a.len() == 3 && c.len() == 2 then 0 else 1
}
"#,
        "cow_set_difference_shared",
    );
}

#[test]
fn test_cow_set_union_identity_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [0].iter().collect();
    let b2 = b.remove(0);
    let c = a.union(b2);
    if c.len() == 3 then 0 else 1
}
"#,
        "cow_set_union_identity_empty",
    );
}

#[test]
fn test_cow_set_intersection_disjoint() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2].iter().collect();
    let b: Set<int> = [3, 4].iter().collect();
    let c = a.intersection(b);
    if c.is_empty() then 0 else 1
}
"#,
        "cow_set_intersection_disjoint",
    );
}
