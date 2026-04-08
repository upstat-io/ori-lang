//! `elem_dec_fn` scope drop verification tests.
//!
//! Verifies that `elem_dec_fn` stored in the RC header correctly cleans up
//! fat pointer elements (str, [T], closures) when collections go out of
//! scope — without iteration. These tests validate the codegen wiring from
//! Section 02.1 of the rc-header-elem-dec plan.
//!
//! All tests run with `ORI_CHECK_LEAKS=1` (via `assert_aot_success`).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// [str] scope drop

#[test]
fn test_str_list_scope_drop() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_scope_drop.ori"),
        "str_list_scope_drop",
    );
}

// [[int]] nested list scope drop

#[test]
fn test_nested_int_list_scope_drop() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/nested_int_list_scope_drop.ori"),
        "nested_int_list_scope_drop",
    );
}

// [str] COW push on shared list

#[test]
fn test_str_list_cow_push_shared() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_cow_push_shared.ori"),
        "str_list_cow_push_shared",
    );
}

// [str] with mixed SSO and heap strings

#[test]
fn test_str_list_mixed_sso_heap() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_mixed_sso_heap.ori"),
        "str_list_mixed_sso_heap",
    );
}

// ori_iter_collect on [str]

#[test]
fn test_str_list_iter_collect() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_iter_collect.ori"),
        "str_list_iter_collect",
    );
}

// .collect() method on [str] iterator — exercises ori_iter_collect runtime function.
// This is different from for-yield (which uses an explicit ARC-managed loop).
// Regression guard: ori_iter_collect must call elem_inc_fn after copying each
// element, or the iterator's Drop double-frees shared child data.

#[test]
fn test_str_list_method_collect() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_method_collect.ori"),
        "str_list_method_collect",
    );
}

// Trampoline ABI for fat-pointer types — regression guards for the sret/indirect
// calling convention fix. Prior to this fix, the Map trampoline loaded 24-byte
// str values by-value and called the closure as if it used direct return, but
// the closure actually uses sret + indirect param ABI for types > 16 bytes.
// Semantic pin: would crash with SIGSEGV if trampoline reverts to by-value ABI.

#[test]
fn test_trampoline_map_str_identity() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/trampoline_map_str_identity.ori"),
        "trampoline_map_str_identity",
    );
}

#[test]
fn test_trampoline_filter_str() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/trampoline_filter_str.ori"),
        "trampoline_filter_str",
    );
}

#[test]
fn test_trampoline_fold_str() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/trampoline_fold_str.ori"),
        "trampoline_fold_str",
    );
}

// ForEach trampoline with fat-pointer elements — semantic pin for the
// TrampolineKind::ForEach indirect-call path. Prior tests cover Map, Predicate,
// and Fold; this covers the remaining ForEach branch where the closure accepts
// an indirect parameter and returns void.
// Semantic pin: would crash with SIGSEGV if ForEach trampoline reverts to
// by-value ABI for types > 16 bytes.

#[test]
#[ignore = "codegen gap: for_each with closure hits unresolved type variable (pre-existing)"]
fn test_trampoline_for_each_str() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/trampoline_for_each_str.ori"),
        "trampoline_for_each_str",
    );
}

// {str: int} map iteration — verify elem_dec_fn on map keys via ownership transfer

#[test]
fn test_map_str_iteration() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_str_iteration.ori"),
        "map_str_iteration",
    );
}

// {str: int} map passed to function and iterated inside

#[test]
fn test_map_str_passed_to_fn() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_str_passed_to_fn.ori"),
        "map_str_passed_to_fn",
    );
}

// map.keys() on {str: int}

#[test]
fn test_map_keys_str_scope_drop() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_keys_str_scope_drop.ori"),
        "map_keys_str_scope_drop",
    );
}

// map.insert() with heap string key — double-free regression guard.
// The inserted key is borrowed from the caller and shallow-copied into the
// map's hash buffer. Without key_inc after the copy, the caller's RcDec
// frees the string data while the map buffer still references it, causing
// double-free when the map's drop function later fires key_dec_fn.
#[test]
fn test_map_insert_heap_str_key() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_insert_heap_str_key.ori"),
        "map_insert_heap_str_key",
    );
}

// map.insert() with heap string value — exercises val_inc path.
#[test]
fn test_map_insert_heap_str_value() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_insert_heap_str_value.ori"),
        "map_insert_heap_str_value",
    );
}

// COW map insert shared — slow path with key_inc/val_inc on rehashed entries.
#[test]
fn test_map_cow_insert_shared_heap_key() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_cow_insert_shared_heap_key.ori"),
        "map_cow_insert_shared_heap_key",
    );
}

// Map insert overwrite with heap string value — exercises val_dec on old value.
// cow_insert_existing fast path (unique) must dec the old value
// before overwriting with the new one.
#[test]
fn test_map_insert_overwrite_heap_str_value() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_insert_overwrite_heap_str_value.ori"),
        "map_insert_overwrite_heap_str_value",
    );
}

// Map insert overwrite with heap string value on shared map — slow path.
// Exercises slow_copy_overwrite_value.
#[test]
fn test_map_insert_overwrite_shared_heap_str_value() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_insert_overwrite_shared_heap_str_value.ori"),
        "map_insert_overwrite_shared_heap_str_value",
    );
}

