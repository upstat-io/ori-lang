//! Borrowed parameter iteration tests — F5: function parameter iteration.
//!
//! Dimension matrix: call count (1, 2, N) × iteration mode (full, break, yield)
//! × element type ([str], [int], struct) × caller context (own, COW, chained).

use crate::util::assert_aot_success;

// Single call × full iteration

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

// Two sequential calls (the original bug scenario)

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

// N calls in a loop

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

// Partial iteration (break) with borrowed param

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

// Yield with borrowed param

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

// COW after borrowed call

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

// Chained callees: A calls B which iterates

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

// Borrowed param: iterate + use list after loop

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

// Borrowed param: two different lists passed to same function

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

// Borrowed param: map iteration with string keys

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

// Combined scenarios: borrowed param + other features

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
