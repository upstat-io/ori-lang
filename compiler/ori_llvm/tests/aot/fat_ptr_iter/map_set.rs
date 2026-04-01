//! Map and set iteration tests — string keys/values with RC cleanup.

use crate::util::assert_aot_success;

// Map iteration with string keys/values

#[test]
fn test_map_str_key_iteration() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/map_str_key_iteration.ori"),
        "map_str_key_iteration",
    );
}

// Map iteration with RC-managed keys/values — key_dec_fn/val_dec_fn

#[test]
fn test_map_str_key_for_do_full() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/map_str_key_for_do_full.ori"),
        "map_str_key_for_do_full",
    );
}

/// `{str: int}` map for-do with early break — unconsumed entries' str keys
/// must be cleaned up by `key_dec_fn` in the iterator's Drop path.
#[test]
fn test_map_str_key_for_do_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/map_str_key_for_do_break.ori"),
        "map_str_key_for_do_break",
    );
}

/// `{int: str}` map for-do — heap str values cleaned up via `val_dec_fn`.
#[test]
fn test_map_str_val_for_do() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/map_str_val_for_do.ori"),
        "map_str_val_for_do",
    );
}

/// `{str: str}` map for-do — both keys and values are heap strings,
/// both `key_dec_fn` and `val_dec_fn` must work.
#[test]
fn test_map_str_key_str_val_for_do() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/map_str_key_str_val_for_do.ori"),
        "map_str_key_str_val_for_do",
    );
}

// T8-F3: {str: int} map for-yield — derive values from map iteration.
// Exercises emit_set_iter conversion under for-yield control flow.

#[test]
fn test_map_str_key_for_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/map_str_key_for_yield.ori"),
        "map_str_key_for_yield",
    );
}

// T9: Set<str> — set with string elements

#[test]
fn test_set_str_iteration() {
    // Set<str> converts to contiguous list via ori_set_to_list before iterating.
    // The output list needs elem_dec_fn for proper string cleanup.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/set_str_iteration.ori"),
        "set_str_iteration",
    );
}

// T9-F3: Set<str> for-yield — exercises emit_set_iter conversion to list under
// for-yield control flow. Both source set and derived list need cleanup.

#[test]
fn test_set_str_for_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/map_set/set_str_for_yield.ori"),
        "set_str_for_yield",
    );
}
