//! Generalize tests — verify `elem_dec_fn` works for all `[T]` where T has Drop.
//!
//! Covers: [str], [[int]], closures, structs with str fields, Option<str>,
//! string iteration, and multi-call patterns.

use crate::util::assert_aot_success;

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
