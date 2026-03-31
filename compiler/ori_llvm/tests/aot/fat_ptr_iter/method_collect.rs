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
    // ori_iter_collect_set with elem_inc_fn (Section 02.3 TPR-02-009 fix).
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/method_collect/iter_collect_set_str.ori"),
        "iter_collect_set_str",
    );
}
