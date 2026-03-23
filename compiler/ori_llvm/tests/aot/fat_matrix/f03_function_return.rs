//! F03: Function Return — fat pointer types returned from functions.
//!
//! Tests return ABI (sret vs register), RC ownership transfer, and
//! correct cleanup when returned values go out of scope.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4: String (SSO) returned from function
#[test]
fn test_fm_return_str_sso() {
    assert_aot_success(
        r#"
@make_greeting () -> str = "hello";

@main () -> int = {
    let s = make_greeting();
    if s.length() == 5 then 0 else 1
}
"#,
        "fm_return_str_sso",
    );
}

// T5: String (heap) returned from function
#[test]
fn test_fm_return_str_heap() {
    assert_aot_success(
        r#"
@make_long_str () -> str = "abcdefghijklmnopqrstuvwxyz1234";

@main () -> int = {
    let s = make_long_str();
    if s.length() == 30 then 0 else 1
}
"#,
        "fm_return_str_heap",
    );
}

// T6: List of scalars returned from function
#[test]
fn test_fm_return_list_scalar() {
    assert_aot_success(
        r#"
@make_list () -> [int] = [10, 20, 30];

@main () -> int = {
    let xs = make_list();
    if xs.length() == 3 then 0 else 1
}
"#,
        "fm_return_list_scalar",
    );
}

// T7: List of fat pointers returned from function
#[test]
fn test_fm_return_list_fat() {
    assert_aot_success(
        r#"
@make_words () -> [str] = ["hello", "world"];

@main () -> int = {
    let ws = make_words();
    if ws.length() == 2 then 0 else 1
}
"#,
        "fm_return_list_fat",
    );
}

// T8: Struct (scalar fields) returned from function
#[test]
fn test_fm_return_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@origin () -> Point = Point { x: 0, y: 0 };

@main () -> int = {
    let p = origin();
    if p.x + p.y == 0 then 0 else 1
}
"#,
        "fm_return_struct_scalar",
    );
}

// T9: Struct (fat fields) returned from function
#[test]
fn test_fm_return_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@make_named () -> Named = Named { name: "alice", id: 42 };

@main () -> int = {
    let n = make_named();
    if n.name.length() + n.id == 47 then 0 else 1
}
"#,
        "fm_return_struct_fat",
    );
}

// T15: Option<int> returned from function
#[test]
fn test_fm_return_option_int() {
    assert_aot_success(
        r#"
@find_positive (n: int) -> Option<int> =
    if n > 0 then Some(n) else None;

@main () -> int = {
    let a = find_positive(n: 42);
    let b = find_positive(n: -1);
    if is_some(option: a) then {
        if is_none(option: b) then 0 else 1
    } else 2
}
"#,
        "fm_return_option_int",
    );
}

// T16: Option<str> returned from function
#[test]
fn test_fm_return_option_str() {
    assert_aot_success(
        r#"
@maybe_greet (do_greet: bool) -> Option<str> =
    if do_greet then Some("hello") else None;

@main () -> int = {
    let a = maybe_greet(do_greet: true);
    let b = maybe_greet(do_greet: false);
    if is_some(option: a) then {
        if is_none(option: b) then 0 else 1
    } else 2
}
"#,
        "fm_return_option_str",
    );
}

// T17: Map returned from function
#[test]
fn test_fm_return_map() {
    assert_aot_success(
        r#"
@make_map () -> {str: int} = {"x": 1, "y": 2};

@main () -> int = {
    let m = make_map();
    if m.length() == 2 then 0 else 1
}
"#,
        "fm_return_map",
    );
}

// T18: Tuple (mixed) returned from function
#[test]
fn test_fm_return_tuple_mixed() {
    assert_aot_success(
        r#"
@make_pair () -> (str, int) = ("hello", 42);

@main () -> int = {
    let (s, n) = make_pair();
    if s.length() + n == 47 then 0 else 1
}
"#,
        "fm_return_tuple_mixed",
    );
}

// Chained returns — returned fat value passed to another function
#[test]
fn test_fm_return_chained() {
    assert_aot_success(
        r#"
@make_str () -> str = "abcdefghijklmnopqrstuvwxyz1234";

@get_len (s: str) -> int = s.length();

@main () -> int = {
    let len = get_len(s: make_str());
    if len == 30 then 0 else 1
}
"#,
        "fm_return_chained",
    );
}
