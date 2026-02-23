//! Collection Method Extension AOT Tests
//!
//! Tests for List, Map, and Set method coverage beyond what exists in spec.rs
//! and `for_loops.rs`. Focuses on method calls (length, `is_empty`, iter, clone)
//! and gap inventory for methods not yet in the AOT builtin table.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── List: length ───

#[test]
fn test_coll_list_length_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    if xs.length() == 0 then 0 else 1
}
"#,
        "coll_list_len_empty",
    );
}

#[test]
fn test_coll_list_length_one() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [42];
    if xs.length() == 1 then 0 else 1
}
"#,
        "coll_list_len_one",
    );
}

#[test]
fn test_coll_list_length_many() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    if xs.length() == 10 then 0 else 1
}
"#,
        "coll_list_len_many",
    );
}

#[test]
fn test_coll_list_len_alias() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    if xs.len() == 3 then 0 else 1
}
"#,
        "coll_list_len_alias",
    );
}

// ─── List: is_empty ───

#[test]
fn test_coll_list_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    if xs.is_empty() then 0 else 1
}
"#,
        "coll_list_is_empty_true",
    );
}

#[test]
fn test_coll_list_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1];
    if !xs.is_empty() then 0 else 1
}
"#,
        "coll_list_is_empty_false",
    );
}

// ─── List: iter & collect ───

#[test]
fn test_coll_list_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let count = xs.iter().count();
    if count == 3 then 0 else 1
}
"#,
        "coll_list_iter_count",
    );
}

#[test]
fn test_coll_list_iter_sum_via_fold() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let sum = xs.iter().fold(0, (acc: int, x: int) -> acc + x);
    if sum == 15 then 0 else 1
}
"#,
        "coll_list_iter_fold",
    );
}

#[test]
fn test_coll_list_iter_filter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5, 6];
    let evens = xs.iter().filter(x -> x % 2 == 0).count();
    if evens == 3 then 0 else 1
}
"#,
        "coll_list_iter_filter",
    );
}

#[test]
fn test_coll_list_iter_map_collect_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let doubled = xs.iter().map(x -> x * 2).collect();
    if doubled.length() == 3 then 0 else 1
}
"#,
        "coll_list_iter_map_collect",
    );
}

#[test]
fn test_coll_list_iter_any_all() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [2, 4, 6, 8];
    let all_even = xs.iter().all(x -> x % 2 == 0);
    let any_ten = xs.iter().any(x -> x == 10);
    if all_even && !any_ten then 0 else 1
}
"#,
        "coll_list_iter_any_all",
    );
}

// ─── List: clone ───

#[test]
fn test_coll_list_clone_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.clone();
    if ys.length() == 3 then 0 else 1
}
"#,
        "coll_list_clone_int",
    );
}

// ─── List: for-yield ───

#[test]
fn test_coll_list_for_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..5 yield i * 10;
    if xs.length() == 5 then 0 else 1
}
"#,
        "coll_list_for_yield",
    );
}

#[test]
fn test_coll_list_for_yield_filter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..10 if i % 2 == 0 yield i;
    if xs.length() == 5 then 0 else 1
}
"#,
        "coll_list_for_yield_filter",
    );
}

// ─── List: string elements ───

#[test]
fn test_coll_list_string_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let names = ["alice", "bob", "charlie"];
    if names.length() == 3 then 0 else 1
}
"#,
        "coll_list_str_len",
    );
}

#[test]
fn test_coll_list_string_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let names = ["hello", "world"];
    let count = names.iter().count();
    if count == 2 then 0 else 1
}
"#,
        "coll_list_str_iter",
    );
}

// ─── Map: length ───

#[test]
fn test_coll_map_length_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"x": 1, "y": 2};
    if m.length() == 2 then 0 else 1
}
"#,
        "coll_map_len_basic",
    );
}

#[test]
fn test_coll_map_length_one() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"key": 42};
    if m.length() == 1 then 0 else 1
}
"#,
        "coll_map_len_one",
    );
}

#[test]
fn test_coll_map_len_alias() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    if m.len() == 3 then 0 else 1
}
"#,
        "coll_map_len_alias",
    );
}

// ─── Map: iter ───

