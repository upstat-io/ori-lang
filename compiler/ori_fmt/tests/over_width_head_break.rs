#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Tests use unwrap/expect for brevity"
)]
//! Over-width expression-body head: break to the next indented line.
//!
//! A function/test whose expression body is a breakable construct (call, list,
//! map, ...) and whose inline head (`= name(`) would overflow the configured
//! width breaks the body to its own indented line, per spec
//! annex-d-formatting.md §Functions / §Return Type. The decision lives in the
//! shared `function_body::emit_expr_body` consumed by both `format_function_body`
//! and `format_test_body`.
//!
//! Regression: the sole width=60 corpus offender was
//! `tests/spec/traits/index_set/updated_method.ori`.

use ori_fmt::{format_module_with_comments_and_config, FormatConfig};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;

/// Format `source` at `max_width`. Panics on parse error (test inputs are valid).
fn fmt_at(source: &str, max_width: usize) -> String {
    let interner = StringInterner::new();
    let lex_output = lex_with_comments(source, &interner);
    let parse_output = ori_parse::parse(&lex_output.tokens, &interner);
    assert!(
        !parse_output.has_errors(),
        "test source failed to parse: {:?}",
        parse_output.errors
    );
    format_module_with_comments_and_config(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
        FormatConfig::with_max_width(max_width),
    )
}

/// Width of the longest line in `s`.
fn max_line_len(s: &str) -> usize {
    s.lines().map(str::len).max().unwrap_or(0)
}

/// The signature line of `func_prefix`'s declaration ends with a bare `=`
/// (the body broke to the next line).
fn body_broke_to_newline(out: &str) -> bool {
    out.lines().any(|l| l.trim_end().ends_with('='))
}

// --- Semantic pin: the repro (floating test, format_test_body path) ---

/// Regression: a floating test with an over-width call head glued
/// `assert_panics(` to the `= ` line, producing a 65-char line at width 60.
#[test]
fn test_floating_test_over_width_call_head_breaks_to_newline_at_60() {
    let src = "@test_list_updated_oob_panics tests _ () -> void = assert_panics(f: () -> [1, 2, 3].updated(key: 5, value: 0));\n";
    let out = fmt_at(src, 60);
    assert!(max_line_len(&out) <= 60, "over-width line at 60:\n{out}");
    assert!(
        body_broke_to_newline(&out),
        "body did not break to newline:\n{out}"
    );
    assert!(
        out.contains("tests _ () -> void =\n"),
        "signature line should end with bare `=`:\n{out}"
    );
}

// --- PF cell: the function-decl path (format_function_body) gets the same fix ---

/// The parallel `format_function_body` path also breaks an over-width call head.
#[test]
fn test_function_over_width_call_head_breaks_to_newline_at_60() {
    let src = "@a_long_enough_floating_function_name () -> void = some_helper_call(arg: 12345);\n";
    let out = fmt_at(src, 60);
    assert!(max_line_len(&out) <= 60, "over-width line at 60:\n{out}");
    assert!(
        body_broke_to_newline(&out),
        "function body did not break to newline:\n{out}"
    );
}

// --- Negative / preservation pins ---

/// A short signature whose `signature = call(` fits keeps the head on the `= `
/// line (the common case must NOT over-break).
#[test]
fn test_short_signature_call_head_stays_inline_at_100() {
    let src = "@foo () -> int = bar(x: 1);\n";
    let out = fmt_at(src, 100);
    assert!(
        out.contains("@foo () -> int = bar(x: 1)"),
        "short body should stay inline-head:\n{out}"
    );
    assert!(!body_broke_to_newline(&out), "should not break:\n{out}");
}

