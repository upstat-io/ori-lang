//! F19: Break/Continue — early exit from loops with fat pointer values in scope.
//!
//! Tests cleanup on break (remaining iterator elements and in-scope fat values),
//! continue semantics with fat values, and correct RC in both paths.

use crate::util::assert_aot_success;

// Break from loop with [str] — remaining elements must be cleaned up
#[test]
fn test_fm_break_str_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for s in ["alpha", "beta", "gamma"] do {
        if s.length() == 4 then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "fm_break_str_list",
    );
}

// Continue in loop with [str]
#[test]
fn test_fm_continue_str_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for s in ["a", "bb", "ccc", "dd", "eeeee"] do {
        if s.length() == 2 then continue;
        total = total + s.length();
    };
    if total == 9 then 0 else 1
}
"#,
        "fm_continue_str_list",
    );
}

// Break with fat value created inside loop
#[test]
fn test_fm_break_inner_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = 0;
    for i in [1, 2, 3, 4, 5] do {
        let s = "hello";
        if i == 3 then break;
        result = result + s.length();
    };
    if result == 10 then 0 else 1
}
"#,
        "fm_break_inner_fat",
    );
}

// Continue with fat value created inside loop
#[test]
fn test_fm_continue_inner_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for i in [1, 2, 3, 4, 5] do {
        let s = "hello";
        if i == 3 then continue;
        total = total + s.length();
    };
    if total == 20 then 0 else 1
}
"#,
        "fm_continue_inner_fat",
    );
}

// Break from for-yield with [str]
#[test]
fn test_fm_break_yield_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lengths = for s in ["abc", "de", "fghi", "jk"] yield {
        if s.length() == 4 then break;
        s.length()
    };
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    if total == 5 then 0 else 1
}
"#,
        "fm_break_yield_str",
    );
}

// Nested loops with break — outer fat values preserved
#[test]
fn test_fm_break_nested_fat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for s in ["hello", "world"] do {
        for i in [1, 2, 3] do {
            if i == 2 then break;
            total = total + s.length();
        };
    };
    if total == 10 then 0 else 1
}
"#,
        "fm_break_nested_fat",
    );
}
