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
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing element cleanup on scope exit",
        "third heap allocated string to verify all elements are cleaned up properly"
    ];
    let total = words.len();
    if total == 3 then 0 else 1
}
"#,
        "str_list_scope_drop",
    );
}

// [[int]] nested list scope drop

#[test]
fn test_nested_int_list_scope_drop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[1, 2, 3], [4, 5], [6, 7, 8, 9]];
    let total = nested.len();
    if total == 3 then 0 else 1
}
"#,
        "nested_int_list_scope_drop",
    );
}

// [str] COW push on shared list

#[test]
fn test_str_list_cow_push_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let original = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing COW push on shared buffer"
    ];
    let shared = original;
    // `shared` and `original` alias the same buffer.
    // Push on `shared` triggers COW (new buffer), both must clean up.
    shared = [...shared, "third long string added via COW push exceeds SSO threshold"];
    let ok = original.len() == 2 && shared.len() == 3;
    if ok then 0 else 1
}
"#,
        "str_list_cow_push_shared",
    );
}

// [str] with mixed SSO and heap strings

#[test]
fn test_str_list_mixed_sso_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let mixed = [
        "hi",
        "a very long string that definitely exceeds the twenty three byte SSO threshold",
        "ok",
        "another heap allocated string that is much longer than twenty three bytes",
        "x"
    ];
    let total = mixed.len();
    if total == 5 then 0 else 1
}
"#,
        "str_list_mixed_sso_heap",
    );
}

// ori_iter_collect on [str]

#[test]
fn test_str_list_iter_collect() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing iter collect element cleanup"
    ];
    let collected = for w in words yield w;
    let ok = collected.len() == 2;
    if ok then 0 else 1
}
"#,
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
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing iter collect element cleanup"
    ];
    let collected = words.iter().collect();
    let ok = collected.len() == 2;
    if ok then 0 else 1
}
"#,
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
        r#"
@main () -> int = {
    let words = [
        "this is a very long heap string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing trampoline ABI correctness"
    ];
    let same = words.iter().map(transform: s -> s).collect();
    if same.len() == 2 then 0 else 1
}
"#,
        "trampoline_map_str_identity",
    );
}

#[test]
fn test_trampoline_filter_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long heap string that exceeds the SSO threshold of twenty three bytes",
        "short",
        "another long heap allocated string for testing predicate trampoline ABI"
    ];
    let long = words.iter().filter(predicate: s -> s.len() > 10).collect();
    if long.len() == 2 then 0 else 1
}
"#,
        "trampoline_filter_str",
    );
}

#[test]
fn test_trampoline_fold_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long heap string that exceeds the SSO threshold",
        "another long heap string here",
        "third one"
    ];
    let count = words.iter().fold(initial: 0, op: (acc, s) -> acc + 1);
    if count == 3 then 0 else 1
}
"#,
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
fn test_trampoline_for_each_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long heap string that exceeds the SSO threshold of twenty three bytes",
        "another long heap allocated string to exercise for_each trampoline ABI"
    ];
    words.iter().for_each(action: s -> {
        let $n = s.len();
        n
    });
    0
}
"#,
        "trampoline_for_each_str",
    );
}

// {str: int} map iteration — verify elem_dec_fn on map keys via ownership transfer

#[test]
fn test_map_str_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "a very long key string that exceeds the twenty three byte SSO threshold for testing": 1,
        "another long key string to exercise map iteration with fat pointer elements": 2,
        "third key string also exceeding the SSO threshold for complete coverage": 3
    };
    let total = 0;
    for entry in m do {
        let (k, v) = entry;
        total = total + v
    };
    if total == 6 then 0 else 1
}
"#,
        "map_str_iteration",
    );
}

// {str: int} map passed to function and iterated inside

#[test]
fn test_map_str_passed_to_fn() {
    assert_aot_success(
        r#"
@sum_values (m: {str: int}) -> int = {
    let total = 0;
    for entry in m do {
        let (k, v) = entry;
        total = total + v
    };
    total
};

@main () -> int = {
    let m = {
        "a very long key string that exceeds the twenty three byte SSO threshold for testing": 10,
        "another long key string to exercise function parameter map cleanup": 20
    };
    let result = sum_values(m: m);
    if result == 30 then 0 else 1
}
"#,
        "map_str_passed_to_fn",
    );
}

// map.keys() on {str: int}

