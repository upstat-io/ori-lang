//! F08: For Loop Iteration — iterating over collections containing fat pointer elements.
//!
//! This is the J15 bug area: iterating over `[str]` and other fat-pointer
//! collections required proper `elem_dec_fn` and iterator ownership contracts.
//! Tests both for-do and for-yield with fat pointer element types.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T6: Iterate over [int] (for-do)
#[test]
fn test_fm_for_list_scalar() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for x in [10, 20, 30] do {
        total = total + x;
    };
    if total == 60 then 0 else 1
}
"#,
        "fm_for_list_scalar",
    );
}

// T7: Iterate over [str] (for-do)
#[test]
fn test_fm_for_list_str_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for s in ["hello", "world", "test"] do {
        total = total + s.length();
    };
    if total == 14 then 0 else 1
}
"#,
        "fm_for_list_str_do",
    );
}

// T7: Iterate over [str] (for-yield)
#[test]
fn test_fm_for_list_str_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lengths = for s in ["hello", "world"] yield s.length();
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    if total == 10 then 0 else 1
}
"#,
        "fm_for_list_str_yield",
    );
}

// T7: Iterate over [str] with break
#[test]
fn test_fm_for_list_str_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for s in ["alpha", "beta", "gamma", "delta"] do {
        if s.length() == 4 then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "fm_for_list_str_break",
    );
}

// T7: Iterate over [str] twice (RC correctness)
#[test]
fn test_fm_for_list_str_two_iterations() {
    assert_aot_success(
        r#"
@sum_lengths (words: [str]) -> int = {
    let total = 0;
    for s in words do {
        total = total + s.length();
    };
    total
}

@main () -> int = {
    let words = ["hello", "world"];
    let a = sum_lengths(words: words);
    let b = sum_lengths(words: words);
    if a + b == 20 then 0 else 1
}
"#,
        "fm_for_list_str_two_iterations",
    );
}

// T8: Iterate over [Point] (struct with scalar fields)
#[test]
fn test_fm_for_list_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let total = 0;
    for p in [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }] do {
        total = total + p.x + p.y;
    };
    if total == 10 then 0 else 1
}
"#,
        "fm_for_list_struct_scalar",
    );
}

// T9: Iterate over [Named] (struct with fat fields)
#[test]
fn test_fm_for_list_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let total = 0;
    for n in [Named { name: "alice", id: 1 }, Named { name: "bob", id: 2 }] do {
        total = total + n.name.length() + n.id;
    };
    if total == 11 then 0 else 1
}
"#,
        "fm_for_list_struct_fat",
    );
}

// T17: Iterate over map (for-do)
#[test]
fn test_fm_for_map_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for (k, v) in {"a": 10, "b": 20} do {
        total = total + v;
    };
    if total == 30 then 0 else 1
}
"#,
        "fm_for_map_str",
    );
}

// Nested for loops with fat elements
#[test]
fn test_fm_for_nested_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for s1 in ["hello", "world"] do {
        for s2 in ["a", "bb"] do {
            total = total + s1.length() + s2.length();
        };
    };
    if total == 26 then 0 else 1
}
"#,
        "fm_for_nested_fat",
    );
}

// For-yield with fat element transformation
#[test]
fn test_fm_for_yield_fat_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lengths = for s in ["abc", "de", "f"] yield s.length();
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    if total == 6 then 0 else 1
}
"#,
        "fm_for_yield_fat_transform",
    );
}
