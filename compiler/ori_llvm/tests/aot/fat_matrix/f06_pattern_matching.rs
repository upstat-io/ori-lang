//! F06: Pattern Matching — fat pointer types used in match expressions.
//!
//! Tests decision tree codegen, extractvalue for payloads, and RC handling
//! when fat pointer values are destructured or compared in patterns.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T11/T16: Match on Option<str> — Some vs None
#[test]
fn test_fm_match_option_str() {
    assert_aot_success(
        r#"
@check (opt: Option<str>) -> int =
    match opt {
        Some(s) -> s.length(),
        None -> 0,
    };

@main () -> int = {
    let a = check(opt: Some("hello"));
    let b = check(opt: None);
    if a == 5 then {
        if b == 0 then 0 else 1
    } else 2
}
"#,
        "fm_match_option_str",
    );
}

// T15: Match on Option<int>
#[test]
fn test_fm_match_option_int() {
    assert_aot_success(
        r#"
@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt {
        Some(v) -> v,
        None -> default,
    };

@main () -> int = {
    let a = unwrap_or(opt: Some(42), default: 0);
    let b = unwrap_or(opt: None, default: 99);
    if a == 42 then {
        if b == 99 then 0 else 1
    } else 2
}
"#,
        "fm_match_option_int",
    );
}

// T10: Match on unit-variant sum type (no fat payload)
#[test]
fn test_fm_match_unit_sum() {
    assert_aot_success(
        r#"
type Color = Red | Green | Blue;

@to_num (c: Color) -> int =
    match c {
        Red -> 1,
        Green -> 2,
        Blue -> 3,
    };

@main () -> int = {
    let r = to_num(c: Red);
    let g = to_num(c: Green);
    let b = to_num(c: Blue);
    if r + g + b == 6 then 0 else 1
}
"#,
        "fm_match_unit_sum",
    );
}

// T11: Match on sum type with fat payload
#[test]
fn test_fm_match_fat_payload() {
    assert_aot_success(
        r#"
type Value = Text(content: str) | Number(n: int);

@get_len (v: Value) -> int =
    match v {
        Text(content) -> content.length(),
        Number(n) -> n,
    };

@main () -> int = {
    let a = get_len(v: Text(content: "hello"));
    let b = get_len(v: Number(n: 42));
    if a == 5 then {
        if b == 42 then 0 else 1
    } else 2
}
"#,
        "fm_match_fat_payload",
    );
}

// T9: Match destructuring struct with fat fields
#[test]
fn test_fm_match_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@get_name_len (n: Named) -> int =
    match n {
        { name, id: _ } -> name.length(),
    };

@main () -> int = {
    let n = Named { name: "alice", id: 1 };
    if get_name_len(n: n) == 5 then 0 else 1
}
"#,
        "fm_match_struct_fat",
    );
}

// T18: Match destructuring tuple with fat element
#[test]
fn test_fm_match_tuple_mixed() {
    assert_aot_success(
        r#"
@first_len (t: (str, int)) -> int =
    match t {
        (s, _) -> s.length(),
    };

@main () -> int = {
    let t = ("hello", 42);
    if first_len(t: t) == 5 then 0 else 1
}
"#,
        "fm_match_tuple_mixed",
    );
}

// Match with multiple arms using fat values
#[test]
#[ignore = "BUG-04-02: multi-field variant match crash — misaligned pointer in ori_rc_dec"]
fn test_fm_match_multi_arm_fat() {
    assert_aot_success(
        r#"
type Shape = Circle(radius: int) | Rect(name: str, w: int, h: int);

@area (s: Shape) -> int =
    match s {
        Circle(radius) -> radius * radius,
        Rect(name, w, h) -> w * h,
    };

@main () -> int = {
    let a = area(s: Circle(radius: 5));
    let b = area(s: Rect(name: "box", w: 3, h: 4));
    if a == 25 then {
        if b == 12 then 0 else 1
    } else 2
}
"#,
        "fm_match_multi_arm_fat",
    );
}

// Nested match on fat values
#[test]
fn test_fm_match_nested_option_str() {
    assert_aot_success(
        r#"
@nested_check (outer: Option<int>, inner: Option<str>) -> int =
    match outer {
        Some(n) -> match inner {
            Some(s) -> n + s.length(),
            None -> n,
        },
        None -> 0,
    };

@main () -> int = {
    let a = nested_check(outer: Some(10), inner: Some("hello"));
    let b = nested_check(outer: Some(10), inner: None);
    let c = nested_check(outer: None, inner: Some("x"));
    if a == 15 then {
        if b == 10 then {
            if c == 0 then 0 else 1
        } else 2
    } else 3
}
"#,
        "fm_match_nested_option_str",
    );
}
