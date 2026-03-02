//! Set Method AOT Tests
//!
//! Tests for Set<T> method coverage in AOT compilation.
//! Set construction via `[...].iter().collect()` → `__collect_set` is
//! wired in the LLVM backend.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
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
