//! Fat pointer iteration tests.
//!
//! Tests for iterating over collections whose elements require Drop
//! (str, [T], closures, structs with Drop fields). These tests verify
//! that element-level RC cleanup happens correctly — no leaks, no
//! double-frees.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// -----------------------------------------------------------------------
// [str] iteration — heap strings (exceeds SSO threshold of 23 bytes)
// -----------------------------------------------------------------------

#[test]
fn test_str_list_full_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    if total == 109 then 0 else 1
}
"#,
        "str_list_full_iteration",
    );
}

#[test]
fn test_str_list_partial_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "stop marker string that is also long enough to be heap",
        "third long string that should not be visited early break"
    ];
    let count = 0;
    for w in words do {
        if w.starts_with(prefix: "stop") then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "str_list_partial_break",
    );
}

#[test]
fn test_str_list_yield_lengths() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = for w in words yield w.len();
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    if total == 109 then 0 else 1
}
"#,
        "str_list_yield_lengths",
    );
}

#[test]
fn test_str_list_passed_to_two_functions() {
    assert_aot_success(
        r#"
@count_chars (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = count_chars(words: words);
    let b = count_chars(words: words);
    if a == 109 then {
        if b == 109 then 0 else 1
    } else 1
}
"#,
        "str_list_passed_to_two_functions",
    );
}

// -----------------------------------------------------------------------
// Borrowed param × iteration mode matrix
//
// Dimension 1 — Call count: 1, 2, N (loop)
// Dimension 2 — Iteration mode: full do, partial break, yield
// Dimension 3 — Element type: [str], [int], struct with str field
// Dimension 4 — Caller context: own, COW after, chained callee
// -----------------------------------------------------------------------

// --- Single call × full iteration ---

#[test]
fn test_borrowed_str_list_single_call() {
    assert_aot_success(
        r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let r = sum_lens(words: words);
    if r == 109 then 0 else 1
}
"#,
        "borrowed_str_list_single_call",
    );
}

#[test]
fn test_borrowed_int_list_single_call() {
    // [int] has scalar elements — no element-level RC, but the list buffer
    // itself is RC-managed. Verifies the fix doesn't break scalar iteration.
    assert_aot_success(
        r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let xs = [10, 20, 30, 40];
    let r = sum_list(xs: xs);
    if r == 100 then 0 else 1
}
"#,
        "borrowed_int_list_single_call",
    );
}

#[test]
fn test_borrowed_struct_list_single_call() {
    // Struct with str field — element-level Drop involves field traversal.
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@sum_values (items: [Item]) -> int = {
    let total = 0;
    for item in items do {
        total = total + item.value;
    };
    total
}

@main () -> int = {
    let items = [
        Item { label: "this is a very long label exceeding SSO threshold", value: 10 },
        Item { label: "another very long label also exceeding SSO threshold", value: 20 }
    ];
    let r = sum_values(items: items);
    if r == 30 then 0 else 1
}
"#,
        "borrowed_struct_list_single_call",
    );
}

// --- Two sequential calls (the original bug scenario) ---

#[test]
fn test_borrowed_str_list_two_calls() {
    // This was the original double-free: two calls to same function with
    // borrowed [str] param. Second call used freed memory.
    assert_aot_success(
        r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = sum_lens(words: words);
    let b = sum_lens(words: words);
    if a == 109 && b == 109 then 0 else 1
}
"#,
        "borrowed_str_list_two_calls",
    );
}

#[test]
fn test_borrowed_int_list_two_calls() {
    assert_aot_success(
        r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let xs = [10, 20, 30, 40];
    let a = sum_list(xs: xs);
    let b = sum_list(xs: xs);
    if a == 100 && b == 100 then 0 else 1
}
"#,
        "borrowed_int_list_two_calls",
    );
}

// --- N calls in a loop ---

#[test]
fn test_borrowed_str_list_called_in_loop() {
    // Call the borrowing function multiple times from a for loop.
    // Stresses RC accounting across repeated borrows.
    assert_aot_success(
        r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let acc = 0;
    for _ in 0..5 do {
        acc = acc + sum_lens(words: words);
    };
    if acc == 545 then 0 else 1
}
"#,
        "borrowed_str_list_called_in_loop",
    );
}

// --- Partial iteration (break) with borrowed param ---

#[test]
fn test_borrowed_str_list_partial_break_two_calls() {
    // Break mid-iteration, then call again. Verifies unconsumed elements
    // are not leaked and the list is still valid for the second call.
    assert_aot_success(
        r#"
@count_until_stop (words: [str]) -> int = {
    let count = 0;
    for w in words do {
        if w.starts_with(prefix: "stop") then break;
        count = count + 1;
    };
    count
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "stop marker string that is also long enough to be heap",
        "third long string that should not be visited early break"
    ];
    let a = count_until_stop(words: words);
    let b = count_until_stop(words: words);
    if a == 1 && b == 1 then 0 else 1
}
"#,
        "borrowed_str_list_partial_break_two_calls",
    );
}

// --- Yield with borrowed param ---

