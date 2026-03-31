//! COW mutation tests — push element into another collection during iteration.
//!
//! The loop element `w` is borrowed from the iterator. When pushed into another
//! list, it escapes the borrow scope. ARC pipeline must `RcInc` the element before
//! the consuming push call.

use crate::util::assert_aot_success;

#[test]
fn test_push_element_in_for_loop() {
    // Manual list construction via push in a for-do loop.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/push_element_in_for_loop.ori"),
        "push_element_in_for_loop",
    );
}

#[test]
fn test_push_element_borrowed_param() {
    // Borrowed param: push elements from one list into another.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/push_element_borrowed_param.ori"),
        "push_element_borrowed_param",
    );
}

#[test]
fn test_push_element_borrowed_param_two_calls() {
    // Two calls: collect from same borrowed list twice.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/push_element_borrowed_param_two_calls.ori"),
        "push_element_borrowed_param_two_calls",
    );
}

// T2-F11: [[int]] COW push on shared list — inner list elements are RC-managed.

#[test]
fn test_nested_list_cow_push() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/nested_list_cow_push.ori"),
        "nested_list_cow_push",
    );
}

// T9-F11: Set<str> remove on shared set — elem_dec_fn called before tombstoning.

#[test]
fn test_set_str_cow_remove() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/set_str_cow_remove.ori"),
        "set_str_cow_remove",
    );
}

// T8-F11: {str: int} map insert overwriting existing key — old value cleaned up.

#[test]
fn test_map_str_insert_overwrite() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/map_str_insert_overwrite.ori"),
        "map_str_insert_overwrite",
    );
}

// T8-F11: {str: int} map remove on shared map — key_dec_fn/val_dec_fn called.

#[test]
fn test_map_str_cow_remove() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/cow/map_str_cow_remove.ori"),
        "map_str_cow_remove",
    );
}
