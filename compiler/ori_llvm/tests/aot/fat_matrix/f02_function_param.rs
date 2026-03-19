//! F02: Function Parameter — fat pointer types passed as arguments to functions.
//!
//! Tests ABI correctness (direct vs indirect passing), borrow elision, and
//! RC inc/dec at call boundaries.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4: String (SSO) as parameter
#[test]
fn test_fm_param_str_sso() {
    assert_aot_success(
        r#"
@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "hello";
    if get_len(s: s) == 5 then 0 else 1
}
"#,
        "fm_param_str_sso",
    );
}

// T5: String (heap) as parameter
#[test]
fn test_fm_param_str_heap() {
    assert_aot_success(
        r#"
@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    if get_len(s: s) == 30 then 0 else 1
}
"#,
        "fm_param_str_heap",
    );
}

// T5: String used after passing to function (RC inc required)
#[test]
fn test_fm_param_str_heap_reuse() {
    assert_aot_success(
        r#"
@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let len1 = get_len(s: s);
    let len2 = get_len(s: s);
    if len1 + len2 == 60 then 0 else 1
}
"#,
        "fm_param_str_heap_reuse",
    );
}

// T6: List of scalars as parameter
#[test]
fn test_fm_param_list_scalar() {
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
    let xs = [10, 20, 30];
    if sum_list(xs: xs) == 60 then 0 else 1
}
"#,
        "fm_param_list_scalar",
    );
}

// T7: List of fat pointers as parameter
#[test]
fn test_fm_param_list_fat() {
    assert_aot_success(
        r#"
@total_len (xs: [str]) -> int = {
    let total = 0;
    for s in xs do {
        total = total + s.length();
    };
    total
}

@main () -> int = {
    let words = ["hello", "world"];
    if total_len(xs: words) == 10 then 0 else 1
}
"#,
        "fm_param_list_fat",
    );
}

// T8: Struct (scalar fields) as parameter
#[test]
fn test_fm_param_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@sum_point (p: Point) -> int = p.x + p.y;

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    if sum_point(p: p) == 30 then 0 else 1
}
"#,
        "fm_param_struct_scalar",
    );
}

// T9: Struct (fat fields) as parameter
#[test]
fn test_fm_param_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@name_len (n: Named) -> int = n.name.length();

@main () -> int = {
    let n = Named { name: "alice", id: 1 };
    if name_len(n: n) == 5 then 0 else 1
}
"#,
        "fm_param_struct_fat",
    );
}

// T15: Option<int> as parameter
#[test]
fn test_fm_param_option_int() {
    assert_aot_success(
        r#"
@unwrap_or (opt: Option<int>, default: int) -> int =
    if is_some(option: opt) then 42 else default;

@main () -> int = {
    let x = Some(42);
    let y: Option<int> = None;
    let a = unwrap_or(opt: x, default: 0);
    let b = unwrap_or(opt: y, default: 99);
    if a == 42 then {
        if b == 99 then 0 else 1
    } else 2
}
"#,
        "fm_param_option_int",
    );
}

// T16: Option<str> as parameter
#[test]
fn test_fm_param_option_str() {
    assert_aot_success(
        r#"
@has_value (opt: Option<str>) -> int =
    if is_some(option: opt) then 1 else 0;

@main () -> int = {
    let x = Some("hello");
    let y: Option<str> = None;
    if has_value(opt: x) == 1 then {
        if has_value(opt: y) == 0 then 0 else 1
    } else 2
}
"#,
        "fm_param_option_str",
    );
}

// T17: Map as parameter
#[test]
fn test_fm_param_map_str() {
    assert_aot_success(
        r#"
@map_size (m: {str: int}) -> int = m.length();

@main () -> int = {
    let m = {"a": 1, "b": 2};
    if map_size(m: m) == 2 then 0 else 1
}
"#,
        "fm_param_map_str",
    );
}

// T18: Tuple (mixed) as parameter
#[test]
fn test_fm_param_tuple_mixed() {
    assert_aot_success(
        r#"
@tuple_sum (t: (str, int)) -> int = {
    let (s, n) = t;
    s.length() + n
}

@main () -> int = {
    let t = ("hello", 10);
    if tuple_sum(t: t) == 15 then 0 else 1
}
"#,
        "fm_param_tuple_mixed",
    );
}

// Multiple fat params — two fat values passed simultaneously
#[test]
fn test_fm_param_multiple_fat() {
    assert_aot_success(
        r#"
@combined_len (a: str, b: str) -> int = a.length() + b.length();

@main () -> int = {
    let s1 = "abcdefghijklmnopqrstuvwxyz1234";
    let s2 = "hello";
    if combined_len(a: s1, b: s2) == 35 then 0 else 1
}
"#,
        "fm_param_multiple_fat",
    );
}