#[test]
fn test_borrowed_str_list_yield_two_calls() {
    // Yield from borrowed param iteration, call twice.
    assert_aot_success(
        r#"
@get_lengths (words: [str]) -> [int] = {
    for w in words yield w.len()
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = get_lengths(words: words);
    let b = get_lengths(words: words);
    let sum_a = 0;
    for n in a do { sum_a = sum_a + n; };
    let sum_b = 0;
    for n in b do { sum_b = sum_b + n; };
    if sum_a == 109 && sum_b == 109 then 0 else 1
}
"#,
        "borrowed_str_list_yield_two_calls",
    );
}

// --- COW after borrowed call ---

#[test]
fn test_borrowed_param_then_cow_mutation() {
    // Pass list to borrowing function, then mutate with COW.
    // Verifies RC is correct for the COW copy-on-write path.
    assert_aot_success(
        r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let xs = [1, 2, 3, 4];
    let $before = sum_list(xs: xs);
    xs = xs.push(99);
    let $after = sum_list(xs: xs);
    if before == 10 && after == 109 then 0 else 1
}
"#,
        "borrowed_param_then_cow_mutation",
    );
}

// --- Chained callees: A calls B which iterates ---

#[test]
fn test_chained_borrowed_callee() {
    // main → wrapper → iterate. The list passes through two borrowed params.
    assert_aot_success(
        r#"
@iterate_words (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@wrapper (words: [str]) -> int = {
    iterate_words(words: words)
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = wrapper(words: words);
    let b = wrapper(words: words);
    if a == 109 && b == 109 then 0 else 1
}
"#,
        "chained_borrowed_callee",
    );
}

// --- Borrowed param: iterate + use list after loop ---

#[test]
fn test_borrowed_param_use_after_iteration() {
    // Use the borrowed list AFTER the for loop completes.
    // Verifies the list is still valid post-iteration.
    assert_aot_success(
        r#"
@process (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total + words.len()
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = process(words: words);
    let b = process(words: words);
    if a == 111 && b == 111 then 0 else 1
}
"#,
        "borrowed_param_use_after_iteration",
    );
}

// --- Borrowed param: two different lists passed to same function ---

#[test]
fn test_two_different_borrowed_lists() {
    assert_aot_success(
        r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let a = [
        "this is a very long string that exceeds SSO threshold"
    ];
    let b = [
        "another very long string that also exceeds the threshold"
    ];
    let ra = sum_lens(words: a);
    let rb = sum_lens(words: b);
    let ra2 = sum_lens(words: a);
    if ra == 53 && rb == 56 && ra2 == 53 then 0 else 1
}
"#,
        "two_different_borrowed_lists",
    );
}

// --- Borrowed param: map iteration with string keys ---

#[test]
fn test_borrowed_map_str_keys_two_calls() {
    assert_aot_success(
        r#"
@sum_values (m: {str: int}) -> int = {
    let total = 0;
    for (k, v) in m do {
        total = total + v;
    };
    total
}

@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let a = sum_values(m: m);
    let b = sum_values(m: m);
    if a == 30 && b == 30 then 0 else 1
}
"#,
        "borrowed_map_str_keys_two_calls",
    );
}

// --- Combined scenarios: borrowed param + other features ---

#[test]
fn test_borrowed_param_break_then_full_iteration() {
    // First call breaks early, second iterates fully. Verifies the list
    // is intact after a partial iteration via borrowed param.
    assert_aot_success(
        r#"
@count_chars (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@first_len (words: [str]) -> int = {
    let result = 0;
    for w in words do {
        result = w.len();
        break;
    };
    result
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let first = first_len(words: words);
    let total = count_chars(words: words);
    if first == 53 && total == 109 then 0 else 1
}
"#,
        "borrowed_param_break_then_full_iteration",
    );
}

#[test]
fn test_borrowed_param_yield_then_iterate_result() {
    // yield from borrowed param produces a new list, then iterate that too.
    assert_aot_success(
        r#"
@get_lengths (words: [str]) -> [int] = {
    for w in words yield w.len()
}

@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = get_lengths(words: words);
    let total = sum_list(xs: lengths);
    let total2 = sum_list(xs: lengths);
    if total == 109 && total2 == 109 then 0 else 1
}
"#,
        "borrowed_param_yield_then_iterate_result",
    );
}

#[test]
fn test_borrowed_struct_list_two_calls_with_field_access() {
    // Struct with str field, called twice. Exercises element-level Drop
    // through field traversal on a borrowed collection.
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@total_values (items: [Item]) -> int = {
    let total = 0;
    for item in items do {
        total = total + item.value;
    };
    total
}

@main () -> int = {
    let items = [
        Item { label: "this is a very long label exceeding SSO threshold", value: 10 },
        Item { label: "another very long label also exceeding SSO threshold", value: 20 }
    ];
    let a = total_values(items: items);
    let b = total_values(items: items);
    if a == 30 && b == 30 then 0 else 1
}
"#,
        "borrowed_struct_list_two_calls_with_field_access",
    );
}