#[test]
fn test_map_keys_str_scope_drop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "a very long key string that exceeds the twenty three byte SSO threshold": 1,
        "another long key string to test map keys element cleanup properly": 2
    };
    let keys = m.keys();
    let ok = keys.len() == 2;
    if ok then 0 else 1
}
"#,
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
        r#"
@main () -> int = {
    let m: {str: int} = {};
    let m = m.insert("this is a long key that definitely exceeds the SSO threshold", 42);
    if m.len() == 1 then 0 else 1
}
"#,
        "map_insert_heap_str_key",
    );
}

// map.insert() with heap string value — exercises val_inc path.
#[test]
fn test_map_insert_heap_str_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "a very long value string that exceeds SSO threshold for sure");
    if m.len() == 1 then 0 else 1
}
"#,
        "map_insert_heap_str_value",
    );
}

// COW map insert shared — slow path with key_inc/val_inc on rehashed entries.
#[test]
fn test_map_cow_insert_shared_heap_key() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base = {"this is a heap string key exceeding SSO": 10};
    let shared = base;
    let fork = shared.insert("another very long heap string key here", 20);
    // base and fork both alive — shared entries rehashed + inc'd
    if base.len() == 1 && fork.len() == 2 then 0 else 1
}
"#,
        "map_cow_insert_shared_heap_key",
    );
}

// Map insert overwrite with heap string value — exercises val_dec on old value.
// TPR-02-012: cow_insert_existing fast path (unique) must dec the old value
// before overwriting with the new one.
#[test]
fn test_map_insert_overwrite_heap_str_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "first value is a heap string that exceeds twenty three bytes SSO");
    let m = m.insert(1, "second value is also a heap string that exceeds SSO threshold");
    if m.len() == 1 then 0 else 1
}
"#,
        "map_insert_overwrite_heap_str_value",
    );
}

// Map insert overwrite with heap string value on shared map — slow path.
// Exercises slow_copy_overwrite_value.
#[test]
fn test_map_insert_overwrite_shared_heap_str_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base: {int: str} = {};
    let base = base.insert(1, "original value heap string exceeding SSO threshold here");
    let shared = base;
    let fork = shared.insert(1, "replacement value heap string exceeding SSO threshold");
    // base still has original value, fork has replacement
    if base.len() == 1 && fork.len() == 1 then 0 else 1
}
"#,
        "map_insert_overwrite_shared_heap_str_value",
    );
}

// Map insert overwrite multiple times — exercises repeated val_dec on unique map.
#[test]
fn test_map_insert_overwrite_multiple_heap_str_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "first value that is a long heap string for testing purposes");
    let m = m.insert(1, "second value that is a long heap string for testing purposes");
    let m = m.insert(1, "third value that is a long heap string for testing purposes");
    if m.len() == 1 then 0 else 1
}
"#,
        "map_insert_overwrite_multiple_heap_str_value",
    );
}

// str.split(sep:) returning [str]

#[test]
fn test_str_split_scope_drop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let text = "this is a very long string that exceeds the SSO threshold of twenty three bytes and has spaces";
    let parts = text.split(sep: " ");
    let ok = parts.len() > 1;
    if ok then 0 else 1
}
"#,
        "str_split_scope_drop",
    );
}

// 02.2 Iterator creation and drop — header-based elem_dec_fn verification

// Iterator is last owner: words not used after loop, ARC may dec words
// before loop ends. Iterator Drop triggers cleanup via header elem_dec_fn.
#[test]
fn test_str_list_iter_last_owner() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing iterator as last buffer owner cleanup"
    ];
    let count = 0;
    for w in words do {
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "str_list_iter_last_owner",
    );
}

// Explicit dec is last owner: words used after loop, so ARC keeps it alive.
// Iterator drops at loop end (RC 2→1), words drops at block exit (RC 1→0).
#[test]
fn test_str_list_explicit_last_owner() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing explicit dec as last buffer owner"
    ];
    let count = 0;
    for w in words do {
        count = count + 1;
    };
    // Use words after loop — forces ARC to keep it alive past iteration
    let $n = words.len();
    if count == 2 && n == 2 then 0 else 1
}
"#,
        "str_list_explicit_last_owner",
    );
}

