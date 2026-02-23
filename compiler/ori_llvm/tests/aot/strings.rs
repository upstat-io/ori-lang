//! String Method AOT Tests
//!
//! Tests for string builtin methods and operations through the AOT pipeline.
//! Covers methods known to be in the builtin table (length, `is_empty`, concat,
//! iter, clone, `to_str`) and methods that may or may not be available yet
//! (contains, `starts_with`, split, trim, etc.) to build a gap inventory.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── length / len ───

#[test]
fn test_str_length_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    if s.length() == 5 then 0 else 1
}
"#,
        "str_length_basic",
    );
}

#[test]
fn test_str_length_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    if s.length() == 0 then 0 else 1
}
"#,
        "str_length_empty",
    );
}

#[test]
fn test_str_length_single_char() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "x";
    if s.length() == 1 then 0 else 1
}
"#,
        "str_length_single",
    );
}

#[test]
fn test_str_length_with_spaces() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    if s.length() == 11 then 0 else 1
}
"#,
        "str_length_spaces",
    );
}

#[test]
fn test_str_length_with_escapes() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "a\tb\nc";
    // 'a', '\t', 'b', '\n', 'c' = 5 bytes
    if s.length() == 5 then 0 else 1
}
"#,
        "str_length_escapes",
    );
}

#[test]
fn test_str_len_alias() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "test";
    // len() is an alias for length()
    if s.len() == 4 then 0 else 1
}
"#,
        "str_len_alias",
    );
}

// ─── is_empty ───

#[test]
fn test_str_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    if s.is_empty() then 0 else 1
}
"#,
        "str_is_empty_true",
    );
}

#[test]
fn test_str_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "x";
    if !s.is_empty() then 0 else 1
}
"#,
        "str_is_empty_false",
    );
}

#[test]
fn test_str_is_empty_space() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = " ";
    // A string with a space is NOT empty
    if !s.is_empty() then 0 else 1
}
"#,
        "str_is_empty_space",
    );
}

// ─── concat ───

#[test]
#[ignore = "AOT gap: str.concat() return type not tracked — comparison uses icmp instead of string eq"]
fn test_str_concat_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = " world";
    let c = a.concat(b);
    if c == "hello world" then 0 else 1
}
"#,
        "str_concat_basic",
    );
}

#[test]
#[ignore = "AOT gap: str.concat() return type not tracked — comparison uses icmp instead of string eq"]
fn test_str_concat_empty_left() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "";
    let b = "hello";
    let c = a.concat(b);
    if c == "hello" then 0 else 1
}
"#,
        "str_concat_empty_left",
    );
}

#[test]
#[ignore = "AOT gap: str.concat() return type not tracked — comparison uses icmp instead of string eq"]
fn test_str_concat_empty_right() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "";
    let c = a.concat(b);
    if c == "hello" then 0 else 1
}
"#,
        "str_concat_empty_right",
    );
}

#[test]
fn test_str_concat_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = "a" + "b" + "c" + "d";
    if result == "abcd" then 0 else 1
}
"#,
        "str_concat_chain",
    );
}

// ─── to_str (identity for strings) ───

#[test]
#[ignore = "AOT gap: str.to_str() return type not tracked as str — comparison falls through to icmp"]
fn test_str_to_str_identity() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    let s2 = s.to_str();
    if s2 == "hello" then 0 else 1
}
"#,
        "str_to_str_identity",
    );
}

// ─── clone ───

#[test]
fn test_str_clone() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    let s2 = s.clone();
    if s == s2 then 0 else 1
}
"#,
        "str_clone",
    );
}

#[test]
fn test_str_clone_independence() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s1 = "hello";
    let s2 = s1.clone();
    let s3 = s1 + " world";
    // s2 should still be "hello" even though s3 was built from s1
    if s2 == "hello" && s3 == "hello world" then 0 else 1
}
"#,
        "str_clone_indep",
    );
}

// ─── iter ───

#[test]
fn test_str_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    let count = s.iter().count();
    if count == 5 then 0 else 1
}
"#,
        "str_iter_count",
    );
}

#[test]
fn test_str_iter_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    let count = s.iter().count();
    if count == 0 then 0 else 1
}
"#,
        "str_iter_empty",
    );
}

#[test]
fn test_str_iter_for_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for c in "abc" do {
        count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "str_iter_for",
    );
}

// ─── String comparison ───

#[test]
fn test_str_compare_equal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    if a == b then 0 else 1
}
"#,
        "str_cmp_equal",
    );
}

#[test]
fn test_str_compare_not_equal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "world";
    if a != b then 0 else 1
}
"#,
        "str_cmp_not_equal",
    );
}