#[test]
fn test_borrowed_param_mixed_callers() {
    // Same function called from two different callers with different lists.
    // Verifies no cross-contamination of borrowed references.
    assert_aot_success(
        r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@call_twice_a (words: [str]) -> int = {
    sum_lens(words: words) + sum_lens(words: words)
}

@call_twice_b (words: [str]) -> int = {
    sum_lens(words: words) + sum_lens(words: words)
}

@main () -> int = {
    let a = ["this is a very long string that exceeds SSO threshold"];
    let b = ["another very long string that also exceeds the threshold"];
    let ra = call_twice_a(words: a);
    let rb = call_twice_b(words: b);
    if ra == 106 && rb == 112 then 0 else 1
}
"#,
        "borrowed_param_mixed_callers",
    );
}

#[test]
fn test_borrowed_param_iterate_then_index() {
    // Iterate borrowed list, then index into it. Both accesses in same callee.
    assert_aot_success(
        r#"
@process (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    let first = words[0];
    total + first.len()
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = process(words: words);
    let b = process(words: words);
    if a == 162 && b == 162 then 0 else 1
}
"#,
        "borrowed_param_iterate_then_index",
    );
}

#[test]
fn test_borrowed_empty_list_iteration() {
    // Edge case: empty list passed to borrowing iterator function.
    assert_aot_success(
        r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let empty: [str] = [];
    let a = sum_lens(words: empty);
    let b = sum_lens(words: empty);
    if a == 0 && b == 0 then 0 else 1
}
"#,
        "borrowed_empty_list_iteration",
    );
}

// -----------------------------------------------------------------------
// [[int]] nested iteration — inner lists are RC-managed
// -----------------------------------------------------------------------

#[test]
fn test_nested_list_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [[1, 2, 3], [4, 5, 6]];
    let total = 0;
    for inner in lists do {
        for n in inner do {
            total = total + n;
        };
    };
    if total == 21 then 0 else 1
}
"#,
        "nested_list_iteration",
    );
}

// -----------------------------------------------------------------------
// Map iteration with string keys/values
// -----------------------------------------------------------------------

#[test]
fn test_map_str_key_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let total = 0;
    for (k, v) in m do {
        total = total + v;
    };
    if total == 30 then 0 else 1
}
"#,
        "map_str_key_iteration",
    );
}

// -----------------------------------------------------------------------
// Struct with string fields
// -----------------------------------------------------------------------

#[test]
fn test_struct_with_str_field_iteration() {
    assert_aot_success(
        r#"
type Person = { name: str, age: int }

@main () -> int = {
    let people = [
        Person { name: "this is a very long name exceeding SSO threshold", age: 30 },
        Person { name: "another very long name exceeding SSO threshold too", age: 25 }
    ];
    let total_age = 0;
    for p in people do {
        total_age = total_age + p.age;
    };
    if total_age == 55 then 0 else 1
}
"#,
        "struct_with_str_field_iteration",
    );
}

// -----------------------------------------------------------------------
// Yield identity — for w in words yield w (fat pointer escapes via yield)
//
// The loop variable `w` is borrowed from the iterator. When yielded
// directly (not transformed to a scalar), the element escapes the
// iterator's borrow scope. The ARC pipeline must emit RcInc on `w`
// before passing it to ori_list_push, otherwise the new list holds
// an un-owned reference.
// -----------------------------------------------------------------------

#[test]
fn test_yield_identity_str_list() {
    // `for w in words yield w` — yields the actual string element
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = for w in words yield w;
    let total = 0;
    for w in copy do {
        total = total + w.len();
    };
    if total == 109 then 0 else 1
}
"#,
        "yield_identity_str_list",
    );
}

#[test]
fn test_yield_identity_str_list_borrowed_param() {
    // Borrowed param + yield identity: the borrowed [str] is iterated
    // and each element is yielded into a new list.
    assert_aot_success(
        r#"
@clone_list (words: [str]) -> [str] = {
    for w in words yield w
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = clone_list(words: words);
    let total = 0;
    for w in copy do {
        total = total + w.len();
    };
    // Verify original is still valid
    let total2 = 0;
    for w in words do {
        total2 = total2 + w.len();
    };
    if total == 109 && total2 == 109 then 0 else 1
}
"#,
        "yield_identity_str_list_borrowed_param",
    );
}

#[test]
fn test_yield_identity_str_list_two_calls() {
    // Two calls to a function that does yield identity on borrowed [str].
    // Stresses RC: original must survive both clones.
    assert_aot_success(
        r#"
@clone_list (words: [str]) -> [str] = {
    for w in words yield w
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = clone_list(words: words);
    let b = clone_list(words: words);
    let sa = 0;
    for w in a do { sa = sa + w.len(); };
    let sb = 0;
    for w in b do { sb = sb + w.len(); };
    if sa == 109 && sb == 109 then 0 else 1
}
"#,
        "yield_identity_str_list_two_calls",
    );
}

// -----------------------------------------------------------------------
// Push element into another collection — for w in words do { ... push ... }
//
// Similar to yield identity: the loop element `w` is borrowed from the
// iterator. When pushed into another list, it escapes the borrow scope.
// ARC pipeline must RcInc the element before the consuming push call.
// -----------------------------------------------------------------------

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

// -----------------------------------------------------------------------
// Unwind path — panic during iteration with catch recovery
// -----------------------------------------------------------------------

