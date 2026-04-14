//! Method collect tests — F13: `.iter().collect()` exercises `ori_iter_collect` with `elem_inc_fn`.

use crate::util::assert_aot_success;

// [str].iter().collect() → [str]

#[test]
fn test_iter_collect_str_list() {
    // .iter().collect() exercises ori_iter_collect with elem_inc_fn (RcInc per element).
    // Distinct from for-yield which uses ori_list_push (per-element push).
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/iter_collect_str_list.ori"),
        "iter_collect_str_list",
    );
}

// Set<str>.iter().collect() → Set<str>

#[test]
fn test_iter_collect_set_str() {
    // ori_iter_collect_set with elem_inc_fn (Section 02.3 fix).
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/iter_collect_set_str.ori"),
        "iter_collect_set_str",
    );
}

// [[int]].iter().collect() → [[int]] — nested list elements exercise elem_inc_fn for [int].
// Note: exercises list collect (not __collect_set) — Set<[int]> crashes (BUG-04-065).

#[test]
fn test_iter_collect_nested_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/iter_collect_nested_list.ori"),
        "iter_collect_nested_list",
    );
}

// [{str: int}].iter().collect() → [{str: int}] — map elements exercise elem_inc_fn for {str: int}.
// Note: exercises list collect (not __collect_set) — Set<{str: int}> crashes (BUG-04-065).

#[test]
fn test_iter_collect_map_elements() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/iter_collect_map_elements.ori"),
        "iter_collect_map_elements",
    );
}

// regression tests: collected List<int> stride must match literal stride

/// Semantic pin: map + collect equality with literal.
/// Fails if collect stores at canonical stride but readers use narrowed stride.
#[test]
fn test_collect_int_map_result_equals_literal() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/collect_int_map_equals_literal.ori"),
        "collect_int_map_equals_literal",
    );
}

/// Identity collect: `[1,2,3].iter().collect() == [1,2,3]`
#[test]
fn test_collect_int_identity_equals_literal() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/collect_int_identity_equals.ori"),
        "collect_int_identity_equals",
    );
}

/// Signed narrowing boundary: negative values through collect.
#[test]
fn test_collect_int_negative_values_equals_literal() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/collect_int_negative_values.ori"),
        "collect_int_negative_values",
    );
}

/// Adapter chain (filter) before collect.
#[test]
fn test_collect_int_filter_result_equals_literal() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/collect_int_filter_equals.ori"),
        "collect_int_filter_equals",
    );
}
