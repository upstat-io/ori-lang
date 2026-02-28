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

// ─── Map: is_empty ───

#[test]
fn test_coll_map_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {str: int} = {};
    if m.is_empty() then 0 else 1
}
"#,
        "coll_map_is_empty_true",
    );
}

#[test]
fn test_coll_map_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1};
    if !m.is_empty() then 0 else 1
}
"#,
        "coll_map_is_empty_false",
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
fn test_coll_list_pop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let last = xs.pop();
    if last.is_some() && last.unwrap() == 3 then 0 else 1
}
"#,
        "coll_list_pop",
    );
}

#[test]
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

// ─── Edge cases: list methods ───

#[test]
fn test_coll_list_first_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    let f = xs.first();
    if f.is_none() then 0 else 1
}
"#,
        "coll_list_first_empty",
    );
}

#[test]
fn test_coll_list_last_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    let l = xs.last();
    if l.is_none() then 0 else 1
}
"#,
        "coll_list_last_empty",
    );
}

#[test]
fn test_coll_list_contains_missing() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    if !xs.contains(99) then 0 else 1
}
"#,
        "coll_list_contains_missing",
    );
}

#[test]
fn test_coll_list_push_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    let ys = xs.push(42);
    if ys.length() == 1 then 0 else 1
}
"#,
        "coll_list_push_empty",
    );
}

#[test]
fn test_coll_list_reverse_single() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [7];
    let rev = xs.reverse();
    if rev.length() == 1 then 0 else 1
}
"#,
        "coll_list_reverse_single",
    );
}

// ─── COW-specific list tests (Section 02.7) ───

#[test]
fn test_coll_list_push_multi() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1].push(2).push(3).push(4).push(5);
    if xs.length() == 5 then 0 else 1
}
"#,
        "coll_list_push_multi",
    );
}

#[test]
fn test_coll_list_concat_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2, 3];
    let b = [4, 5];
    let c = a.concat(b);
    if c.length() == 5 then 0 else 1
}
"#,
        "coll_list_concat_basic",
    );
}

#[test]
fn test_coll_list_concat_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2];
    let b: [int] = [];
    let c = a.concat(b);
    if c.length() == 2 then 0 else 1
}
"#,
        "coll_list_concat_empty",
    );
}

#[test]
fn test_coll_list_add_operator() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2];
    let b = [3, 4];
    let c = a + b;
    if c.length() == 4 then 0 else 1
}
"#,
        "coll_list_add_operator",
    );
}

#[test]
fn test_coll_list_reverse_values() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let rev = xs.reverse();
    let f = rev.first();
    if f.is_some() && f.unwrap() == 30 then 0 else 1
}
"#,
        "coll_list_reverse_values",
    );
}

#[test]
fn test_coll_list_reverse_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    let rev = xs.reverse();
    if rev.length() == 0 then 0 else 1
}
"#,
        "coll_list_reverse_empty",
    );
}

#[test]
fn test_coll_list_push_then_reverse() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2].push(3).reverse();
    let f = xs.first();
    if xs.length() == 3 && f.is_some() && f.unwrap() == 3 then 0 else 1
}
"#,
        "coll_list_push_then_reverse",
    );
}

#[test]
fn test_coll_list_push_push() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1].push(2).push(3);
    if xs.length() == 3 && xs.first().unwrap() == 1 && xs.last().unwrap() == 3 then 0 else 1
}
"#,
        "coll_list_push_push",
    );
}

#[test]
fn test_coll_list_reverse_reverse() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3].reverse().reverse();
    if xs.length() == 3 && xs.first().unwrap() == 1 && xs.last().unwrap() == 3 then 0 else 1
}
"#,
        "coll_list_reverse_reverse",
    );
}

#[test]
fn test_coll_list_push_concat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2].push(3) + [4, 5];
    if xs.length() == 5 && xs.first().unwrap() == 1 && xs.last().unwrap() == 5 then 0 else 1
}
"#,
        "coll_list_push_concat",
    );
}

#[test]
fn test_coll_list_concat_reverse() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ([1, 2] + [3]).reverse();
    if xs.length() == 3 && xs.first().unwrap() == 3 && xs.last().unwrap() == 1 then 0 else 1
}
"#,
        "coll_list_concat_reverse",
    );
}

// ─── COW list set/insert/remove/sort ───

#[test]
fn test_coll_list_set_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let ys = xs.set(1, 99);
    if ys.length() == 3 && ys.first().unwrap() == 10 && ys.last().unwrap() == 30 then 0 else 1
}
"#,
        "coll_list_set_basic",
    );
}

#[test]
fn test_coll_list_set_first() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let ys = xs.set(0, 99);
    if ys.first().unwrap() == 99 then 0 else 1
}
"#,
        "coll_list_set_first",
    );
}

#[test]
fn test_coll_list_set_last() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let ys = xs.set(2, 99);
    if ys.last().unwrap() == 99 then 0 else 1
}
"#,
        "coll_list_set_last",
    );
}