/// Panic during iteration of [str] — catch recovers, no leak or double-free.
///
/// Exercises: Invoke unwind cleanup for borrowed list parameter inside
/// for-loop body, iterator drop on unwind, catch(expr:) recovery.
#[test]
fn test_unwind_panic_during_str_iteration() {
    assert_aot_success(
        r#"
@might_panic (s: str) -> int = {
    if s == "boom this is a long enough string for heap allocation" then {
        panic(msg: "kaboom during iteration")
    };
    s.len()
}

@process (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + might_panic(s: w)
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should never be reached at all"
    ];
    let result = catch(expr: process(words: words));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_panic_during_str_iteration",
    );
}

/// Panic during iteration, then reuse the list — verifies RC is correct
/// after unwind (list still accessible, no double-free).
#[test]
fn test_unwind_list_reusable_after_catch() {
    assert_aot_success(
        r#"
@panicking_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w == "boom this is a long enough string for heap allocation" then {
            panic(msg: "kaboom")
        };
        total = total + w.len()
    };
    total
}

@safe_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should also be on the heap here"
    ];
    let r1 = catch(expr: panicking_iter(words: words));
    let r2 = safe_iter(words: words);
    match r1 {
        Ok(_) -> 1,
        Err(_) -> if r2 == 160 then 0 else 1
    }
}
"#,
        "unwind_list_reusable_after_catch",
    );
}

/// Multiple invoke calls in one function — panic at second call, verify
/// cleanup is correct for both call sites.
#[test]
fn test_unwind_multiple_invokes_with_panic() {
    assert_aot_success(
        r#"
@count_lengths (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len()
    };
    total
}

@panicking_count (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w == "boom this is a long enough string for heap allocation" then {
            panic(msg: "kaboom in second call")
        };
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should also be on the heap here"
    ];
    let r1 = count_lengths(words: words);
    let r2 = catch(expr: panicking_count(words: words));
    match r2 {
        Ok(_) -> 1,
        Err(_) -> if r1 == 160 then 0 else 1
    }
}
"#,
        "unwind_multiple_invokes_with_panic",
    );
}

/// Panic inside nested function call chain — A calls B calls C, C panics.
/// Verifies unwind cleanup propagates correctly through multiple frames.
#[test]
fn test_unwind_nested_call_chain_panic() {
    assert_aot_success(
        r#"
@inner (s: str) -> int = {
    if s == "boom this is a long enough string for heap allocation" then {
        panic(msg: "deep panic")
    };
    s.len()
}

@middle (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + inner(s: w)
    };
    total
}

@outer (words: [str]) -> int = {
    middle(words: words)
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should also be on the heap here"
    ];
    let result = catch(expr: outer(words: words));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_nested_call_chain_panic",
    );
}

/// Partial iteration + break, then panic in separate call — verifies
/// that break cleanup and unwind cleanup are independent and correct.
#[test]
fn test_unwind_break_then_panic() {
    assert_aot_success(
        r#"
@partial_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w.len() > 60 then break;
        total = total + w.len()
    };
    total
}

@panicking_func (words: [str]) -> int = {
    panic(msg: "always panics")
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold plus",
        "third long string that should also be on the heap here now"
    ];
    let r1 = partial_iter(words: words);
    let r2 = catch(expr: panicking_func(words: words));
    match r2 {
        Ok(_) -> 1,
        Err(_) -> if r1 == 53 then 0 else 1
    }
}
"#,
        "unwind_break_then_panic",
    );
}

/// Panic at FIRST element during iteration — iterator is live but no
/// elements have been consumed yet. Verifies cleanup handles zero-consumed
/// iterator state correctly.
#[test]
fn test_unwind_panic_at_first_element() {
    assert_aot_success(
        r#"
@process (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w.starts_with(prefix: "boom") then {
            panic(msg: "panic at first element")
        };
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "boom this is a long enough string for heap allocation",
        "second very long string that also exceeds the threshold",
        "third long string that should also be on the heap here"
    ];
    let result = catch(expr: process(words: words));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_panic_at_first_element",
    );
}

/// Repeated catch/panic cycles on the same list — stresses RC balance
/// across multiple unwind/recovery sequences.
#[test]
fn test_unwind_repeated_catch_cycles() {
    assert_aot_success(
        r#"
@panicking_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w.starts_with(prefix: "boom") then {
            panic(msg: "cycle panic")
        };
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "boom this is a long enough string for heap allocation",
        "second very long string that also exceeds the threshold"
    ];
    let r1 = catch(expr: panicking_iter(words: words));
    let r2 = catch(expr: panicking_iter(words: words));
    let r3 = catch(expr: panicking_iter(words: words));
    let all_err = match r1 { Ok(_) -> false, Err(_) -> true }
        && match r2 { Ok(_) -> false, Err(_) -> true }
        && match r3 { Ok(_) -> false, Err(_) -> true };
    if all_err then 0 else 1
}
"#,
        "unwind_repeated_catch_cycles",
    );
}

