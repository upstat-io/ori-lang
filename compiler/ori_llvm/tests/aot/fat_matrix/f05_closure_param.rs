//! F05: Closure Parameter — fat pointer types passed through closure calls.
//!
//! Tests indirect call ABI, trampoline generation, and RC handling when fat
//! pointer values flow through closure parameters (as opposed to captures).

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4: SSO string passed as closure parameter
#[test]
fn test_fm_closure_param_str_sso() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (s: str) -> int = s.length();
    if f("hello") == 5 then 0 else 1
}
"#,
        "fm_closure_param_str_sso",
    );
}

// T5: Heap string passed as closure parameter
#[test]
fn test_fm_closure_param_str_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (s: str) -> int = s.length();
    if f("abcdefghijklmnopqrstuvwxyz1234") == 30 then 0 else 1
}
"#,
        "fm_closure_param_str_heap",
    );
}

// T6: List of scalars passed as closure parameter
#[test]
fn test_fm_closure_param_list_scalar() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (xs: [int]) -> int = xs.length();
    if f([10, 20, 30]) == 3 then 0 else 1
}
"#,
        "fm_closure_param_list_scalar",
    );
}

// T7: List of fat pointers passed as closure parameter
#[test]
fn test_fm_closure_param_list_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (xs: [str]) -> int = xs.length();
    if f(["hello", "world"]) == 2 then 0 else 1
}
"#,
        "fm_closure_param_list_fat",
    );
}

// T8: Struct with scalar fields passed as closure parameter
#[test]
fn test_fm_closure_param_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let f = (p: Point) -> int = p.x + p.y;
    let p = Point { x: 10, y: 20 };
    if f(p) == 30 then 0 else 1
}
"#,
        "fm_closure_param_struct_scalar",
    );
}

// T9: Struct with fat fields passed as closure parameter
#[test]
fn test_fm_closure_param_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let f = (n: Named) -> int = n.name.length() + n.id;
    let n = Named { name: "alice", id: 5 };
    if f(n) == 10 then 0 else 1
}
"#,
        "fm_closure_param_struct_fat",
    );
}

// T15: Option<int> passed as closure parameter
#[test]
fn test_fm_closure_param_option_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (opt: Option<int>) -> int = {
        if is_some(option: opt) then 1 else 0
    };
    let a = f(Some(42));
    let b = f(None);
    if a == 1 then {
        if b == 0 then 0 else 1
    } else 2
}
"#,
        "fm_closure_param_option_int",
    );
}

// T17: Map passed as closure parameter
#[test]
fn test_fm_closure_param_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (m: {str: int}) -> int = m.length();
    if f({"a": 1, "b": 2}) == 2 then 0 else 1
}
"#,
        "fm_closure_param_map",
    );
}

// T18: Tuple (mixed) passed as closure parameter
#[test]
fn test_fm_closure_param_tuple_mixed() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (t: (str, int)) -> int = {
        let (s, n) = t;
        s.length() + n
    };
    if f(("hello", 5)) == 10 then 0 else 1
}
"#,
        "fm_closure_param_tuple_mixed",
    );
}

// Closure with fat capture AND fat parameter
#[test]
fn test_fm_closure_param_with_fat_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "hello";
    let f = (s: str) -> int = prefix.length() + s.length();
    if f("world") == 10 then 0 else 1
}
"#,
        "fm_closure_param_with_fat_capture",
    );
}

// Closure passed to higher-order function with fat parameter
#[test]
fn test_fm_closure_param_higher_order() {
    assert_aot_success(
        r#"
@apply_to_str (f: (str) -> int, s: str) -> int = f(s);

@main () -> int = {
    let f = (s: str) -> int = s.length();
    if apply_to_str(f: f, s: "hello") == 5 then 0 else 1
}
"#,
        "fm_closure_param_higher_order",
    );
}

// Multiple fat parameters to closure
#[test]
fn test_fm_closure_param_multi_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (a: str, b: str) -> int = a.length() + b.length();
    if f("hello", "world") == 10 then 0 else 1
}
"#,
        "fm_closure_param_multi_fat",
    );
}
