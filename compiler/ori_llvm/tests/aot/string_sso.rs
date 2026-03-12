//! SSO (Small String Optimization) AOT Tests
//!
//! Tests that specifically exercise the SSO boundary (23 bytes) and ensure
//! correct behavior for both inline (SSO) and heap-allocated strings through
//! the AOT pipeline. Covers:
//! - SSO strings (≤23 bytes inline, no heap)
//! - Heap strings (>23 bytes, RC-managed)
//! - Boundary transitions (concat crossing 23-byte threshold)
//! - RC operations (clone, drop, sharing) on both representations
//! - String methods dispatching correctly for both representations

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── SSO: basic inline strings ───

#[test]
fn test_sso_empty_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    if s.length() == 0 && s.is_empty() then 0 else 1
}
"#,
        "sso_empty",
    );
}

#[test]
fn test_sso_short_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    if s.length() == 5 && s == "hello" then 0 else 1
}
"#,
        "sso_short",
    );
}

#[test]
fn test_sso_max_length_23_bytes() {
    assert_aot_success(
        r#"
@main () -> int = {
    // Exactly 23 bytes — max SSO capacity
    let s = "12345678901234567890123";
    if s.length() == 23 then 0 else 1
}
"#,
        "sso_max_23",
    );
}

#[test]
fn test_sso_max_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "12345678901234567890123";
    let b = "12345678901234567890123";
    if a == b then 0 else 1
}
"#,
        "sso_max_eq",
    );
}

// ─── Heap: strings exceeding SSO ───

#[test]
fn test_heap_24_bytes() {
    assert_aot_success(
        r#"
@main () -> int = {
    // 24 bytes — first byte past SSO threshold
    let s = "123456789012345678901234";
    if s.length() == 24 then 0 else 1
}
"#,
        "heap_24",
    );
}

#[test]
fn test_heap_long_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "this is a string that is definitely longer than twenty-three bytes";
    if s.length() == 66 then 0 else 1
}
"#,
        "heap_long",
    );
}

#[test]
fn test_heap_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "123456789012345678901234";
    let b = "123456789012345678901234";
    if a == b then 0 else 1
}
"#,
        "heap_eq",
    );
}

#[test]
fn test_heap_inequality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "123456789012345678901234";
    let b = "123456789012345678901235";
    if a != b then 0 else 1
}
"#,
        "heap_neq",
    );
}

// ─── SSO ↔ heap transitions via concatenation ───

#[test]
fn test_sso_concat_stays_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    // 5 + 5 = 10 bytes — stays SSO
    let a = "hello";
    let b = " worl";
    let c = a + b;
    if c.length() == 10 && c == "hello worl" then 0 else 1
}
"#,
        "sso_concat_stays",
    );
}

#[test]
fn test_sso_concat_to_max_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    // 11 + 12 = 23 bytes — exactly at SSO limit
    let a = "12345678901";
    let b = "234567890123";
    let c = a + b;
    if c.length() == 23 then 0 else 1
}
"#,
        "sso_concat_max",
    );
}

#[test]
fn test_sso_concat_crosses_to_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    // 12 + 12 = 24 bytes — crosses SSO boundary → heap
    let a = "123456789012";
    let b = "345678901234";
    let c = a + b;
    if c.length() == 24 then 0 else 1
}
"#,
        "sso_concat_to_heap",
    );
}

#[test]
fn test_sso_chain_concat_to_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    // Start SSO, grow through concatenation to heap
    let s = "abc";
    let s = s + "defgh";
    let s = s + "ijklmnop";
    let s = s + "qrstuvwxyz";
    // 3 + 5 + 8 + 10 = 26 bytes > 23 → heap
    if s.length() == 26 then 0 else 1
}
"#,
        "sso_chain_to_heap",
    );
}

#[test]
fn test_heap_concat_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "this is already on the heap!";
    let b = " and so is this one too!!";
    let c = a + b;
    if c == "this is already on the heap! and so is this one too!!" then 0 else 1
}
"#,
        "heap_concat_heap",
    );
}

// ─── RC operations: clone/drop on both representations ───

