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
