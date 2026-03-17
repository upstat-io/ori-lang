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
// [[int]] nested iteration — inner lists are RC-managed
// -----------------------------------------------------------------------

#[test]
#[ignore = "intermittent double-free — requires RC header extension — fat-pointer-hardening 01.2"]
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
