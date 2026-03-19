//! F07: Branching — fat pointer types used in if/else expressions.
//!
//! Tests select vs branch emission, phi node merging for fat pointer results,
//! and correct RC on both branches.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T5: String from if/else branches
#[test]
fn test_fm_branch_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let s = if flag then "hello" else "world";
    if s.length() == 5 then 0 else 1
}
"#,
        "fm_branch_str",
    );
}

// T5: Heap string from conditional
#[test]
fn test_fm_branch_str_heap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let s = if flag
        then "abcdefghijklmnopqrstuvwxyz1234"
        else "short";
    if s.length() == 30 then 0 else 1
}
"#,
        "fm_branch_str_heap",
    );
}

// T6: List from conditional
#[test]
fn test_fm_branch_list_scalar() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let xs = if flag then [1, 2, 3] else [4, 5];
    if xs.length() == 3 then 0 else 1
}
"#,
        "fm_branch_list_scalar",
    );
}

// T7: List of strings from conditional
#[test]
fn test_fm_branch_list_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = false;
    let xs = if flag then ["a", "b"] else ["x", "y", "z"];
    if xs.length() == 3 then 0 else 1
}
"#,
        "fm_branch_list_fat",
    );
}

// T8: Struct from conditional
#[test]
fn test_fm_branch_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let flag = true;
    let p = if flag then Point { x: 1, y: 2 } else Point { x: 3, y: 4 };
    if p.x == 1 then 0 else 1
}
"#,
        "fm_branch_struct_scalar",
    );
}

// T9: Struct with fat fields from conditional
#[test]
fn test_fm_branch_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let flag = true;
    let n = if flag
        then Named { name: "alice", id: 1 }
        else Named { name: "bob", id: 2 };
    if n.name.length() == 5 then 0 else 1
}
"#,
        "fm_branch_struct_fat",
    );
}

// T15: Option<int> from conditional
#[test]
fn test_fm_branch_option_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let opt = if flag then Some(42) else None;
    if is_some(option: opt) then 0 else 1
}
"#,
        "fm_branch_option_int",
    );
}

// T16: Option<str> from conditional
#[test]
fn test_fm_branch_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let opt = if flag then Some("hello") else None;
    if is_some(option: opt) then 0 else 1
}
"#,
        "fm_branch_option_str",
    );
}

// T17: Map from conditional
#[test]
fn test_fm_branch_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let m = if flag then {"a": 1} else {"b": 2, "c": 3};
    if m.length() == 1 then 0 else 1
}
"#,
        "fm_branch_map",
    );
}

// T18: Tuple from conditional
#[test]
fn test_fm_branch_tuple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let t = if flag then ("hello", 1) else ("world", 2);
    let (s, n) = t;
    if s.length() + n == 6 then 0 else 1
}
"#,
        "fm_branch_tuple",
    );
}

// Nested branching with fat values
#[test]
fn test_fm_branch_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = true;
    let b = false;
    let s = if a then {
        if b then "inner_true" else "inner_false"
    } else "outer_false";
    if s.length() == 11 then 0 else 1
}
"#,
        "fm_branch_nested",
    );
}