#[test]
fn test_coll_list_insert_beginning() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [2, 3];
    let ys = xs.insert(0, 1);
    if ys.length() == 3 && ys.first().unwrap() == 1 then 0 else 1
}
"#,
        "coll_list_insert_beginning",
    );
}

#[test]
fn test_coll_list_insert_middle() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 3];
    let ys = xs.insert(1, 2);
    if ys.length() == 3 then 0 else 1
}
"#,
        "coll_list_insert_middle",
    );
}

#[test]
fn test_coll_list_insert_end() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2];
    let ys = xs.insert(2, 3);
    if ys.length() == 3 && ys.last().unwrap() == 3 then 0 else 1
}
"#,
        "coll_list_insert_end",
    );
}

#[test]
fn test_coll_list_remove_first() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.remove(0);
    if ys.length() == 2 && ys.first().unwrap() == 2 then 0 else 1
}
"#,
        "coll_list_remove_first",
    );
}

#[test]
fn test_coll_list_remove_last() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.remove(2);
    if ys.length() == 2 && ys.last().unwrap() == 2 then 0 else 1
}
"#,
        "coll_list_remove_last",
    );
}

#[test]
fn test_coll_list_sort_ints() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [3, 1, 4, 1, 5, 9, 2, 6];
    let sorted = xs.sort();
    if sorted.length() == 8 && sorted.first().unwrap() == 1 && sorted.last().unwrap() == 9 then 0 else 1
}
"#,
        "coll_list_sort_ints",
    );
}

#[test]
fn test_coll_list_sort_already_sorted() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let sorted = xs.sort();
    if sorted.first().unwrap() == 1 && sorted.last().unwrap() == 5 then 0 else 1
}
"#,
        "coll_list_sort_already_sorted",
    );
}

#[test]
fn test_coll_list_sort_reverse_order() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [5, 4, 3, 2, 1];
    let sorted = xs.sort();
    if sorted.first().unwrap() == 1 && sorted.last().unwrap() == 5 then 0 else 1
}
"#,
        "coll_list_sort_reverse_order",
    );
}

#[test]
fn test_coll_list_sort_single() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [42];
    let sorted = xs.sort();
    if sorted.length() == 1 && sorted.first().unwrap() == 42 then 0 else 1
}
"#,
        "coll_list_sort_single",
    );
}

// ─── COW loop-based push (1000-element benchmark) ───

#[test]
fn test_coll_list_push_loop_1000() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    for i in 0..1000 do {
        xs = xs.push(i);
    };
    if xs.length() == 1000 && xs.first().unwrap() == 0 && xs.last().unwrap() == 999 then 0 else 1
}
"#,
        "coll_list_push_loop_1000",
    );
}

// ─── COW sharing: mutate copy, original untouched ───

#[test]
fn test_coll_list_cow_push_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.push(4);
    // xs should still be length 3, ys should be length 4
    if xs.length() == 3 && ys.length() == 4 then 0 else 1
}
"#,
        "coll_list_cow_push_shared",
    );
}

#[test]
fn test_coll_list_cow_reverse_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.reverse();
    // xs should still be [1,2,3], ys should be [3,2,1]
    if xs.first().unwrap() == 1 && ys.first().unwrap() == 3 then 0 else 1
}
"#,
        "coll_list_cow_reverse_shared",
    );
}

#[test]
fn test_coll_list_cow_sort_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [3, 1, 2];
    let ys = xs.sort();
    // xs should still be [3,1,2], ys should be [1,2,3]
    if xs.first().unwrap() == 3 && ys.first().unwrap() == 1 then 0 else 1
}
"#,
        "coll_list_cow_sort_shared",
    );
}

#[test]
fn test_coll_list_cow_set_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let ys = xs.set(1, 99);
    // xs should still have 20 at index 1
    if xs.last().unwrap() == 30 && ys.length() == 3 then 0 else 1
}
"#,
        "coll_list_cow_set_shared",
    );
}

#[test]
fn test_coll_list_cow_concat_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2];
    let ys = xs + [3, 4];
    // xs should still be length 2
    if xs.length() == 2 && ys.length() == 4 then 0 else 1
}
"#,
        "coll_list_cow_concat_shared",
    );
}

// ─── Gap inventory: map methods not in builtin table ───

#[test]
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

// ─── Edge cases for map methods ───

#[test]
fn test_coll_map_contains_key_missing() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"x": 10};
    if m.contains_key("y") then 1 else 0
}
"#,
        "coll_map_contains_key_missing",
    );
}

#[test]
fn test_coll_map_keys_single() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"only": 42};
    let ks = m.keys();
    if ks.length() == 1 then 0 else 1
}
"#,
        "coll_map_keys_single",
    );
}

#[test]
fn test_coll_map_values_sum() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 10, "b": 20, "c": 30};
    let vs = m.values();
    if vs.length() == 3 then 0 else 1
}
"#,
        "coll_map_values_sum",
    );
}
