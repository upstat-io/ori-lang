//! Set Method AOT Tests
//!
//! Tests for Set<T> method coverage in AOT compilation.
//! Currently ignored because `iter().collect()` to Set is not yet
//! implemented in the LLVM backend (`__collect_set` unresolved).
//! These tests will pass once set collection is implemented.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    if s.len() == 3 then 0 else 1
}
"#,
        "set_length",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_is_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [].iter().collect();
    if s.is_empty() then 0 else 1
}
"#,
        "set_is_empty",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_contains() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    if s.contains(2) then 0 else 1
}
"#,
        "set_contains",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_insert() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2].iter().collect();
    let s2 = s.insert(3);
    if s2.len() == 3 then 0 else 1
}
"#,
        "set_insert",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_remove() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    let s2 = s.remove(2);
    if s2.len() == 2 then 0 else 1
}
"#,
        "set_remove",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_union() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2].iter().collect();
    let b: Set<int> = [2, 3].iter().collect();
    if a.union(b).len() == 3 then 0 else 1
}
"#,
        "set_union",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_intersection() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [2, 3, 4].iter().collect();
    if a.intersection(b).len() == 2 then 0 else 1
}
"#,
        "set_intersection",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_difference() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [2].iter().collect();
    if a.difference(b).len() == 2 then 0 else 1
}
"#,
        "set_difference",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_to_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    if s.to_list().len() == 3 then 0 else 1
}
"#,
        "set_to_list",
    );
}

#[test]
#[ignore = "AOT collect-to-set not yet implemented"]
fn test_aot_set_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    if s.iter().count() == 3 then 0 else 1
}
"#,
        "set_iter_count",
    );
}