#[test]
fn test_coll_map_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    let count = m.iter().count();
    if count == 3 then 0 else 1
}
"#,
        "coll_map_iter_count",
    );
}

#[test]
fn test_coll_map_for_loop_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"x": 10, "y": 20};
    let count = 0;
    for entry in m do {
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "coll_map_for_loop",
    );
}

// ─── Map: int keys ───

#[test]
fn test_coll_map_int_keys() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {1: "one", 2: "two", 3: "three"};
    if m.length() == 3 then 0 else 1
}
"#,
        "coll_map_int_keys",
    );
}

// ─── List + Map interaction ───

#[test]
fn test_coll_list_of_tuples() {
    assert_aot_success(
        r#"
@main () -> int = {
    let pairs = [(1, "a"), (2, "b"), (3, "c")];
    if pairs.length() == 3 then 0 else 1
}
"#,
        "coll_list_of_tuples",
    );
}

// ─── Gap inventory: list methods not in builtin table ───

#[test]
#[ignore = "AOT gap: list.push() not in builtin table"]
fn test_coll_list_push() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.push(4);
    if ys.length() == 4 then 0 else 1
}
"#,
        "coll_list_push",
    );
}

#[test]
#[ignore = "AOT gap: list.pop() not in builtin table"]
fn test_coll_list_pop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let (last, rest) = xs.pop();
    if last == 3 && rest.length() == 2 then 0 else 1
}
"#,
        "coll_list_pop",
    );
}

#[test]
#[ignore = "AOT gap: list.first() not in builtin table"]
fn test_coll_list_first() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let f = xs.first();
    if f.is_some() && f.unwrap() == 10 then 0 else 1
}
"#,
        "coll_list_first",
    );
}

#[test]
#[ignore = "AOT gap: list.last() not in builtin table"]
fn test_coll_list_last() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let l = xs.last();
    if l.is_some() && l.unwrap() == 30 then 0 else 1
}
"#,
        "coll_list_last",
    );
}

#[test]
#[ignore = "AOT gap: list[index] subscript not resolved"]
fn test_coll_list_index() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    if xs[0] == 10 && xs[2] == 30 then 0 else 1
}
"#,
        "coll_list_index",
    );
}

#[test]
#[ignore = "AOT gap: list.reverse() not in builtin table"]
fn test_coll_list_reverse() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let rev = xs.reverse();
    if rev.length() == 3 then 0 else 1
}
"#,
        "coll_list_reverse",
    );
}

#[test]
#[ignore = "AOT gap: list.contains() not in builtin table"]
fn test_coll_list_contains() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    if xs.contains(3) then 0 else 1
}
"#,
        "coll_list_contains",
    );
}

// ─── Gap inventory: map methods not in builtin table ───

#[test]
#[ignore = "AOT gap: map.get() not in builtin table"]
fn test_coll_map_get() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let v = m.get("a");
    if v.is_some() && v.unwrap() == 1 then 0 else 1
}
"#,
        "coll_map_get",
    );
}

#[test]
#[ignore = "AOT gap: map.contains_key() not in builtin table"]
fn test_coll_map_contains_key() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    if m.contains_key("a") && !m.contains_key("z") then 0 else 1
}
"#,
        "coll_map_contains_key",
    );
}

#[test]
#[ignore = "AOT gap: map.keys() not in builtin table"]
fn test_coll_map_keys() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let ks = m.keys();
    if ks.length() == 2 then 0 else 1
}
"#,
        "coll_map_keys",
    );
}

#[test]
#[ignore = "AOT gap: map.values() not in builtin table"]
fn test_coll_map_values() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let vs = m.values();
    if vs.length() == 2 then 0 else 1
}
"#,
        "coll_map_values",
    );
}

#[test]
#[ignore = "AOT gap: map.insert() not in builtin table"]
fn test_coll_map_insert() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1};
    let m2 = m.insert("b", 2);
    if m2.length() == 2 then 0 else 1
}
"#,
        "coll_map_insert",
    );
}

#[test]
#[ignore = "AOT gap: map.remove() not in builtin table"]
fn test_coll_map_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let m2 = m.remove("a");
    if m2.length() == 1 then 0 else 1
}
"#,
        "coll_map_remove",
    );
}
