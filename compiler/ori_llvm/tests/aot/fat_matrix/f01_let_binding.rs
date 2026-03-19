//! F01: Let Binding — fat pointer types bound via `let` and used in expressions.
//!
//! Tests that each fat pointer type category can be constructed, bound to a
//! variable, and used without leaks or double-frees.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4: String (SSO — ≤23 bytes)
#[test]
fn test_fm_let_str_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    if s.length() == 5 then 0 else 1
}
"#,
        "fm_let_str_sso",
    );
}

// T5: String (heap — >23 bytes)
#[test]
fn test_fm_let_str_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    if s.length() == 30 then 0 else 1
}
"#,
        "fm_let_str_heap",
    );
}

// T6: List of scalars
#[test]
fn test_fm_let_list_scalar() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    if xs.length() == 5 then 0 else 1
}
"#,
        "fm_let_list_scalar",
    );
}

// T7: List of fat pointers
#[test]
fn test_fm_let_list_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["hello", "world", "test"];
    if xs.length() == 3 then 0 else 1
}
"#,
        "fm_let_list_fat",
    );
}

// T8: Struct (scalar fields only)
#[test]
fn test_fm_let_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    if p.x + p.y == 30 then 0 else 1
}
"#,
        "fm_let_struct_scalar",
    );
}

// T9: Struct (fat fields)
#[test]
fn test_fm_let_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let n = Named { name: "alice", id: 1 };
    if n.name.length() == 5 then 0 else 1
}
"#,
        "fm_let_struct_fat",
    );
}

// T15: Option<int>
#[test]
fn test_fm_let_option_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some(42);
    let y: Option<int> = None;
    if is_some(option: x) then {
        if is_none(option: y) then 0 else 1
    } else 2
}
"#,
        "fm_let_option_int",
    );
}

// T16: Option<str>
#[test]
fn test_fm_let_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some("hello");
    let y: Option<str> = None;
    if is_some(option: x) then {
        if is_none(option: y) then 0 else 1
    } else 2
}
"#,
        "fm_let_option_str",
    );
}

// T17: Map (str keys)
#[test]
fn test_fm_let_map_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    if m.length() == 3 then 0 else 1
}
"#,
        "fm_let_map_str",
    );
}

// T18: Tuple (mixed — str + int)
#[test]
fn test_fm_let_tuple_mixed() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = ("hello", 42);
    let (s, n) = t;
    if s.length() + n == 47 then 0 else 1
}
"#,
        "fm_let_tuple_mixed",
    );
}

// T12: Closure (no capture)
#[test]
fn test_fm_let_closure_no_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (x: int) -> int = x + 1;
    if f(0) == 1 then 0 else 1
}
"#,
        "fm_let_closure_no_capture",
    );
}

// T13: Closure (scalar capture)
#[test]
fn test_fm_let_closure_scalar_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = 10;
    let f = (x: int) -> int = x + n;
    if f(5) == 15 then 0 else 1
}
"#,
        "fm_let_closure_scalar_capture",
    );
}

// T14: Closure (fat capture — str)
#[test]
fn test_fm_let_closure_fat_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "abcdefghijklmnopqrstuvwxyz1234";
    let f = (x: int) -> int = prefix.length() + x;
    if f(5) == 35 then 0 else 1
}
"#,
        "fm_let_closure_fat_capture",
    );
}

// Multiple bindings — several fat values in scope at once
#[test]
fn test_fm_let_multiple_fat_values() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s1 = "abcdefghijklmnopqrstuvwxyz1234";
    let s2 = "another_long_string_for_heap_alloc";
    let xs = [1, 2, 3];
    let result = s1.length() + s2.length() + xs.length();
    if result == 67 then 0 else 1
}
"#,
        "fm_let_multiple_fat_values",
    );
}

// Rebinding — fat value rebound should not leak
#[test]
fn test_fm_let_rebind_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let s = "short";
    if s.length() == 5 then 0 else 1
}
"#,
        "fm_let_rebind_fat",
    );
}