// Function parameter: callee iterates [str], caller uses after return.
// Both callee's iterator dec and caller's scope dec hit ori_buffer_rc_dec.
// store_elem_dec_fn_once ensures header is populated by whichever fires first.
#[test]
fn test_str_list_fn_param_iter() {
    assert_aot_success(
        r#"
@count_words (words: [str]) -> int = {
    let count = 0;
    for _ in words do {
        count = count + 1;
    };
    count
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing function parameter iteration cleanup"
    ];
    let $n = count_words(words:);
    // words still alive after call — caller retains ownership
    let $wlen = words.len();
    if n == 2 && wlen == 2 then 0 else 1
}
"#,
        "str_list_fn_param_iter",
    );
}

// Slice + iteration: create [str], take a slice, iterate the original list.
// Both slice and iterator share the SAME buffer's header. store_elem_dec_fn_once
// ensures the header is populated once. Whichever is last owner triggers cleanup.
#[test]
fn test_str_list_slice_then_iter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing slice plus iteration interaction",
        "third heap allocated string to verify slice and iterator share header correctly"
    ];
    let $first_two = words.take(count: 2);
    // first_two is a seamless slice sharing words' buffer
    let count = 0;
    for w in words do {
        count = count + 1;
    };
    let $ft_len = first_two.len();
    if count == 3 && ft_len == 2 then 0 else 1
}
"#,
        "str_list_slice_then_iter",
    );
}

// Set<str> scope drop and iteration

#[test]
fn test_set_str_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a long heap string exceeding SSO threshold for testing purposes",
        "another long heap string to verify set iteration cleanup works correctly"
    ].iter().collect();
    let count = 0;
    for item in s do {
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "set_str_iteration",
    );
}

#[test]
fn test_set_str_passed_to_fn() {
    assert_aot_success(
        r#"
@count_items (items: Set<str>) -> int = {
    let count = 0;
    for _ in items do {
        count = count + 1;
    };
    count
}

@main () -> int = {
    let s: Set<str> = [
        "this is a long heap string exceeding SSO threshold for testing purposes",
        "another long heap string to verify set function parameter cleanup works"
    ].iter().collect();
    let n = count_items(items: s);
    if n == 2 then 0 else 1
}
"#,
        "set_str_passed_to_fn",
    );
}

#[test]
fn test_set_str_cow_insert_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base: Set<str> = [
        "this is a long heap string exceeding SSO threshold for testing purposes"
    ].iter().collect();
    let shared = base;
    let fork = shared.insert("another long heap string to verify set COW insert cleanup");
    // base has 1 element, fork has 2 — both alive, shared buffer rehashed
    if base.len() == 1 && fork.len() == 2 then 0 else 1
}
"#,
        "set_str_cow_insert_shared",
    );
}

#[test]
fn test_set_str_union() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<str> = [
        "alpha long heap string exceeding SSO threshold for testing purposes"
    ].iter().collect();
    let b: Set<str> = [
        "beta long heap string exceeding SSO threshold for testing purposes"
    ].iter().collect();
    let u = a.union(other: b);
    if u.len() == 2 then 0 else 1
}
"#,
        "set_str_union",
    );
}

#[test]
fn test_set_str_to_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a long heap string exceeding SSO threshold for testing purposes",
        "another long heap string to verify set to_list creates proper list buffer"
    ].iter().collect();
    let items = s.to_list();
    if items.len() == 2 then 0 else 1
}
"#,
        "set_str_to_list",
    );
}

// Set<str> remove — fat-pointer element cleanup on remove (TPR-02-013)

#[test]
fn test_set_str_remove_remaining() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for sure",
        "another long heap allocated string to verify remove cleans up properly",
        "third heap string in the set to check remaining elements after removal"
    ].iter().collect();
    let s2 = s.remove(value: "another long heap allocated string to verify remove cleans up properly");
    if s2.len() == 2 then 0 else 1
}
"#,
        "set_str_remove_remaining",
    );
}

#[test]
fn test_set_str_remove_last_element() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for testing"
    ].iter().collect();
    let s2 = s.remove(value: "this is a very long heap string that exceeds the SSO threshold for testing");
    if s2.len() == 0 then 0 else 1
}
"#,
        "set_str_remove_last",
    );
}

// Set<str> intersection — fat-pointer element cleanup on filter (TPR-02-014)

#[test]
fn test_set_str_intersection_unique() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s1: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for sure",
        "another long heap allocated string for intersection testing purposes here",
        "third heap string only in set1 should be cleaned up during intersection"
    ].iter().collect();
    let s2: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for sure"
    ].iter().collect();
    let result = s1.intersection(other: s2);
    if result.len() == 1 then 0 else 1
}
"#,
        "set_str_intersection_unique",
    );
}

