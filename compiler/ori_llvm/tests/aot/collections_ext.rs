//! Collection Method Extension AOT Tests
//!
//! Tests for List, Map, and Set method coverage beyond what exists in spec.rs
//! and `for_loops.rs`. Focuses on method calls (length, `is_empty`, iter, clone)
//! and gap inventory for methods not yet in the AOT builtin table.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// ─── List: length ───

#[test]
fn test_coll_list_length_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_length_empty.ori"),
        "coll_list_len_empty",
    );
}

#[test]
fn test_coll_list_length_one() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_length_one.ori"),
        "coll_list_len_one",
    );
}

#[test]
fn test_coll_list_length_many() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_length_many.ori"),
        "coll_list_len_many",
    );
}

#[test]
fn test_coll_list_len_alias() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_len_alias.ori"),
        "coll_list_len_alias",
    );
}

// ─── List: is_empty ───

#[test]
fn test_coll_list_is_empty_true() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_is_empty_true.ori"),
        "coll_list_is_empty_true",
    );
}

#[test]
fn test_coll_list_is_empty_false() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_is_empty_false.ori"),
        "coll_list_is_empty_false",
    );
}

// ─── List: iter & collect ───

#[test]
fn test_coll_list_iter_count() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_iter_count.ori"),
        "coll_list_iter_count",
    );
}

#[test]
fn test_coll_list_iter_sum_via_fold() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_iter_sum_via_fold.ori"),
        "coll_list_iter_fold",
    );
}

#[test]
fn test_coll_list_iter_filter_count() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_iter_filter_count.ori"),
        "coll_list_iter_filter",
    );
}

#[test]
fn test_coll_list_iter_map_collect_length() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_iter_map_collect_length.ori"),
        "coll_list_iter_map_collect",
    );
}

#[test]
fn test_coll_list_iter_any_all() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_iter_any_all.ori"),
        "coll_list_iter_any_all",
    );
}

// ─── List: clone ───

#[test]
fn test_coll_list_clone_int() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_clone_int.ori"),
        "coll_list_clone_int",
    );
}

// ─── List: for-yield ───

#[test]
fn test_coll_list_for_yield() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_for_yield.ori"),
        "coll_list_for_yield",
    );
}

#[test]
fn test_coll_list_for_yield_filter() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_for_yield_filter.ori"),
        "coll_list_for_yield_filter",
    );
}

// ─── List: string elements ───

#[test]
fn test_coll_list_string_length() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_string_length.ori"),
        "coll_list_str_len",
    );
}

#[test]
fn test_coll_list_string_iter_count() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_string_iter_count.ori"),
        "coll_list_str_iter",
    );
}

// ─── Map: length ───

#[test]
fn test_coll_map_length_basic() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_length_basic.ori"),
        "coll_map_len_basic",
    );
}

#[test]
fn test_coll_map_length_one() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_length_one.ori"),
        "coll_map_len_one",
    );
}

#[test]
fn test_coll_map_len_alias() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_len_alias.ori"),
        "coll_map_len_alias",
    );
}

// ─── Map: iter ───

#[test]
fn test_coll_map_iter_count() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_iter_count.ori"),
        "coll_map_iter_count",
    );
}

#[test]
fn test_coll_map_for_loop_count() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_for_loop_count.ori"),
        "coll_map_for_loop",
    );
}

// ─── Map: is_empty ───

#[test]
fn test_coll_map_is_empty_true() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_is_empty_true.ori"),
        "coll_map_is_empty_true",
    );
}

#[test]
fn test_coll_map_is_empty_false() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_is_empty_false.ori"),
        "coll_map_is_empty_false",
    );
}

// ─── Map: int keys ───

#[test]
fn test_coll_map_int_keys() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_int_keys.ori"),
        "coll_map_int_keys",
    );
}

// ─── List + Map interaction ───

#[test]
fn test_coll_list_of_tuples() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_of_tuples.ori"),
        "coll_list_of_tuples",
    );
}

