//! F04: Closure Capture — fat pointer types captured in closure environments.
//!
//! This is the J17 bug area: closures capturing non-scalar values had missing
//! AIMS param ownership annotations, causing spurious `RcDec` on borrowed aliases.
//! Tests that each fat pointer type can be captured and used correctly.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4: Closure captures SSO string
#[test]
fn test_fm_capture_str_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    let greeting = "hello";
    let f = (n: int) -> int = greeting.length() + n;
    if f(5) == 10 then 0 else 1
}
"#,
        "fm_capture_str_sso",
    );
}

// T5: Closure captures heap string
#[test]
fn test_fm_capture_str_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "abcdefghijklmnopqrstuvwxyz1234";
    let f = (n: int) -> int = prefix.length() + n;
    if f(5) == 35 then 0 else 1
}
"#,
        "fm_capture_str_heap",
    );
}

// T5: Closure captures heap string, called twice (RC must be correct)
#[test]
fn test_fm_capture_str_heap_two_calls() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "abcdefghijklmnopqrstuvwxyz1234";
    let f = (n: int) -> int = prefix.length() + n;
    let a = f(5);
    let b = f(10);
    if a + b == 75 then 0 else 1
}
"#,
        "fm_capture_str_heap_two_calls",
    );
}

// T6: Closure captures list of scalars
#[test]
fn test_fm_capture_list_scalar() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let f = (n: int) -> int = xs.length() + n;
    if f(7) == 10 then 0 else 1
}
"#,
        "fm_capture_list_scalar",
    );
}

// T7: Closure captures list of strings
#[test]
fn test_fm_capture_list_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = ["hello", "world", "test"];
    let f = (n: int) -> int = words.length() + n;
    if f(2) == 5 then 0 else 1
}
"#,
        "fm_capture_list_fat",
    );
}

// T8: Closure captures struct with scalar fields
#[test]
fn test_fm_capture_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    let f = (n: int) -> int = p.x + p.y + n;
    if f(5) == 35 then 0 else 1
}
"#,
        "fm_capture_struct_scalar",
    );
}

// T9: Closure captures struct with fat fields
#[test]
fn test_fm_capture_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let n = Named { name: "alice", id: 1 };
    let f = (x: int) -> int = n.name.length() + n.id + x;
    if f(4) == 10 then 0 else 1
}
"#,
        "fm_capture_struct_fat",
    );
}

// T15: Closure captures Option<int>
#[test]
fn test_fm_capture_option_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some(42);
    let f = (n: int) -> int = if is_some(option: opt) then n + 1 else n;
    if f(9) == 10 then 0 else 1
}
"#,
        "fm_capture_option_int",
    );
}

// T17: Closure captures map
#[test]
fn test_fm_capture_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2};
    let f = (n: int) -> int = m.length() + n;
    if f(8) == 10 then 0 else 1
}
"#,
        "fm_capture_map",
    );
}

// Multiple captures: str + [int]
#[test]
fn test_fm_capture_multi_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let name = "abcdefghijklmnopqrstuvwxyz1234";
    let nums = [1, 2, 3];
    let f = (x: int) -> int = name.length() + nums.length() + x;
    if f(7) == 40 then 0 else 1
}
"#,
        "fm_capture_multi_fat",
    );
}

// Closure passed as argument with fat capture
#[test]
fn test_fm_capture_passed_as_arg() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let prefix = "abcdefghijklmnopqrstuvwxyz1234";
    let f = (n: int) -> int = prefix.length() + n;
    if apply(f: f, x: 5) == 35 then 0 else 1
}
"#,
        "fm_capture_passed_as_arg",
    );
}

// Closure used in for loop body with fat capture
#[test]
fn test_fm_capture_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "hello";
    let f = (n: int) -> int = prefix.length() + n;
    let total = 0;
    for i in [1, 2, 3] do {
        total = total + f(i);
    };
    if total == 21 then 0 else 1
}
"#,
        "fm_capture_in_loop",
    );
}