/// Non-iterator local heap value in callee + panic — verifies general
/// RC cleanup for non-iterator heap variables on unwind path.
#[test]
fn test_unwind_callee_local_heap_value() {
    assert_aot_success(
        r#"
@process (input: str) -> int = {
    let local_copy = input + " extra text to ensure heap allocation";
    if local_copy.len() > 50 then {
        panic(msg: "callee local panic")
    };
    local_copy.len()
}

@main () -> int = {
    let s = "this is a very long string that exceeds SSO threshold";
    let result = catch(expr: process(input: s));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_callee_local_heap_value",
    );
}

// -----------------------------------------------------------------------
// 01.4 Generalize to All [T] Where T Has Drop
// -----------------------------------------------------------------------

/// [str] iteration — the original J15 scenario. Full iteration, break,
/// and yield over heap-allocated strings.
#[test]
fn test_generalize_str_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let total = 0;
    for w in words do {
        total = total + w.len()
    };
    if total == 109 then 0 else 1
}
"#,
        "generalize_str_list",
    );
}

/// [[int]] iteration — nested list elements are heap-allocated.
#[test]
fn test_generalize_nested_int_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[1, 2, 3], [4, 5], [6, 7, 8, 9]];
    let total = 0;
    for inner in nested do {
        for n in inner do {
            total = total + n
        }
    };
    if total == 45 then 0 else 1
}
"#,
        "generalize_nested_int_list",
    );
}

/// [(int) -> int] iteration — closures that capture heap values.
#[test]
fn test_generalize_closure_list() {
    assert_aot_success(
        r#"
@make_adder (n: int) -> (int) -> int = {
    x -> x + n
}

@main () -> int = {
    let fns = [make_adder(n: 10), make_adder(n: 20), make_adder(n: 30)];
    let total = 0;
    for f in fns do {
        total = total + f(1)
    };
    if total == 63 then 0 else 1
}
"#,
        "generalize_closure_list",
    );
}

/// [{name: str, age: int}] iteration — structs with string fields.
#[test]
fn test_generalize_struct_with_str_fields() {
    assert_aot_success(
        r#"
type Person = { name: str, age: int }

@main () -> int = {
    let people = [
        Person { name: "alice with a very long name for heap allocation", age: 30 },
        Person { name: "bob with another very long name for heap allocation", age: 25 }
    ];
    let total_age = 0;
    for p in people do {
        total_age = total_age + p.age
    };
    if total_age == 55 then 0 else 1
}
"#,
        "generalize_struct_with_str_fields",
    );
}

/// [Option<str>] iteration — sum types with fat pointer payloads.
#[test]
fn test_generalize_option_str_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string that also exceeds the threshold")
    ];
    let count = 0;
    for item in items do {
        match item {
            Some(_) -> { count = count + 1 },
            None -> ()
        }
    };
    if count == 2 then 0 else 1
}
"#,
        "generalize_option_str_list",
    );
}

/// Partially consumed [str] with break — consumed and unconsumed elements
/// must both be correctly cleaned up.
#[test]
fn test_generalize_partial_break_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "stop marker string that is also long enough to be heap",
        "third long string that should not be visited early break"
    ];
    let count = 0;
    for w in words do {
        if w.starts_with(prefix: "stop") then break;
        count = count + 1
    };
    if count == 1 then 0 else 1
}
"#,
        "generalize_partial_break_str",
    );
}

/// `for w in words yield w.len()` — yield consumes each element value.
#[test]
fn test_generalize_yield_str_lengths() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = for w in words yield w.len();
    let total = 0;
    for n in lengths do {
        total = total + n
    };
    if total == 109 then 0 else 1
}
"#,
        "generalize_yield_str_lengths",
    );
}

/// [str] passed to TWO functions — verifies list RC increment on second
/// call preserves elements for both iteration passes.
#[test]
fn test_generalize_str_list_two_calls() {
    assert_aot_success(
        r#"
@count_lengths (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let r1 = count_lengths(words: words);
    let r2 = count_lengths(words: words);
    if r1 == 109 && r2 == 109 then 0 else 1
}
"#,
        "generalize_str_list_two_calls",
    );
}

/// String iteration — `for c in s` where `s: str`. `IterState::Str` owns
/// its data via `owns_data` flag.
#[test]
fn test_generalize_string_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world this is a very long string for heap allocation";
    let count = 0;
    for c in s do {
        count = count + 1
    };
    if count == 58 then 0 else 1
}
"#,
        "generalize_string_iteration",
    );
}

// -----------------------------------------------------------------------
// Regression matrix: element type × iteration pattern
//
// Covers the cross-product of element types (str, [int], closures,
// structs, Option<str>) × patterns (full, break, yield, two-call,
// nested) to prevent regressions in the iterator element ownership
// contract. Each test runs with ORI_CHECK_LEAKS=1 (via assert_aot_success).
// -----------------------------------------------------------------------

/// [[int]] with partial break — inner list elements and outer list
/// must all be freed correctly even when inner loop is partially consumed.
#[test]
fn test_matrix_nested_list_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[10, 20, 30], [40, 50], [60, 70, 80, 90]];
    let total = 0;
    for inner in nested do {
        for n in inner do {
            if n > 50 then break;
            total = total + n
        }
    };
    if total == 150 then 0 else 1
}
"#,
        "matrix_nested_list_break",
    );
}