#[test]
fn test_sso_clone() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = a.clone();
    if a == b && a == "hello" then 0 else 1
}
"#,
        "sso_clone",
    );
}

#[test]
fn test_heap_clone() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "this is a heap-allocated string!";
    let b = a.clone();
    if a == b && a == "this is a heap-allocated string!" then 0 else 1
}
"#,
        "heap_clone",
    );
}

#[test]
fn test_sso_clone_independence() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = a.clone();
    // Modify a via concat — b should be unaffected
    let a2 = a + " world";
    if b == "hello" && a2 == "hello world" then 0 else 1
}
"#,
        "sso_clone_indep",
    );
}

#[test]
fn test_heap_clone_independence() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "this is a heap-allocated string!";
    let b = a.clone();
    let a2 = a + " extended";
    if b == "this is a heap-allocated string!" && a2 == "this is a heap-allocated string! extended" then 0 else 1
}
"#,
        "heap_clone_indep",
    );
}

#[test]
fn test_sso_multiple_references() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "short";
    let b = a;
    let c = a;
    let d = a;
    if b == "short" && c == "short" && d == "short" then 0 else 1
}
"#,
        "sso_multi_ref",
    );
}

#[test]
fn test_heap_multiple_references() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "this is definitely a heap string";
    let b = a;
    let c = a;
    let d = a;
    if b == "this is definitely a heap string" && c == d then 0 else 1
}
"#,
        "heap_multi_ref",
    );
}

// ─── String methods on SSO vs heap ───

#[test]
fn test_sso_contains() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    if s.contains("world") && !s.contains("xyz") then 0 else 1
}
"#,
        "sso_contains",
    );
}

#[test]
fn test_heap_contains() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "this is a heap string with searchable content";
    if s.contains("searchable") && !s.contains("xyz") then 0 else 1
}
"#,
        "heap_contains",
    );
}

#[test]
fn test_sso_starts_ends_with() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world";
    if s.starts_with("hello") && s.ends_with("world") then 0 else 1
}
"#,
        "sso_starts_ends",
    );
}

#[test]
fn test_heap_starts_ends_with() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "this is a long heap string ending here";
    if s.starts_with("this") && s.ends_with("here") then 0 else 1
}
"#,
        "heap_starts_ends",
    );
}

#[test]
fn test_sso_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "apple";
    let b = "banana";
    if a < b && b > a then 0 else 1
}
"#,
        "sso_compare",
    );
}

#[test]
fn test_heap_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "apple is a fruit that grows on trees";
    let b = "banana is a fruit that grows in bunches";
    if a < b && b > a then 0 else 1
}
"#,
        "heap_compare",
    );
}

#[test]
fn test_sso_vs_heap_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    // SSO string vs heap string with same prefix
    let sso = "hello";
    let heap = "hello is a greeting word many use";
    // "hello" < "hello is..." (prefix is less)
    if sso < heap && sso != heap then 0 else 1
}
"#,
        "sso_vs_heap_cmp",
    );
}

// ─── Strings in data structures ───

#[test]
fn test_sso_in_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["a", "bb", "ccc"];
    if xs.length() == 3 then 0 else 1
}
"#,
        "sso_in_list",
    );
}

#[test]
fn test_heap_in_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [
        "this is a heap allocated string one",
        "this is a heap allocated string two",
        "this is a heap allocated string three"
    ];
    if xs.length() == 3 then 0 else 1
}
"#,
        "heap_in_list",
    );
}

#[test]
fn test_mixed_sso_heap_in_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [
        "short",
        "this is a heap allocated string!!!",
        "also short",
        "another heap string that is quite long indeed"
    ];
    if xs.length() == 4 then 0 else 1
}
"#,
        "mixed_sso_heap_list",
    );
}

#[test]
fn test_sso_in_struct() {
    assert_aot_success(
        r#"
type Pair = { name: str, value: str }

@main () -> int = {
    let p = Pair { name: "key", value: "val" };
    if p.name == "key" && p.value == "val" then 0 else 1
}
"#,
        "sso_in_struct",
    );
}