// ─── Gap inventory: list methods not in builtin table ───

#[test]
fn test_coll_list_push() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push.ori"),
        "coll_list_push",
    );
}

#[test]
fn test_coll_list_pop() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_pop.ori"),
        "coll_list_pop",
    );
}

#[test]
fn test_coll_list_pop_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_pop_empty.ori"),
        "coll_list_pop_empty",
    );
}

#[test]
fn test_coll_list_first() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_first.ori"),
        "coll_list_first",
    );
}

#[test]
fn test_coll_list_last() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_last.ori"),
        "coll_list_last",
    );
}

#[test]
fn test_coll_list_index() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_index.ori"),
        "coll_list_index",
    );
}

#[test]
fn test_coll_list_reverse() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_reverse.ori"),
        "coll_list_reverse",
    );
}

#[test]
fn test_coll_list_contains() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_contains.ori"),
        "coll_list_contains",
    );
}

// ─── Edge cases: list methods ───

#[test]
fn test_coll_list_first_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_first_empty.ori"),
        "coll_list_first_empty",
    );
}

#[test]
fn test_coll_list_last_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_last_empty.ori"),
        "coll_list_last_empty",
    );
}

#[test]
fn test_coll_list_contains_missing() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_contains_missing.ori"),
        "coll_list_contains_missing",
    );
}

#[test]
fn test_coll_list_push_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push_empty.ori"),
        "coll_list_push_empty",
    );
}

#[test]
fn test_coll_list_reverse_single() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_reverse_single.ori"),
        "coll_list_reverse_single",
    );
}

// ─── COW-specific list tests (Section 02.7) ───

#[test]
fn test_coll_list_push_multi() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push_multi.ori"),
        "coll_list_push_multi",
    );
}

#[test]
fn test_coll_list_concat_basic() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_concat_basic.ori"),
        "coll_list_concat_basic",
    );
}

#[test]
fn test_coll_list_concat_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_concat_empty.ori"),
        "coll_list_concat_empty",
    );
}

#[test]
fn test_coll_list_add_operator() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_add_operator.ori"),
        "coll_list_add_operator",
    );
}

#[test]
fn test_coll_list_reverse_values() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_reverse_values.ori"),
        "coll_list_reverse_values",
    );
}

#[test]
fn test_coll_list_reverse_empty() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_reverse_empty.ori"),
        "coll_list_reverse_empty",
    );
}

#[test]
fn test_coll_list_push_then_reverse() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push_then_reverse.ori"),
        "coll_list_push_then_reverse",
    );
}

#[test]
fn test_coll_list_push_push() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push_push.ori"),
        "coll_list_push_push",
    );
}

#[test]
fn test_coll_list_reverse_reverse() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_reverse_reverse.ori"),
        "coll_list_reverse_reverse",
    );
}

#[test]
fn test_coll_list_push_concat() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push_concat.ori"),
        "coll_list_push_concat",
    );
}

#[test]
fn test_coll_list_concat_reverse() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_concat_reverse.ori"),
        "coll_list_concat_reverse",
    );
}

// ─── COW list set/insert/remove/sort ───

#[test]
fn test_coll_list_set_basic() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_set_basic.ori"),
        "coll_list_set_basic",
    );
}

#[test]
fn test_coll_list_set_first() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_set_first.ori"),
        "coll_list_set_first",
    );
}

#[test]
fn test_coll_list_set_last() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_set_last.ori"),
        "coll_list_set_last",
    );
}

#[test]
fn test_coll_list_insert_beginning() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_insert_beginning.ori"),
        "coll_list_insert_beginning",
    );
}

#[test]
fn test_coll_list_insert_middle() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_insert_middle.ori"),
        "coll_list_insert_middle",
    );
}

#[test]
fn test_coll_list_insert_end() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_insert_end.ori"),
        "coll_list_insert_end",
    );
}

#[test]
fn test_coll_list_remove_first() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_remove_first.ori"),
        "coll_list_remove_first",
    );
}