/// [[int]] passed to two functions — verifies RC balance across multiple
/// iteration passes over nested collections.
#[test]
fn test_matrix_nested_list_two_calls() {
    assert_aot_success(
        r#"
@sum_nested (lists: [[int]]) -> int = {
    let total = 0;
    for inner in lists do {
        for n in inner do {
            total = total + n
        }
    };
    total
}

@main () -> int = {
    let nested = [[1, 2], [3, 4], [5, 6]];
    let r1 = sum_nested(lists: nested);
    let r2 = sum_nested(lists: nested);
    if r1 == 21 && r2 == 21 then 0 else 1
}
"#,
        "matrix_nested_list_two_calls",
    );
}

/// [[int]] with yield — outer for-yield produces lengths of inner lists.
#[test]
fn test_matrix_nested_list_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[1, 2, 3], [4, 5], [6]];
    let lengths = for inner in nested yield inner.len();
    let total = 0;
    for n in lengths do {
        total = total + n
    };
    if total == 6 then 0 else 1
}
"#,
        "matrix_nested_list_yield",
    );
}

/// Struct with Drop field + break — partially consumed iteration.
#[test]
fn test_matrix_struct_break() {
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@main () -> int = {
    let items = [
        Item { label: "first long label exceeding SSO threshold here", value: 10 },
        Item { label: "second long label also exceeding SSO threshold", value: 20 },
        Item { label: "third long label should not be reached at all", value: 30 }
    ];
    let total = 0;
    for item in items do {
        if item.value > 15 then break;
        total = total + item.value
    };
    if total == 10 then 0 else 1
}
"#,
        "matrix_struct_break",
    );
}

/// Struct with Drop field + yield — extracts labels from structs.
#[test]
fn test_matrix_struct_yield() {
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@main () -> int = {
    let items = [
        Item { label: "first long label exceeding SSO threshold here", value: 10 },
        Item { label: "second long label also exceeding SSO threshold", value: 20 }
    ];
    let values = for item in items yield item.value;
    let total = 0;
    for v in values do {
        total = total + v
    };
    if total == 30 then 0 else 1
}
"#,
        "matrix_struct_yield",
    );
}

/// Closure list + break — partially consumed closure iteration.
#[test]
fn test_matrix_closure_break() {
    assert_aot_success(
        r#"
@make_fn (n: int) -> (int) -> int = {
    x -> x + n
}

@main () -> int = {
    let fns = [make_fn(n: 100), make_fn(n: 200), make_fn(n: 300)];
    let total = 0;
    for f in fns do {
        let result = f(1);
        if result > 150 then break;
        total = total + result
    };
    if total == 101 then 0 else 1
}
"#,
        "matrix_closure_break",
    );
}

/// Option<str> + yield — extracts string lengths from Some values.
#[test]
fn test_matrix_option_str_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string that also exceeds the threshold")
    ];
    let lengths = for item in items yield {
        match item {
            Some(s) -> s.len(),
            None -> 0
        }
    };
    let total = 0;
    for n in lengths do {
        total = total + n
    };
    if total == 109 then 0 else 1
}
"#,
        "matrix_option_str_yield",
    );
}

/// [str] + unwind during nested function + catch — most complex
/// scenario combining fat pointers, iteration, and error recovery.
#[test]
fn test_matrix_str_nested_unwind() {
    assert_aot_success(
        r#"
@process_word (w: str) -> int = {
    if w.starts_with(prefix: "panic") then {
        panic(msg: "word triggered panic")
    };
    w.len()
}

@process_all (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + process_word(w: w)
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "panic trigger long string for heap allocation needed",
        "unreachable very long string that is also on the heap"
    ];
    let r1 = catch(expr: process_all(words: words));
    let r2 = process_all(words: [
        "first safe long string exceeding SSO threshold here",
        "second safe long string also exceeding the threshold"
    ]);
    let caught = match r1 { Ok(_) -> false, Err(_) -> true };
    if caught && r2 == 103 then 0 else 1
}
"#,
        "matrix_str_nested_unwind",
    );
}

// -----------------------------------------------------------------------
// For-yield with non-scalar elements — elem_dec_fn correctness
// -----------------------------------------------------------------------

/// `[str]` for-yield identity — borrowed str elements yielded into new list.
/// Verifies `elem_dec_fn` correctly cleans up source list elements when the
/// iterator drops, while the result list owns its own copies.
#[test]
fn test_for_yield_str_identity() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copied = for w in words yield w;
    let expected = 53 + 56;
    if copied[0].length() + copied[1].length() == expected then 0 else 1
}
"#,
        "for_yield_str_identity",
    );
}

/// [str] for-yield with scalar transformation — str elements borrowed,
/// lengths (int) yielded. Verifies no leak on source str elements.
#[test]
fn test_for_yield_str_to_lengths() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = for w in words yield w.length();
    if lengths[0] == 53 && lengths[1] == 56 then 0 else 1
}
"#,
        "for_yield_str_to_lengths",
    );
}

/// `[[int]]` for-yield — nested list elements borrowed from outer list,
/// inner sums yielded as scalars. Verifies `elem_dec_fn` on nested `[int]`.
#[test]
fn test_for_yield_nested_list() {
    assert_aot_success(
        r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let lists = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let sums = for l in lists yield sum_list(l);
    if sums[0] == 6 && sums[1] == 15 && sums[2] == 24 then 0 else 1
}
"#,
        "for_yield_nested_list",
    );
}

