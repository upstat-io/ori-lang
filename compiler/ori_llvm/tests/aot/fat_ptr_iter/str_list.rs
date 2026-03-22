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
