//! For-yield on `Option<T>` iterable — break/continue support.
//!
//! When the iterable is `Option<T>` (not `[Option<T>]`), the ARC lowering
//! dispatches to `lower_for_yield_option()`. This codepath must install a
//! `LoopContext` so that `break`/`continue` inside the body work correctly.
//!
//! Matrix:
//!   Type:    str (heap, RC-tracked), int (scalar)
//!   Pattern: break, break-value, continue, continue-value, guard+break

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// -----------------------------------------------------------------------
// str (heap-allocated, RC-tracked)
// -----------------------------------------------------------------------

/// `for x in Some(str) yield { break }` → empty list.
#[test]
fn test_for_yield_option_str_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        break
    };
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_option_str_break",
    );
}

/// `for x in Some(str) yield { break value }` → [value].
#[test]
fn test_for_yield_option_str_break_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        break 42
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_option_str_break_value",
    );
}

/// `for x in Some(str) yield { continue }` → empty list (skip element).
#[test]
fn test_for_yield_option_str_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        continue
    };
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_option_str_continue",
    );
}

/// `for x in Some(str) yield { continue value }` → [value].
#[test]
fn test_for_yield_option_str_continue_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        continue 42
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_option_str_continue_value",
    );
}

/// `for x in None yield { break }` on None — break never reached, empty list.
#[test]
fn test_for_yield_option_str_break_none() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt: Option<str> = None;
    let result = for x in opt yield {
        break
    };
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_option_str_break_none",
    );
}

/// Conditional break inside option for-yield: `if cond then break`.
#[test]
fn test_for_yield_option_str_conditional_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        if x.len() > 10 then break;
        x.len()
    };
    // break fires (len > 10), so result is empty
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_option_str_conditional_break",
    );
}

/// Conditional continue-value inside option for-yield.
#[test]
fn test_for_yield_option_str_conditional_continue_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        if x.len() > 10 then continue 99;
        x.len()
    };
    // continue 99 fires, so result is [99]
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_option_str_conditional_continue_value",
    );
}

// -----------------------------------------------------------------------
// int (scalar, no RC)
// -----------------------------------------------------------------------

/// `for x in Some(int) yield { break }` → empty list.
#[test]
fn test_for_yield_option_int_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some(42);
    let result = for x in opt yield {
        break
    };
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_option_int_break",
    );
}

/// `for x in Some(int) yield { break value }` → [value].
#[test]
fn test_for_yield_option_int_break_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some(42);
    let result = for x in opt yield {
        break x * 2
    };
    // result should be [84]
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_option_int_break_value",
    );
}

/// `for x in Some(int) yield { continue value }` → [value].
#[test]
fn test_for_yield_option_int_continue_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some(42);
    let result = for x in opt yield {
        continue x + 1
    };
    // result should be [43]
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_option_int_continue_value",
    );
}

// -----------------------------------------------------------------------
// Semantic pin: option for-yield break produces correct value
// -----------------------------------------------------------------------

/// Semantic pin: `break value` in option for-yield yields `[value]`, not error.
/// This test ONLY passes when `LoopContext` is properly installed in the
/// option for-yield path. Without it, `break` emits a warning and exits 1.
#[test]
fn test_for_yield_option_break_semantic_pin() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some("this string exceeds SSO threshold by being very long indeed");
    let result = for x in opt yield {
        break x.len()
    };
    // With LoopContext: result = [len], length == 1, exit 0
    // Without LoopContext: warning + exit 1 (failure)
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_option_break_semantic_pin",
    );
}