/// `[Option<str>]` for-yield with match — `Option<str>` elements borrowed,
/// pattern-matched to extract lengths. Verifies `elem_dec_fn` on `Option<str>`.
#[test]
fn test_for_yield_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [Option<str>] = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string exceeding threshold here")
    ];
    let lengths = for item in items yield match item {
        Some(s) -> s.length(),
        None -> 0,
    };
    if lengths[0] == 53 && lengths[1] == 0 && lengths[2] == 49 then 0 else 1
}
"#,
        "for_yield_option_str",
    );
}

// -----------------------------------------------------------------------
// For-yield mutable variable threading (TPR-02-002 regression)
// -----------------------------------------------------------------------

/// Outer mutable variable mutation inside for-yield body — the body
/// assigns to `sum` which is declared outside the for-yield. Verifies
/// that the assignment is correctly propagated through the loop's SSA
/// block parameters. Regression test for TPR-02-002 where
/// `clear_mutable_names()` silently dropped the assignment in AOT.
#[test]
fn test_for_yield_outer_mutable_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    let result = for x in [10, 20, 30] yield {
        sum = sum + x;
        x
    };
    // sum should be 60, result should be [10, 20, 30]
    if sum == 60 && result[0] == 10 && result[2] == 30 then 0 else 1
}
"#,
        "for_yield_outer_mutable_mutation",
    );
}

/// Nested for-do inside for-yield body — inner loop mutates an outer
/// variable. Verifies mutable threading works with nested control flow.
#[test]
fn test_for_yield_nested_for_do_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    let result = for x in [1, 2, 3] yield {
        for y in [10, 20] do {
            total = total + y
        };
        total
    };
    // Each iteration adds 30 to total: [30, 60, 90]
    // total ends at 90
    if total == 90 && result[0] == 30 && result[2] == 90 then 0 else 1
}
"#,
        "for_yield_nested_for_do_mutation",
    );
}

/// For-yield with str elements and outer mutable counter — combines
/// fat pointer iteration with mutable variable threading and leak check.
#[test]
fn test_for_yield_str_with_mutable_counter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let lengths = for s in ["this is a very long string that exceeds SSO threshold", "another very long exceeding SSO"] yield {
        count = count + 1;
        s.length()
    };
    if count == 2 && lengths[0] == 53 && lengths[1] == 31 then 0 else 1
}
"#,
        "for_yield_str_with_mutable_counter",
    );
}

// -----------------------------------------------------------------------
// For-yield RC balance tests (Section 03.4)
// -----------------------------------------------------------------------

/// For-yield with closure elements — closures applied to argument in body.
#[test]
fn test_for_yield_closure_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [
        x -> x + 1,
        x -> x * 2,
        x -> x - 3
    ];
    let results = for f in fns yield f(10);
    if results[0] == 11 && results[1] == 20 && results[2] == 7 then 0 else 1
}
"#,
        "for_yield_closure_elements",
    );
}

/// For-yield with struct elements containing str fields — struct field
/// access in body, verifies element RC through struct aggregates.
#[test]
fn test_for_yield_struct_elements() {
    assert_aot_success(
        r#"
type Item = { name: str }

@main () -> int = {
    let items = [
        Item { name: "this is a very long name that exceeds SSO threshold" },
        Item { name: "another long name exceeding SSO for sure here" }
    ];
    let lengths = for item in items yield item.name.length();
    if lengths[0] == 51 && lengths[1] == 45 then 0 else 1
}
"#,
        "for_yield_struct_elements",
    );
}

/// For-yield with guard on `[str]` — filters short strings, yields only
/// long ones. Verifies `guard_skip` path threads mutable params correctly.
#[test]
fn test_for_yield_guard_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "short",
        "this is a very long string that exceeds SSO threshold",
        "hi",
        "another very long string exceeding SSO for sure here"
    ];
    let long_lengths = for w in words if w.length() > 10 yield w.length();
    if long_lengths[0] == 53 && long_lengths[1] == 52 then 0 else 1
}
"#,
        "for_yield_guard_str",
    );
}

/// For-yield on `[[str]]` — nested list of str lists. Verifies element
/// cleanup for nested fat pointer types.
#[test]
fn test_for_yield_nested_str_loops() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [
        ["this is a very long string exceeding SSO threshold here", "short"],
        ["another very long string that exceeds SSO"]
    ];
    let counts = for sublist in lists yield sublist.length();
    if counts[0] == 2 && counts[1] == 1 then 0 else 1
}
"#,
        "for_yield_nested_str_loops",
    );
}

/// For-yield on empty `[str]` — zero iterations, empty result list.
/// Verifies no leak from allocated-but-empty growable list.
#[test]
fn test_for_yield_empty_str_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words: [str] = [];
    let result = for w in words yield w.length();
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_empty_str_list",
    );
}

// -----------------------------------------------------------------------
// Map iteration with RC-managed keys/values — key_dec_fn/val_dec_fn
// -----------------------------------------------------------------------

