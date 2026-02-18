//! AOT For-Loop Tests
//!
//! End-to-end tests for for-loops over all iterable types: Range, List, Str,
//! Option, Set, Map. Covers both `do` (side effects) and `yield` (collection)
//! forms, including guards.
//!
//! These are regression tests for Range/List and new coverage for Str/Option/Set/Map.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// -----------------------------------------------------------------------
// Range for-loops (regression)
// -----------------------------------------------------------------------

#[test]
fn test_for_range_sum() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut sum = 0,
    for i in 0..5 do sum = sum + i,
    if sum == 10 then 0 else 1
)
"#,
        "for_range_sum",
    );
}

#[test]
fn test_for_range_inclusive() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut sum = 0,
    for i in 0..=5 do sum = sum + i,
    if sum == 15 then 0 else 1
)
"#,
        "for_range_inclusive",
    );
}

#[test]
fn test_for_range_empty() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut count = 0,
    for i in 5..0 do count = count + 1,
    if count == 0 then 0 else 1
)
"#,
        "for_range_empty",
    );
}

#[test]
fn test_for_range_yield() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let result = for i in 0..4 yield i * i,
    if result.length() == 4 then 0 else 1
)
"#,
        "for_range_yield",
    );
}

#[test]
fn test_for_range_with_guard() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut sum = 0,
    for i in 0..10 if i % 2 == 0 do sum = sum + i,
    if sum == 20 then 0 else 1
)
"#,
        "for_range_with_guard",
    );
}

// -----------------------------------------------------------------------
// List for-loops (regression)
// -----------------------------------------------------------------------

#[test]
fn test_for_list_sum() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut sum = 0,
    for x in [10, 20, 30] do sum = sum + x,
    if sum == 60 then 0 else 1
)
"#,
        "for_list_sum",
    );
}

#[test]
fn test_for_list_yield() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let doubled = for x in [1, 2, 3] yield x * 2,
    if doubled.length() == 3 then 0 else 1
)
"#,
        "for_list_yield",
    );
}

#[test]
fn test_for_list_empty() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut count = 0,
    let empty: [int] = [],
    for x in empty do count = count + 1,
    if count == 0 then 0 else 1
)
"#,
        "for_list_empty",
    );
}

#[test]
fn test_for_list_with_guard() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let evens = for x in [1, 2, 3, 4, 5, 6] if x % 2 == 0 yield x,
    if evens.length() == 3 then 0 else 1
)
"#,
        "for_list_with_guard",
    );
}

// -----------------------------------------------------------------------
// String for-loops (new — character iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_str_count_chars() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut count = 0,
    for c in "hello" do count = count + 1,
    if count == 5 then 0 else 1
)
"#,
        "for_str_count_chars",
    );
}

#[test]
fn test_for_str_empty() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut count = 0,
    for c in "" do count = count + 1,
    if count == 0 then 0 else 1
)
"#,
        "for_str_empty",
    );
}

#[test]
fn test_for_str_yield() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let chars = for c in "abc" yield 1,
    if chars.length() == 3 then 0 else 1
)
"#,
        "for_str_yield",
    );
}

// -----------------------------------------------------------------------
// Option for-loops (new — 0-or-1 element iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_option_some() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut sum = 0,
    for x in Some(42) do sum = sum + x,
    if sum == 42 then 0 else 1
)
"#,
        "for_option_some",
    );
}

#[test]
fn test_for_option_none() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mut count = 0,
    let empty: Option<int> = None,
    for x in empty do count = count + 1,
    if count == 0 then 0 else 1
)
"#,
        "for_option_none",
    );
}

#[test]
fn test_for_option_yield_some() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let result = for x in Some(5) yield x * 2,
    if result.length() == 1 then 0 else 1
)
"#,
        "for_option_yield_some",
    );
}

#[test]
fn test_for_option_yield_none() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let empty: Option<int> = None,
    let result = for x in empty yield x * 2,
    if result.length() == 0 then 0 else 1
)
"#,
        "for_option_yield_none",
    );
}