#[test]
#[ignore = "AOT gap: string ordering comparison (<, >) — icmp used instead of string compare runtime call"]
fn test_str_compare_less() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "apple";
    let b = "banana";
    if a < b then 0 else 1
}
"#,
        "str_cmp_less",
    );
}

#[test]
#[ignore = "AOT gap: string ordering comparison (<, >) — icmp used instead of string compare runtime call"]
fn test_str_compare_greater() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "zebra";
    let b = "apple";
    if a > b then 0 else 1
}
"#,
        "str_cmp_greater",
    );
}

#[test]
#[ignore = "AOT gap: string ordering comparison (<, >) — icmp used instead of string compare runtime call"]
fn test_str_compare_prefix() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello world";
    // "hello" < "hello world" (prefix is less)
    if a < b then 0 else 1
}
"#,
        "str_cmp_prefix",
    );
}

// ─── String + type conversion concat ───

#[test]
fn test_str_concat_with_int_to_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let name = "item";
    let num = 42;
    let label = name + " #" + num.to_str();
    if label == "item #42" then 0 else 1
}
"#,
        "str_concat_int_to_str",
    );
}

#[test]
fn test_str_concat_with_bool_to_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "active: ";
    let flag = true;
    let msg = prefix + flag.to_str();
    if msg == "active: true" then 0 else 1
}
"#,
        "str_concat_bool_to_str",
    );
}

// ─── String in data structures ───

#[test]
fn test_str_in_tuple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let pair = ("hello", 42);
    let (s, n) = pair;
    if s == "hello" && n == 42 then 0 else 1
}
"#,
        "str_in_tuple",
    );
}

#[test]
fn test_str_in_struct() {
    assert_aot_success(
        r#"
type Person = { name: str, age: int }

@main () -> int = {
    let p = Person { name: "Alice", age: 30 };
    if p.name == "Alice" && p.age == 30 then 0 else 1
}
"#,
        "str_in_struct",
    );
}

#[test]
fn test_str_struct_field_length() {
    assert_aot_success(
        r#"
type Message = { text: str }

@main () -> int = {
    let m = Message { text: "hello world" };
    if m.text.length() == 11 then 0 else 1
}
"#,
        "str_struct_field_len",
    );
}

// ─── String methods not in builtin table (expected gaps) ───

#[test]
#[ignore = "AOT gap: str.contains not in builtin table"]
fn test_str_contains() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    if s.contains("world") then 0 else 1
}
"#,
        "str_contains",
    );
}

#[test]
#[ignore = "AOT gap: str.starts_with not in builtin table"]
fn test_str_starts_with() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    if s.starts_with("hello") then 0 else 1
}
"#,
        "str_starts_with",
    );
}

#[test]
#[ignore = "AOT gap: str.ends_with not in builtin table"]
fn test_str_ends_with() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    if s.ends_with("world") then 0 else 1
}
"#,
        "str_ends_with",
    );
}

#[test]
#[ignore = "AOT gap: str.trim not in builtin table"]
fn test_str_trim() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "  hello  ";
    if s.trim() == "hello" then 0 else 1
}
"#,
        "str_trim",
    );
}

#[test]
#[ignore = "AOT gap: str.to_uppercase not in builtin table"]
fn test_str_to_uppercase() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    if s.to_uppercase() == "HELLO" then 0 else 1
}
"#,
        "str_to_uppercase",
    );
}

#[test]
#[ignore = "AOT gap: str.to_lowercase not in builtin table"]
fn test_str_to_lowercase() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "HELLO";
    if s.to_lowercase() == "hello" then 0 else 1
}
"#,
        "str_to_lowercase",
    );
}

#[test]
#[ignore = "AOT gap: str.replace not in builtin table"]
fn test_str_replace() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    let r = s.replace("world", "earth");
    if r == "hello earth" then 0 else 1
}
"#,
        "str_replace",
    );
}

#[test]
#[ignore = "AOT gap: str.split not in builtin table"]
fn test_str_split() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "a,b,c";
    let parts = s.split(",");
    if parts.length() == 3 then 0 else 1
}
"#,
        "str_split",
    );
}

#[test]
#[ignore = "AOT gap: str.repeat not in builtin table"]
fn test_str_repeat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "ab";
    let r = s.repeat(3);
    if r == "ababab" then 0 else 1
}
"#,
        "str_repeat",
    );
}

#[test]
#[ignore = "AOT gap: str.chars not in builtin table"]
fn test_str_chars() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "abc";
    let chars = s.chars();
    if chars.count() == 3 then 0 else 1
}
"#,
        "str_chars",
    );
}