// Map insert overwrite multiple times — exercises repeated val_dec on unique map.
#[test]
fn test_map_insert_overwrite_multiple_heap_str_value() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_insert_overwrite_multiple_heap_str_value.ori"),
        "map_insert_overwrite_multiple_heap_str_value",
    );
}

// str.split(sep:) returning [str]

#[test]
fn test_str_split_scope_drop() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_split_scope_drop.ori"),
        "str_split_scope_drop",
    );
}

// 02.2 Iterator creation and drop — header-based elem_dec_fn verification

// Iterator is last owner: words not used after loop, ARC may dec words
// before loop ends. Iterator Drop triggers cleanup via header elem_dec_fn.
#[test]
fn test_str_list_iter_last_owner() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_iter_last_owner.ori"),
        "str_list_iter_last_owner",
    );
}

// Explicit dec is last owner: words used after loop, so ARC keeps it alive.
// Iterator drops at loop end (RC 2→1), words drops at block exit (RC 1→0).
#[test]
fn test_str_list_explicit_last_owner() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_explicit_last_owner.ori"),
        "str_list_explicit_last_owner",
    );
}

// Function parameter: callee iterates [str], caller uses after return.
// Both callee's iterator dec and caller's scope dec hit ori_buffer_rc_dec.
// store_elem_dec_fn_once ensures header is populated by whichever fires first.
#[test]
fn test_str_list_fn_param_iter() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_fn_param_iter.ori"),
        "str_list_fn_param_iter",
    );
}

// Slice + iteration: create [str], take a slice, iterate the original list.
// Both slice and iterator share the SAME buffer's header. store_elem_dec_fn_once
// ensures the header is populated once. Whichever is last owner triggers cleanup.
#[test]
fn test_str_list_slice_then_iter() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/str_list_slice_then_iter.ori"),
        "str_list_slice_then_iter",
    );
}

// Set<str> scope drop and iteration

#[test]
fn test_set_str_iteration() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_iteration.ori"),
        "set_str_iteration",
    );
}

#[test]
fn test_set_str_passed_to_fn() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_passed_to_fn.ori"),
        "set_str_passed_to_fn",
    );
}

#[test]
fn test_set_str_cow_insert_shared() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_cow_insert_shared.ori"),
        "set_str_cow_insert_shared",
    );
}

#[test]
fn test_set_str_union() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_union.ori"),
        "set_str_union",
    );
}

#[test]
fn test_set_str_to_list() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_to_list.ori"),
        "set_str_to_list",
    );
}

// Set<str> remove — fat-pointer element cleanup on remove

#[test]
fn test_set_str_remove_remaining() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_remove_remaining.ori"),
        "set_str_remove_remaining",
    );
}

#[test]
fn test_set_str_remove_last_element() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_remove_last_element.ori"),
        "set_str_remove_last",
    );
}

// Set<str> intersection — fat-pointer element cleanup on filter

#[test]
fn test_set_str_intersection_unique() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_intersection_unique.ori"),
        "set_str_intersection_unique",
    );
}

#[test]
fn test_set_str_difference_unique() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_difference_unique.ori"),
        "set_str_difference_unique",
    );
}

// Map remove — fat-pointer key/value cleanup (discovered during )

#[test]
fn test_map_remove_str_key() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_remove_str_key.ori"),
        "map_remove_str_key",
    );
}

#[test]
fn test_map_remove_str_key_last() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_remove_str_key_last.ori"),
        "map_remove_str_key_last",
    );
}

// Shared set remove double-dec — removing last element from aliased set

#[test]
fn test_set_str_remove_last_shared() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_remove_last_shared.ori"),
        "set_str_remove_last_shared",
    );
}

#[test]
fn test_set_str_remove_last_shared_only_alias_survives() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_remove_last_shared_only_alias_survives.ori"),
        "set_str_remove_last_shared_alias_survives",
    );
}

// Set<str> collect via ori_iter_collect_set — exercises elem_inc_fn on set elements.

#[test]
fn test_set_str_iter_collect() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/set_str_iter_collect.ori"),
        "set_str_iter_collect",
    );
}

// map.values() with fat-pointer values — exercises val_inc_fn path in
// ori_map_values_to_list. Previous tests only used {str: int} (primitive values).

#[test]
fn test_map_values_heap_str_values() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_values_heap_str_values.ori"),
        "map_values_heap_str_values",
    );
}

#[test]
fn test_map_values_str_str() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_values_str_str.ori"),
        "map_values_str_str",
    );
}

// map remove with fat-pointer values — exercises val_dec branches in
// ori_map_remove_cow (cow.rs empty sentinel path, cow.rs unique fast path).

#[test]
fn test_map_remove_heap_str_value() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_remove_heap_str_value.ori"),
        "map_remove_heap_str_value",
    );
}

#[test]
fn test_map_remove_heap_str_value_last() {
    assert_aot_success(
        include_str!("fixtures/elem_dec_scope/map_remove_heap_str_value_last.ori"),
        "map_remove_heap_str_value_last",
    );
}
