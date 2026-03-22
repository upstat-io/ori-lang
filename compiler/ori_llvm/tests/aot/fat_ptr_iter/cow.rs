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
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let result: [str] = [];
    for w in words do {
        result = result.push(value: w);
    };
    let total = 0;
    for w in result do {
        total = total + w.len();
    };
    if total == 109 then 0 else 1
}
"#,
        "push_element_in_for_loop",
    );
}

#[test]
fn test_push_element_borrowed_param() {
    // Borrowed param: push elements from one list into another.
    assert_aot_success(
        r#"
@collect_words (words: [str]) -> [str] = {
    let result: [str] = [];
    for w in words do {
        result = result.push(value: w);
    };
    result
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let collected = collect_words(words: words);
    let total = 0;
    for w in collected do {
        total = total + w.len();
    };
    // Original still valid
    let total2 = 0;
    for w in words do {
        total2 = total2 + w.len();
    };
    if total == 109 && total2 == 109 then 0 else 1
}
"#,
        "push_element_borrowed_param",
    );
}

#[test]
fn test_push_element_borrowed_param_two_calls() {
    // Two calls: collect from same borrowed list twice.
    assert_aot_success(
        r#"
@collect_words (words: [str]) -> [str] = {
    let result: [str] = [];
    for w in words do {
        result = result.push(value: w);
    };
    result
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = collect_words(words: words);
    let b = collect_words(words: words);
    let sa = 0;
    for w in a do { sa = sa + w.len(); };
    let sb = 0;
    for w in b do { sb = sb + w.len(); };
    if sa == 109 && sb == 109 then 0 else 1
}
"#,
        "push_element_borrowed_param_two_calls",
    );
}

// T2-F11: [[int]] COW push on shared list — inner list elements are RC-managed.

#[test]
fn test_nested_list_cow_push() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [[1, 2], [3, 4]];
    let copy = lists;
    copy = copy.push(value: [5, 6]);
    let orig_sum = 0;
    for inner in lists do {
        for n in inner do { orig_sum = orig_sum + n; };
    };
    let copy_sum = 0;
    for inner in copy do {
        for n in inner do { copy_sum = copy_sum + n; };
    };
    // orig: 1+2+3+4=10, copy: 1+2+3+4+5+6=21
    if orig_sum == 10 && copy_sum == 21 then 0 else 1
}
"#,
        "nested_list_cow_push",
    );
}

// T9-F11: Set<str> remove on shared set — elem_dec_fn called before tombstoning.

#[test]
fn test_set_str_cow_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ].iter().collect();
    let copy = s;
    copy = copy.remove(value: "this is a very long string that exceeds SSO threshold");
    let orig_count = 0;
    for item in s do { orig_count = orig_count + 1; };
    let copy_count = 0;
    for item in copy do { copy_count = copy_count + 1; };
    if orig_count == 2 && copy_count == 1 then 0 else 1
}
"#,
        "set_str_cow_remove",
    );
}

// T8-F11: {str: int} map insert overwriting existing key — old value cleaned up.

#[test]
fn test_map_str_insert_overwrite() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let copy = m;
    copy = copy.insert(key: "this is a very long key exceeding SSO threshold here", value: 99);
    let orig_total = 0;
    for (k, v) in m do { orig_total = orig_total + v; };
    let copy_total = 0;
    for (k, v) in copy do { copy_total = copy_total + v; };
    // orig: 10+20=30, copy: 99+20=119
    if orig_total == 30 && copy_total == 119 then 0 else 1
}
"#,
        "map_str_insert_overwrite",
    );
}

// T8-F11: {str: int} map remove on shared map — key_dec_fn/val_dec_fn called.

#[test]
fn test_map_str_cow_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let copy = m;
    copy = copy.remove(key: "this is a very long key exceeding SSO threshold here");
    let orig_count = 0;
    for (k, v) in m do { orig_count = orig_count + 1; };
    let copy_count = 0;
    for (k, v) in copy do { copy_count = copy_count + 1; };
    if orig_count == 2 && copy_count == 1 then 0 else 1
}
"#,
        "map_str_cow_remove",
    );
}