#[test]
fn test_heap_in_struct() {
    assert_aot_success(
        r#"
type Article = { title: str, body: str }

@main () -> int = {
    let a = Article {
        title: "A Title That Exceeds The SSO Limit",
        body: "And a body that is even longer than the title for sure"
    };
    if a.title == "A Title That Exceeds The SSO Limit" && a.body.length() > 23 then 0 else 1
}
"#,
        "heap_in_struct",
    );
}

#[test]
fn test_sso_in_tuple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = ("hello", "world");
    let (a, b) = t;
    if a == "hello" && b == "world" then 0 else 1
}
"#,
        "sso_in_tuple",
    );
}

// ─── String iteration on SSO vs heap ───

#[test]
fn test_sso_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for c in "hello" do {
        count = count + 1;
    };
    if count == 5 then 0 else 1
}
"#,
        "sso_iter",
    );
}

#[test]
fn test_heap_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for c in "this is a heap string now!" do {
        count = count + 1;
    };
    if count == 26 then 0 else 1
}
"#,
        "heap_iter",
    );
}

// ─── to_str conversions producing SSO vs heap ───

#[test]
fn test_int_to_str_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = 42.to_str();
    if s == "42" && s.length() == 2 then 0 else 1
}
"#,
        "int_to_str_sso",
    );
}

#[test]
fn test_bool_to_str_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = true.to_str();
    if s == "true" then 0 else 1
}
"#,
        "bool_to_str_sso",
    );
}

// ─── format strings with SSO and heap ───

#[test]
fn test_format_sso_result() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let s = `{x}`;
    if s == "42" then 0 else 1
}
"#,
        "fmt_sso_result",
    );
}

#[test]
fn test_format_heap_result() {
    assert_aot_success(
        r#"
@main () -> int = {
    let name = "world";
    let s = `hello {name}, this greeting is going to be quite long indeed!`;
    if s.length() > 23 then 0 else 1
}
"#,
        "fmt_heap_result",
    );
}

// ─── SSO boundary stress: repeated concat ───

#[test]
fn test_sso_repeated_concat_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    for i in 0..50 do {
        s = s + "x";
    };
    // After 50 iterations: "xxx...x" (50 chars) — well past SSO
    if s.length() == 50 then 0 else 1
}
"#,
        "sso_repeated_concat",
    );
}

#[test]
fn test_sso_boundary_exact_transition() {
    assert_aot_success(
        r#"
@main () -> int = {
    // Build string char by char across the SSO boundary
    let s = "";
    for i in 0..24 do {
        s = s + "a";
    };
    // After 24 iterations: "aaa...a" (24 chars) — just past SSO
    if s.length() == 24 then 0 else 1
}
"#,
        "sso_boundary_exact",
    );
}

// ─── Catch/error handling with SSO strings ───

#[test]
fn test_catch_returns_sso_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = catch(expr: "ok");
    match result {
        Ok(v) -> if v == "ok" then 0 else 1,
        Err(_) -> 2
    }
}
"#,
        "catch_sso_str",
    );
}

#[test]
fn test_catch_returns_heap_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = catch(expr: "this is a heap string for catch");
    match result {
        Ok(v) -> if v == "this is a heap string for catch" then 0 else 1,
        Err(_) -> 2
    }
}
"#,
        "catch_heap_str",
    );
}

// Immortal empty string: codegen calls ori_str_empty() (no heap, no RC).

#[test]
fn test_immortal_empty_string_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $a = "";
    let $b = "";
    let $c = "" + "";
    if a == "" && b == "" && c == "" && a.is_empty() then 0 else 1
}
"#,
        "immortal_empty_str_no_leak",
    );
}

#[test]
fn test_immortal_empty_string_passed_to_function() {
    assert_aot_success(
        r#"
@check (s: str) -> bool = s.is_empty();

@main () -> int = {
    let $empty = "";
    if check(s: empty) && check(s: "") then 0 else 1
}
"#,
        "immortal_empty_str_passed",
    );
}

#[test]
fn test_immortal_empty_string_in_collection() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = ["", "a", "", "b", ""];
    let count = 0;
    for item in items do {
        if item.is_empty() then {
            count += 1;
        }
    };
    if count == 3 then 0 else 1
}
"#,
        "immortal_empty_str_collection",
    );
}