/// A block body keeps its `= {` K&R brace on the signature line (Annex D §671) —
/// the over-width-head break must never apply to block bodies.
#[test]
fn test_block_body_keeps_brace_on_signature_line_at_60() {
    let src = "@a_function_with_a_fairly_long_name () -> int = {\n    let $x = 1;\n    x\n}\n";
    let out = fmt_at(src, 60);
    assert!(
        out.lines().any(|l| l.trim_end().ends_with("= {")),
        "block brace should stay on the `= ` line:\n{out}"
    );
}

/// When the signature itself overflows the width, breaking the body off does not
/// resolve the overflow — the body stays inline (no spurious re-format).
#[test]
fn test_over_width_signature_body_stays_inline_at_60() {
    // The return-type tuple makes the signature intrinsically wider than 60.
    let src = "@wide_sig () -> (int, int, int, int, int, int, int, int, int, int) = foo(x: 1);\n";
    let out = fmt_at(src, 60);
    assert!(!body_broke_to_newline(&out), "should not break:\n{out}");
}

/// A call head that fits at the wider width stays inline-head — the guard fires
/// only when the head genuinely overflows.
#[test]
fn test_fitting_call_head_stays_inline_at_100() {
    let src = "@test_list_updated_oob_panics tests _ () -> void = assert_panics(f: () -> [1, 2, 3].updated(key: 5, value: 0));\n";
    let out = fmt_at(src, 100);
    assert!(
        out.contains("void = assert_panics("),
        "head should stay inline at width 100:\n{out}"
    );
}

// --- Matrix clamp: every breakable body kind fits the width ---

/// Self-verifying matrix: for every breakable body kind, a long-signature
/// declaration formats with no line exceeding width 60. Kinds with a long
/// first-line head (call, method-call, named struct, binary) break to a newline;
/// kinds with a short head (list `[`, map `{`, tuple `(`) stay inline-head. The
/// invariant that holds for ALL of them is: no over-width line.
#[test]
fn test_all_breakable_body_kinds_fit_width_60() {
    let sig = "@a_long_enough_floating_function_name () -> void = ";
    let cases = [
        ("call", "some_helper_call(argument: 123456789);"),
        ("list", "[111111, 222222, 333333, 444444, 555555, 666666];"),
        ("map", "{aaaaaa: 1, bbbbbb: 2, cccccc: 3, dddddd: 4};"),
        ("method", "receiver_value.some_method(argument: 123456789);"),
        ("binary", "first_operand + second_operand + third_operand;"),
        ("struct", "SomeNamedStruct { field_one: 1, field_two: 2 };"),
        ("tuple", "(first_elem, second_elem, third_elem, fourth_e);"),
    ];
    let mut n = 0;
    for (label, body) in cases {
        let src = format!("{sig}{body}\n");
        let out = fmt_at(&src, 60);
        assert!(
            max_line_len(&out) <= 60,
            "{label}: over-width line at 60:\n{out}"
        );
        n += 1;
    }
    assert_eq!(n, cases.len(), "not all breakable-kind cells exercised");
}

/// A non-breakable atomic body too wide for its own line stays inline-head
/// (breaking would not help) — the guard's "newline layout fits" condition
/// returns false. No spurious re-route, no oscillation.
#[test]
fn test_non_breakable_wide_string_body_stays_inline_at_60() {
    let src = "@a_long_enough_floating_function_name () -> void = \"a string literal that is itself far wider than sixty chars and cannot break\";\n";
    let out = fmt_at(src, 60);
    assert!(!body_broke_to_newline(&out), "should not break:\n{out}");
}

// --- Idempotence ---

/// Formatting the over-width-head break twice reproduces the first result.
#[test]
fn test_over_width_head_break_is_idempotent() {
    let src = "@test_list_updated_oob_panics tests _ () -> void = assert_panics(f: () -> [1, 2, 3].updated(key: 5, value: 0));\n";
    for width in [60usize, 80, 100, 120] {
        let once = fmt_at(src, width);
        let twice = fmt_at(&once, width);
        assert_eq!(once, twice, "non-idempotent at width {width}");
    }
}