#[test]
fn test_coll_list_remove_last() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_remove_last.ori"),
        "coll_list_remove_last",
    );
}

#[test]
fn test_coll_list_sort_ints() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_sort_ints.ori"),
        "coll_list_sort_ints",
    );
}

#[test]
fn test_coll_list_sort_already_sorted() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_sort_already_sorted.ori"),
        "coll_list_sort_already_sorted",
    );
}

#[test]
fn test_coll_list_sort_reverse_order() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_sort_reverse_order.ori"),
        "coll_list_sort_reverse_order",
    );
}

#[test]
fn test_coll_list_sort_single() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_sort_single.ori"),
        "coll_list_sort_single",
    );
}

// ─── COW loop-based push (1000-element benchmark) ───

#[test]
fn test_coll_list_push_loop_1000() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_push_loop_1000.ori"),
        "coll_list_push_loop_1000",
    );
}

// ─── COW sharing: mutate copy, original untouched ───

#[test]
fn test_coll_list_cow_push_shared() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_cow_push_shared.ori"),
        "coll_list_cow_push_shared",
    );
}

#[test]
fn test_coll_list_cow_reverse_shared() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_cow_reverse_shared.ori"),
        "coll_list_cow_reverse_shared",
    );
}

#[test]
fn test_coll_list_cow_sort_shared() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_cow_sort_shared.ori"),
        "coll_list_cow_sort_shared",
    );
}

#[test]
fn test_coll_list_cow_set_shared() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_cow_set_shared.ori"),
        "coll_list_cow_set_shared",
    );
}

#[test]
fn test_coll_list_cow_concat_shared() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_cow_concat_shared.ori"),
        "coll_list_cow_concat_shared",
    );
}

// ─── COW list sort_stable ───

#[test]
fn test_coll_list_sort_stable_ints() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_sort_stable_ints.ori"),
        "coll_list_sort_stable_ints",
    );
}

#[test]
fn test_coll_list_sort_stable_cow_shared() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_sort_stable_cow_shared.ori"),
        "coll_list_sort_stable_cow_shared",
    );
}

// ─── Gap inventory: map methods not in builtin table ───

#[test]
fn test_coll_map_get() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_get.ori"),
        "coll_map_get",
    );
}

#[test]
fn test_coll_map_contains_key() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_contains_key.ori"),
        "coll_map_contains_key",
    );
}

#[test]
fn test_coll_map_keys() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_keys.ori"),
        "coll_map_keys",
    );
}

#[test]
fn test_coll_map_values() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_values.ori"),
        "coll_map_values",
    );
}

#[test]
fn test_coll_map_insert() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_insert.ori"),
        "coll_map_insert",
    );
}

#[test]
fn test_coll_map_remove() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_remove.ori"),
        "coll_map_remove",
    );
}

// ─── Edge cases for map methods ───

#[test]
fn test_coll_map_contains_key_missing() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_contains_key_missing.ori"),
        "coll_map_contains_key_missing",
    );
}

#[test]
fn test_coll_map_keys_single() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_keys_single.ori"),
        "coll_map_keys_single",
    );
}

#[test]
fn test_coll_map_values_sum() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_values_sum.ori"),
        "coll_map_values_sum",
    );
}

// ─── List indexing ───

#[test]
fn test_coll_list_index_variable() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_index_variable.ori"),
        "coll_list_index_variable",
    );
}

#[test]
fn test_coll_list_index_expression() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_list_index_expression.ori"),
        "coll_list_index_expression",
    );
}

#[test]
fn test_coll_list_index_oob_panics() {
    let (exit_code, _, _) = compile_and_run_capture(include_str!(
        "fixtures/collections_ext/coll_list_index_oob_panics.ori"
    ));
    assert_ne!(exit_code, 0, "OOB index should panic (non-zero exit)");
}

#[test]
fn test_coll_map_index() {
    assert_aot_success(
        include_str!("fixtures/collections_ext/coll_map_index.ori"),
        "coll_map_index",
    );
}
