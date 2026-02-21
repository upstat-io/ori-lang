//! AOT Iterator Tests
//!
//! End-to-end tests for iterator support in LLVM codegen: constructors
//! (`from_list`, `from_range`), adapters (`map`, `filter`, `take`, `skip`,
//! `enumerate`), consumers (`collect`, `count`), and for-loop over Iterator.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// -----------------------------------------------------------------------
// List.iter() — construct from list
// -----------------------------------------------------------------------

#[test]
fn test_list_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = [10, 20, 30].iter().count();
    if c == 3 then 0 else 1
}
"#,
        "list_iter_count",
    );
}

#[test]
fn test_list_iter_collect() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = [1, 2, 3].iter().collect();
    if result.length() == 3 then 0 else 1
}
"#,
        "list_iter_collect",
    );
}

// -----------------------------------------------------------------------
// Range.iter() — construct from range
// -----------------------------------------------------------------------

#[test]
fn test_range_iter_count() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = (0..5).iter().count();
    if c == 5 then 0 else 1
}
"#,
        "range_iter_count",
    );
}

#[test]
fn test_range_iter_collect() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = (0..5).iter().collect();
    if result.length() == 5 then 0 else 1
}
"#,
        "range_iter_collect",
    );
}

// -----------------------------------------------------------------------
// map adapter
// -----------------------------------------------------------------------

#[test]
fn test_iter_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = [1, 2, 3].iter().map((x) -> x * 2).collect();
    if result.length() == 3 then 0 else 1
}
"#,
        "iter_map",
    );
}

// -----------------------------------------------------------------------
// filter adapter
// -----------------------------------------------------------------------

#[test]
fn test_iter_filter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = [1, 2, 3, 4, 5, 6].iter().filter((x) -> x % 2 == 0).count();
    if result == 3 then 0 else 1
}
"#,
        "iter_filter",
    );
}

// -----------------------------------------------------------------------
// take adapter
// -----------------------------------------------------------------------

#[test]
fn test_iter_take() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = (0..100).iter().take(5).count();
    if c == 5 then 0 else 1
}
"#,
        "iter_take",
    );
}

// -----------------------------------------------------------------------
// skip adapter
// -----------------------------------------------------------------------

#[test]
fn test_iter_skip() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = [10, 20, 30, 40, 50].iter().skip(3).count();
    if c == 2 then 0 else 1
}
"#,
        "iter_skip",
    );
}

// -----------------------------------------------------------------------
// count consumer
// -----------------------------------------------------------------------

#[test]
fn test_iter_count_range() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = (0..10).iter().count();
    if c == 10 then 0 else 1
}
"#,
        "iter_count_range",
    );
}

// -----------------------------------------------------------------------
// for-loop over Iterator
// -----------------------------------------------------------------------

#[test]
fn test_for_over_iterator() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for x in [1, 2, 3].iter() do sum = sum + x;
    if sum == 6 then 0 else 1
}
"#,
        "for_over_iterator",
    );
}

#[test]
fn test_for_over_range_iterator() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in (0..5).iter() do sum = sum + i;
    if sum == 10 then 0 else 1
}
"#,
        "for_over_range_iterator",
    );
}

// -----------------------------------------------------------------------
// Chained adapters
// -----------------------------------------------------------------------

#[test]
fn test_chained_map_filter_take() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = (0..100).iter().map((x) -> x * 2).filter((x) -> x % 4 == 0).take(5).count();
    if c == 5 then 0 else 1
}
"#,
        "chained_map_filter_take",
    );
}
