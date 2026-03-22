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