/// `{str: int}` map for-do full consumption — heap str keys properly
/// cleaned up via `key_dec_fn` when buffer is freed.
#[test]
fn test_map_str_key_for_do_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": 10,
        "another very long key that exceeds SSO too": 20,
        "third long key exceeding SSO threshold": 30
    };
    let total = 0;
    for (k, v) in m do {
        total = total + v;
    };
    if total == 60 then 0 else 1
}
"#,
        "map_str_key_for_do_full",
    );
}

/// `{str: int}` map for-do with early break — unconsumed entries' str keys
/// must be cleaned up by `key_dec_fn` in the iterator's Drop path.
#[test]
fn test_map_str_key_for_do_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": 10,
        "another very long key that exceeds SSO too": 20,
        "third long key exceeding SSO threshold": 30
    };
    let total = 0;
    for (k, v) in m do {
        total = total + v;
        if total >= 10 then break;
    };
    if total >= 10 then 0 else 1
}
"#,
        "map_str_key_for_do_break",
    );
}

/// `{int: str}` map for-do — heap str values cleaned up via `val_dec_fn`.
#[test]
fn test_map_str_val_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        10: "this is a very long value exceeding SSO",
        20: "another very long value that exceeds SSO too",
        30: "third long value exceeding SSO threshold"
    };
    let total = 0;
    for (k, v) in m do {
        total = total + k;
    };
    if total == 60 then 0 else 1
}
"#,
        "map_str_val_for_do",
    );
}

/// `{str: str}` map for-do — both keys and values are heap strings,
/// both `key_dec_fn` and `val_dec_fn` must work.
#[test]
fn test_map_str_key_str_val_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": "long value exceeding SSO threshold too",
        "another very long key that exceeds SSO": "another long value exceeding threshold"
    };
    let count = 0;
    for (k, v) in m do {
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "map_str_key_str_val_for_do",
    );
}

// For-yield break/continue lowering

/// `break` in for-yield: stop early, return accumulated list.
#[test]
fn test_for_yield_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 4 then break;
        x * 2
    };
    // Expected: [2, 4, 6] (x=1→2, x=2→4, x=3→6, x=4→break)
    if result.length() == 3 then 0 else 1
}
"#,
        "for_yield_break",
    );
}

/// `break value` in for-yield: push value then stop.
#[test]
fn test_for_yield_break_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 4 then break 99;
        x
    };
    // Expected: [1, 2, 3, 99] (x=4→break 99 pushes 99)
    let ok = result.length() == 4;
    if ok then {
        // Verify last element is 99
        ok = result[3] == 99
    };
    if ok then 0 else 1
}
"#,
        "for_yield_break_value",
    );
}

/// `continue` in for-yield: skip element, don't push.
#[test]
fn test_for_yield_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 3 then continue;
        x
    };
    // Expected: [1, 2, 4, 5] (x=3 skipped)
    if result.length() == 4 then 0 else 1
}
"#,
        "for_yield_continue",
    );
}

/// `continue value` in for-yield: push substituted value.
#[test]
fn test_for_yield_continue_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 3 then continue 0;
        x
    };
    // Expected: [1, 2, 0, 4, 5] (x=3→continue 0 pushes 0 instead)
    let ok = result.length() == 5;
    if ok then {
        ok = result[2] == 0
    };
    if ok then 0 else 1
}
"#,
        "for_yield_continue_value",
    );
}

/// `break` in for-yield over str list: RC correctness with fat pointers.
#[test]
fn test_for_yield_break_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold",
        "third string for good measure and beyond SSO"
    ];
    let result = for w in words yield {
        if w.length() > 55 then break;
        w
    };
    // Expected: ["this is a very long string..."] — first string (53 chars) passes,
    // second (56 chars) triggers break
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_break_str",
    );
}

/// `continue` in for-yield over str list: skip without leaking.
#[test]
fn test_for_yield_continue_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "short",
        "another very long string that also exceeds the threshold"
    ];
    let result = for w in words yield {
        if w.length() < 10 then continue;
        w
    };
    // Expected: 2 elements (both long strings, "short" skipped)
    if result.length() == 2 then 0 else 1
}
"#,
        "for_yield_continue_str",
    );
}

/// `break` + mutable var: mutable variable threading preserved across break.
#[test]
fn test_for_yield_break_mutable() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let result = for x in [10, 20, 30, 40, 50] yield {
        count = count + 1;
        if x == 30 then break;
        x
    };
    // result = [10, 20], count = 3 (incremented before break check)
    let ok = result.length() == 2;
    if ok then {
        ok = count == 3
    };
    if ok then 0 else 1
}
"#,
        "for_yield_break_mutable",
    );
}

/// `continue value` + mutable var: mutable variable threading through continue.
#[test]
fn test_for_yield_continue_value_mutable() {
    assert_aot_success(
        r#"
@main () -> int = {
    let skipped = 0;
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x % 2 == 0 then {
            skipped = skipped + 1;
            continue 0
        };
        x
    };
    // result = [1, 0, 3, 0, 5], skipped = 2
    let ok = result.length() == 5;
    if ok then {
        ok = skipped == 2
    };
    if ok then 0 else 1
}
"#,
        "for_yield_continue_value_mutable",
    );
}