#[test]
fn test_set_str_difference_unique() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s1: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for sure",
        "another long heap allocated string for difference testing purposes here",
        "third heap string only in set1 should remain after difference operation"
    ].iter().collect();
    let s2: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for sure"
    ].iter().collect();
    let result = s1.difference(other: s2);
    if result.len() == 2 then 0 else 1
}
"#,
        "set_str_difference_unique",
    );
}

// Map remove — fat-pointer key/value cleanup (discovered during TPR-02-013)

#[test]
fn test_map_remove_str_key() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long heap string that exceeds the SSO threshold for sure": 1,
        "another long heap allocated string for map remove testing purposes here": 2,
        "third heap string key to check remaining entries after removal operation": 3
    };
    let m2 = m.remove(key: "another long heap allocated string for map remove testing purposes here");
    if m2.len() == 2 then 0 else 1
}
"#,
        "map_remove_str_key",
    );
}

#[test]
fn test_map_remove_str_key_last() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long heap string that exceeds the SSO threshold for testing": 42
    };
    let m2 = m.remove(key: "this is a very long heap string that exceeds the SSO threshold for testing");
    if m2.len() == 0 then 0 else 1
}
"#,
        "map_remove_str_key_last",
    );
}

// TPR-02-015: Shared set remove double-dec — removing last element from aliased set

#[test]
fn test_set_str_remove_last_shared() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for testing"
    ].iter().collect();
    let s2 = s;
    let empty = s2.remove(value: "this is a very long heap string that exceeds the SSO threshold for testing");
    if s.len() == 1 && empty.len() == 0 then 0 else 1
}
"#,
        "set_str_remove_last_shared",
    );
}

#[test]
fn test_set_str_remove_last_shared_only_alias_survives() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long heap string that exceeds the SSO threshold for testing"
    ].iter().collect();
    let s2 = s;
    let empty = s.remove(value: "this is a very long heap string that exceeds the SSO threshold for testing");
    if s2.len() == 1 && empty.len() == 0 then 0 else 1
}
"#,
        "set_str_remove_last_shared_alias_survives",
    );
}

// Set<str> collect via ori_iter_collect_set — exercises elem_inc_fn on set elements.

#[test]
fn test_set_str_iter_collect() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long heap string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for set collect testing purposes in AOT test",
        "third heap allocated string to verify ori_iter_collect_set elem_inc_fn"
    ];
    let s: Set<str> = words.iter().collect();
    if s.len() == 3 then 0 else 1
}
"#,
        "set_str_iter_collect",
    );
}

// TPR-02-018: map.values() with fat-pointer values — exercises val_inc_fn path in
// ori_map_values_to_list. Previous tests only used {str: int} (primitive values).

#[test]
fn test_map_values_heap_str_values() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "this is a very long heap string value that exceeds the SSO threshold for sure");
    let m = m.insert(2, "another long heap allocated string value for map values testing purposes");
    let m = m.insert(3, "third heap string value to check all elements are inc'd by val_inc_fn");
    let vals = m.values();
    if vals.len() == 3 then 0 else 1
}
"#,
        "map_values_heap_str_values",
    );
}

#[test]
fn test_map_values_str_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {str: str} = {};
    let m = m.insert("key one is a heap string that exceeds SSO threshold", "val one is a heap string that exceeds SSO threshold");
    let m = m.insert("key two is a heap string that exceeds SSO threshold", "val two is a heap string that exceeds SSO threshold");
    let vals = m.values();
    let keys = m.keys();
    if vals.len() == 2 && keys.len() == 2 then 0 else 1
}
"#,
        "map_values_str_str",
    );
}

// TPR-02-019: map remove with fat-pointer values — exercises val_dec branches in
// ori_map_remove_cow (cow.rs:373 empty sentinel path, cow.rs:391 unique fast path).

#[test]
fn test_map_remove_heap_str_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "first heap string value that exceeds the SSO threshold of twenty three");
    let m = m.insert(2, "second heap string value that exceeds the SSO threshold for testing");
    let m = m.insert(3, "third heap string value that exceeds the SSO threshold for removal");
    let m2 = m.remove(key: 2);
    if m2.len() == 2 then 0 else 1
}
"#,
        "map_remove_heap_str_value",
    );
}

#[test]
fn test_map_remove_heap_str_value_last() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "this is a very long heap string value that exceeds the SSO threshold");
    let m2 = m.remove(key: 1);
    if m2.len() == 0 then 0 else 1
}
"#,
        "map_remove_heap_str_value_last",
    );
}
