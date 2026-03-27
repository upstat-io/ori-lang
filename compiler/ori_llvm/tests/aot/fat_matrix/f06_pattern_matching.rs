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

// Match with multiple arms using fat values (multi-field variant offset)
#[test]
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

// Multi-field variant with fat first field, scalars after
// Semantic pin: would crash (misaligned pointer) without byte-offset GEP fix
#[test]
fn test_fm_match_multi_field_str_then_ints() {
    assert_aot_success(
        r#"
type Msg = Alert(text: str, level: int, code: int) | Ack(id: int);

@process (m: Msg) -> int =
    match m {
        Alert(text, level, code) -> text.length() + level + code,
        Ack(id) -> id,
    };

@main () -> int = {
    let a = process(m: Alert(text: "error", level: 3, code: 42));
    let b = process(m: Ack(id: 99));
    if a == 50 then {
        if b == 99 then 0 else 1
    } else 2
}
"#,
        "fm_match_multi_str_then_ints",
    );
}

// Fat field in middle position
#[test]
fn test_fm_match_fat_field_middle() {
    assert_aot_success(
        r#"
type Entry = Record(id: int, name: str, score: int) | Empty;

@get_name_len (e: Entry) -> int =
    match e {
        Record(id, name, score) -> name.length() + id + score,
        Empty -> 0,
    };

@main () -> int = {
    let r = get_name_len(e: Record(id: 10, name: "alice", score: 20));
    let e = get_name_len(e: Empty);
    if r == 35 then {
        if e == 0 then 0 else 1
    } else 2
}
"#,
        "fm_match_fat_middle",
    );
}

// Fat field in last position
#[test]
fn test_fm_match_fat_field_last() {
    assert_aot_success(
        r#"
type Tagged = Data(x: int, y: int, label: str) | Empty;

@get_label_len (t: Tagged) -> int =
    match t {
        Data(x, y, label) -> x + y + label.length(),
        Empty -> 0,
    };

@main () -> int = {
    let d = get_label_len(t: Data(x: 1, y: 2, label: "hello"));
    if d == 8 then 0 else 1
}
"#,
        "fm_match_fat_last",
    );
}

// Multiple fat fields in same variant
#[test]
fn test_fm_match_multiple_fat_fields() {
    assert_aot_success(
        r#"
type Pair = Both(first: str, second: str) | Neither;

@total_len (p: Pair) -> int =
    match p {
        Both(first, second) -> first.length() + second.length(),
        Neither -> 0,
    };

@main () -> int = {
    let r = total_len(p: Both(first: "hello", second: "world!"));
    if r == 11 then 0 else 1
}
"#,
        "fm_match_multi_fat_fields",
    );
}

// str + int + str (fat-scalar-fat interleave)
#[test]
fn test_fm_match_fat_scalar_fat() {
    assert_aot_success(
        r#"
type Row = Full(name: str, age: int, city: str) | Blank;

@info (r: Row) -> int =
    match r {
        Full(name, age, city) -> name.length() + age + city.length(),
        Blank -> 0,
    };

@main () -> int = {
    let r = info(r: Full(name: "Bob", age: 30, city: "NYC"));
    if r == 36 then 0 else 1
}
"#,
        "fm_match_fat_scalar_fat",
    );
}

// Heap string (>23 bytes, non-SSO) in multi-field variant
#[test]
fn test_fm_match_heap_str_multi_field() {
    assert_aot_success(
        r#"
type Item = Named(description: str, count: int) | Anon(count: int);

@desc_len (i: Item) -> int =
    match i {
        Named(description, count) -> description.length() + count,
        Anon(count) -> count,
    };

@main () -> int = {
    let heap = "this is a long string that exceeds SSO threshold!!!";
    let r = desc_len(i: Named(description: heap, count: 7));
    if r == 58 then 0 else 1
}
"#,
        "fm_match_heap_str_multi",
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
