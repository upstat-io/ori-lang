//! F18: Multiple Values — multiple fat pointer values of the same type in scope.
//!
//! Tests RC tracking correctness when multiple values compete for cleanup,
//! drop ordering, and no interference between independent values.

use crate::util::assert_aot_success;

// Multiple heap strings in scope
#[test]
fn test_fm_multi_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "abcdefghijklmnopqrstuvwxyz1234";
    let b = "another_heap_string_for_testing";
    let c = "third_heap_string_goes_here_now";
    if a.length() + b.length() + c.length() == 92 then 0 else 1
}
"#,
        "fm_multi_str",
    );
}

// Multiple lists in scope
#[test]
fn test_fm_multi_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = [4, 5, 6, 7];
    let zs = ["a", "bb"];
    if xs.length() + ys.length() + zs.length() == 9 then 0 else 1
}
"#,
        "fm_multi_list",
    );
}

// Multiple structs with fat fields
#[test]
fn test_fm_multi_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let a = Named { name: "alice", id: 1 };
    let b = Named { name: "bob", id: 2 };
    if a.name.length() + b.name.length() == 8 then 0 else 1
}
"#,
        "fm_multi_struct_fat",
    );
}

// Multiple maps
#[test]
fn test_fm_multi_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m1 = {"a": 1, "b": 2};
    let m2 = {"x": 10, "y": 20, "z": 30};
    if m1.length() + m2.length() == 5 then 0 else 1
}
"#,
        "fm_multi_map",
    );
}

// Mixed fat types in same scope
#[test]
fn test_fm_multi_mixed() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let xs = [1, 2, 3];
    let n = Named { name: "alice", id: 1 };
    let m = {"x": 10};
    let result = s.length() + xs.length() + n.name.length() + m.length();
    if result == 39 then 0 else 1
}
"#,
        "fm_multi_mixed",
    );
}
