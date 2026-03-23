//! `[str]` iteration tests — T1 (heap strings) and T1b (mixed SSO/heap).

use crate::util::assert_aot_success;

#[test]
fn test_str_list_full_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    if total == 109 then 0 else 1
}
"#,
        "str_list_full_iteration",
    );
}

#[test]
fn test_str_list_partial_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "stop marker string that is also long enough to be heap",
        "third long string that should not be visited early break"
    ];
    let count = 0;
    for w in words do {
        if w.starts_with(prefix: "stop") then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "str_list_partial_break",
    );
}

#[test]
fn test_str_list_yield_lengths() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = for w in words yield w.len();
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    if total == 109 then 0 else 1
}
"#,
        "str_list_yield_lengths",
    );
}

#[test]
fn test_str_list_passed_to_two_functions() {
    assert_aot_success(
        r#"
@count_chars (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = count_chars(words: words);
    let b = count_chars(words: words);
    if a == 109 then {
        if b == 109 then 0 else 1
    } else 1
}
"#,
        "str_list_passed_to_two_functions",
    );
}

// T1b: mixed SSO/heap strings — semantic pin for SSO check in elem_dec_fn

#[test]
fn test_str_list_mixed_sso_heap() {
    // Mix of short strings (<= 23 bytes, SSO inline) and long strings (> 23 bytes, heap).
    // The elem_dec_fn thunk must correctly skip SSO strings (no RC to dec) and
    // only dec heap strings. If the SSO check is broken, this leaks or double-frees.
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "hi",
        "this is a very long string that exceeds SSO threshold",
        "ok",
        "another very long string that also exceeds the threshold",
        "x"
    ];
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    // "hi"=2 + 53 + "ok"=2 + 56 + "x"=1 = 114
    if total == 114 then 0 else 1
}
"#,
        "str_list_mixed_sso_heap",
    );
}

// TPR-04-010: Semantic pin — slice-backed string through string methods

/// `substring(...).to_uppercase()` must not crash on slice-backed strings.
/// Without the `is_slice_cap` guard, `ori_rc_is_unique` dereferences an
/// interior pointer → misaligned access → abort.
#[test]
fn test_substring_to_uppercase() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "the quick brown fox jumps over the lazy dog";
    let sub = s.substring(start: 4, end: 43);
    let upper = sub.to_uppercase();
    if upper == "QUICK BROWN FOX JUMPS OVER THE LAZY DOG" then 0 else 1
}
"#,
        "substring_to_uppercase",
    );
}

/// `split(...)[0].to_lowercase()` — slice from split through a string method.
#[test]
fn test_split_first_to_lowercase() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "AAAAAAAAAAAAAAAAAAAAAAAAA|BBBBBBBBBBBBBBBBBBBBBBBBB";
    let parts = s.split(sep: "|");
    let first = parts[0];
    let lower = first.to_lowercase();
    if lower == "aaaaaaaaaaaaaaaaaaaaaaaaa" then 0 else 1
}
"#,
        "split_first_to_lowercase",
    );
}

// TPR-04-011: Semantic pin — repeat(1) must not double-free

/// `s.repeat(count: 1)` must return an owned string, not alias the original.
/// Without the fix, RC double-free when both original and result are live.
#[test]
fn test_repeat_one_no_double_free() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "the quick brown fox jumps over the lazy dog";
    let t = s.repeat(count: 1);
    // Both s and t must stay live — no double-free
    let total = s.len() + t.len();
    if total == 86 then 0 else 1
}
"#,
        "repeat_one_no_double_free",
    );
}

/// `substring(...).repeat(count: 1)` — slice through repeat.
#[test]
fn test_substring_repeat_one() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "the quick brown fox jumps over the lazy dog";
    let sub = s.substring(start: 4, end: 43);
    let t = sub.repeat(count: 1);
    let total = sub.len() + t.len();
    if total == 78 then 0 else 1
}
"#,
        "substring_repeat_one",
    );
}

// TPR-04-012: Semantic pin — concat("") must not double-free

/// `heap_str + ""` must return an owned result, not alias the operand.
#[test]
fn test_concat_empty_right_no_double_free() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "the quick brown fox jumps over the lazy dog";
    let t = s + "";
    // Both s and t must stay live — no double-free
    let total = s.len() + t.len();
    if total == 86 then 0 else 1
}
"#,
        "concat_empty_right_no_double_free",
    );
}

/// `"" + heap_str` — empty left operand.
#[test]
fn test_concat_empty_left_no_double_free() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "the quick brown fox jumps over the lazy dog";
    let t = "" + s;
    let total = s.len() + t.len();
    if total == 86 then 0 else 1
}
"#,
        "concat_empty_left_no_double_free",
    );
}
