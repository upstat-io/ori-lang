//! `#compile_fail` / `#skip` / `#fail` attribute round-trip formatting.
//!
//! Pins the keyed-argument re-emit for `#compile_fail` and the shared
//! literal-escaping of every test-attribute string. Each keyed shape must
//! (a) format to the faithful attribute text, (b) re-parse (Spec: Annex D —
//! formatting preserves observable semantics, so the output must re-lex/parse),
//! and (c) be idempotent (`format(format(x)) == format(x)`).

use ori_fmt::format_module;
use ori_ir::StringInterner;

/// Parse `source` and format it through the real `format_module` entry point.
fn fmt(source: &str) -> String {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let output = ori_parse::parse(&tokens, &interner);
    assert!(
        !output.has_errors(),
        "source failed to parse:\n{source}\nerrors: {:?}",
        output.errors
    );
    format_module(&output.module, &output.arena, &interner)
}

/// Format `source`, then assert the formatted output re-parses clean (the
/// `#compile_fail` bare-attr regression is exactly a re-parse break).
fn assert_reparses(source: &str) -> String {
    let formatted = fmt(source);
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(&formatted, &interner);
    let output = ori_parse::parse(&tokens, &interner);
    assert!(
        !output.has_errors(),
        "formatted output failed to re-parse:\n{formatted}\nerrors: {:?}",
        output.errors
    );
    formatted
}

/// Format twice; the second format must equal the first (idempotency pin).
fn assert_idempotent(source: &str) -> String {
    let first = fmt(source);
    let second = fmt(&first);
    assert_eq!(first, second, "formatting is not idempotent for:\n{source}");
    first
}

/// One floating test carrying `attr` as its sole attribute. Substitutes into
/// the parse-valid `.ori` template (`fixtures/attr_roundtrip_cell.ori`) rather
/// than assembling the program via `format!` (Spec: Annex D — the fixture
/// stays independently parseable/formattable Ori source).
fn test_cell(attr: &str) -> String {
    const TEMPLATE: &str = include_str!("fixtures/attr_roundtrip_cell.ori");
    const PLACEHOLDER: &str = "#skip(\"PLACEHOLDER\")";
    TEMPLATE.replacen(PLACEHOLDER, attr, 1)
}

/// Two floating-test attributes stacked on one test. Substitutes into the
/// two-placeholder `.ori` template (`fixtures/attrs_multi_roundtrip_cell.ori`).
fn test_cell_multi(attr_a: &str, attr_b: &str) -> String {
    const TEMPLATE: &str = include_str!("fixtures/attrs_multi_roundtrip_cell.ori");
    TEMPLATE
        .replacen("#skip(\"PLACEHOLDER_1\")", attr_a, 1)
        .replacen("#skip(\"PLACEHOLDER_2\")", attr_b, 1)
}

// The exact failing case: code-only must keep its keyed arg, not
// collapse to a bare `#compile_fail` that no longer re-parses.
#[test]
fn compile_fail_code_only_keeps_keyed_arg() {
    let out = assert_reparses(&test_cell("#compile_fail(code: \"E2001\")"));
    assert!(
        out.contains("#compile_fail(code: \"E2001\")"),
        "code-only lost its keyed arg:\n{out}"
    );
    assert!(
        !out.contains("#compile_fail\n"),
        "emitted a bare attr:\n{out}"
    );
}

// Keyed-field coverage.
#[test]
fn compile_fail_message_only_is_shorthand() {
    let out = assert_reparses(&test_cell("#compile_fail(message: \"boom\")"));
    assert!(
        out.contains("#compile_fail(\"boom\")"),
        "message-alone should format shorthand:\n{out}"
    );
}

#[test]
fn compile_fail_code_and_message_keyed_in_order() {
    let out = assert_reparses(&test_cell(
        "#compile_fail(message: \"boom\", code: \"E2001\")",
    ));
    assert!(
        out.contains("#compile_fail(code: \"E2001\", message: \"boom\")"),
        "canonical order is code, message:\n{out}"
    );
}

#[test]
fn compile_fail_two_field_subsets_keyed() {
    // code+line, code+column, message+line, message+column, line+column.
    for (input, expected) in [
        (
            "#compile_fail(code: \"E2001\", line: 3)",
            "#compile_fail(code: \"E2001\", line: 3)",
        ),
        (
            "#compile_fail(code: \"E2001\", column: 7)",
            "#compile_fail(code: \"E2001\", column: 7)",
        ),
        (
            "#compile_fail(message: \"m\", line: 3)",
            "#compile_fail(message: \"m\", line: 3)",
        ),
        (
            "#compile_fail(message: \"m\", column: 7)",
            "#compile_fail(message: \"m\", column: 7)",
        ),
        (
            "#compile_fail(line: 3, column: 7)",
            "#compile_fail(line: 3, column: 7)",
        ),
    ] {
        let out = assert_reparses(&test_cell(input));
        assert!(out.contains(expected), "2-field subset {input}:\n{out}");
    }
}

#[test]
fn compile_fail_three_field_subsets_keyed() {
    for (input, expected) in [
        (
            "#compile_fail(code: \"E2001\", message: \"m\", line: 3)",
            "#compile_fail(code: \"E2001\", message: \"m\", line: 3)",
        ),
        (
            "#compile_fail(code: \"E2001\", message: \"m\", column: 7)",
            "#compile_fail(code: \"E2001\", message: \"m\", column: 7)",
        ),
        (
            "#compile_fail(code: \"E2001\", line: 3, column: 7)",
            "#compile_fail(code: \"E2001\", line: 3, column: 7)",
        ),
        (
            "#compile_fail(message: \"m\", line: 3, column: 7)",
            "#compile_fail(message: \"m\", line: 3, column: 7)",
        ),
    ] {
        let out = assert_reparses(&test_cell(input));
        assert!(out.contains(expected), "3-field subset {input}:\n{out}");
    }
}

#[test]
fn compile_fail_all_four_fields_canonical_order() {
    let out = assert_reparses(&test_cell(
        "#compile_fail(column: 7, line: 3, message: \"m\", code: \"E2001\")",
    ));
    assert!(
        out.contains("#compile_fail(code: \"E2001\", message: \"m\", line: 3, column: 7)"),
        "all four fields must emit in canonical order:\n{out}"
    );
}

#[test]
fn compile_fail_line_only_and_column_only_keyed() {
    let line_out = assert_reparses(&test_cell("#compile_fail(line: 3)"));
    assert!(line_out.contains("#compile_fail(line: 3)"), "{line_out}");
    let col_out = assert_reparses(&test_cell("#compile_fail(column: 7)"));
    assert!(col_out.contains("#compile_fail(column: 7)"), "{col_out}");
}

// Multi-error: both entries round-trip, one attr per entry.
#[test]
fn compile_fail_two_entries_both_round_trip() {
    let src = test_cell_multi(
        "#compile_fail(code: \"E2001\")",
        "#compile_fail(code: \"E3002\")",
    );
    let out = assert_reparses(&src);
    assert!(out.contains("#compile_fail(code: \"E2001\")"), "{out}");
    assert!(out.contains("#compile_fail(code: \"E3002\")"), "{out}");
}

#[test]
fn compile_fail_mixed_shape_multi_error() {
    // One message-alone (shorthand) entry + one code entry (keyed).
    let src = test_cell_multi(
        "#compile_fail(message: \"boom\")",
        "#compile_fail(code: \"E2001\")",
    );
    let out = assert_reparses(&src);
    assert!(
        out.contains("#compile_fail(\"boom\")"),
        "shorthand entry:\n{out}"
    );
    assert!(
        out.contains("#compile_fail(code: \"E2001\")"),
        "keyed entry:\n{out}"
    );
}

// Alias canonicalization: parser aliases fold to canonical names.
#[test]
fn compile_fail_msg_alias_folds_to_message() {
    let out = assert_reparses(&test_cell("#compile_fail(msg: \"x\")"));
    assert!(
        out.contains("#compile_fail(\"x\")"),
        "`msg` alias should format as canonical message (shorthand when alone):\n{out}"
    );
}

#[test]
fn compile_fail_col_alias_folds_to_column() {
    let out = assert_reparses(&test_cell("#compile_fail(col: 7)"));
    assert!(
        out.contains("#compile_fail(column: 7)"),
        "`col` alias should format as canonical column:\n{out}"
    );
}

// Edge cases: escaping via the shared literal helper.
#[test]
fn compile_fail_message_escapes_quote_and_newline() {
    // Source embeds a literal `"` and newline in a message-alone attr; the
    // formatter re-emits them escaped in the shorthand form and re-parses.
    let out = assert_reparses(&test_cell("#compile_fail(message: \"a\\\"b\\nc\")"));
    assert!(
        out.contains("#compile_fail(\"a\\\"b\\nc\")"),
        "message-alone must be re-escaped shorthand:\n{out}"
    );
}

#[test]
fn compile_fail_message_escapes_standalone_backslash() {
    // Source embeds a literal `\` in a message-alone attr; the formatter must
    // re-emit it as `\\` in the shorthand form and re-parse identically.
    let out = assert_reparses(&test_cell("#compile_fail(message: \"a\\\\b\")"));
    assert!(
        out.contains("#compile_fail(\"a\\\\b\")"),
        "standalone backslash must be re-escaped as \\\\ in shorthand:\n{out}"
    );
}

#[test]
fn compile_fail_keyed_message_escapes_quote_and_newline() {
    // With a second field the attr stays keyed, exercising the keyed message
    // emit site's escaping (not the shorthand branch).
    let out = assert_reparses(&test_cell(
        "#compile_fail(code: \"E2001\", message: \"a\\\"b\\nc\")",
    ));
    assert!(
        out.contains("#compile_fail(code: \"E2001\", message: \"a\\\"b\\nc\")"),
        "keyed message must be re-escaped:\n{out}"
    );
}

#[test]
fn skip_reason_escapes_quote_and_newline() {
    let out = assert_reparses(&test_cell("#skip(\"why \\\" and \\n newline\")"));
    assert!(
        out.contains("#skip(\"why \\\" and \\n newline\")"),
        "#skip reason must be re-escaped:\n{out}"
    );
}

#[test]
fn fail_message_escapes_quote_and_newline() {
    let out = assert_reparses(&test_cell("#fail(\"panic \\\" and \\n newline\")"));
    assert!(
        out.contains("#fail(\"panic \\\" and \\n newline\")"),
        "#fail message must be re-escaped:\n{out}"
    );
}

#[test]
fn skip_backend_llvm_round_trips() {
    // A per-backend skip must survive format->reparse (the backend + reason are
    // load-bearing; dropping either re-parses to a different test disposition).
    let out = assert_reparses(&test_cell("#skip(backend: \"llvm\", reason: \"flaky\")"));
    assert!(
        out.contains("#skip(backend: \"llvm\", reason: \"flaky\")"),
        "llvm backend skip must round-trip faithfully:\n{out}"
    );
}

#[test]
fn skip_backend_interpreter_round_trips() {
    let out = assert_reparses(&test_cell(
        "#skip(backend: \"interpreter\", reason: \"slow\")",
    ));
    assert!(
        out.contains("#skip(backend: \"interpreter\", reason: \"slow\")"),
        "interpreter backend skip must round-trip faithfully:\n{out}"
    );
}

#[test]
fn skip_backend_reason_escapes_quote_and_newline() {
    let out = assert_reparses(&test_cell(
        "#skip(backend: \"llvm\", reason: \"a\\\"b\\nc\")",
    ));
    assert!(
        out.contains("#skip(backend: \"llvm\", reason: \"a\\\"b\\nc\")"),
        "backend-skip reason must be re-escaped:\n{out}"
    );
}

#[test]
fn skip_backend_both_backends_stack() {
    // Two per-backend skips on one test must each emit their own
    // #skip(backend: ...) line and both round-trip.
    let src = test_cell_multi(
        "#skip(backend: \"llvm\", reason: \"a\")",
        "#skip(backend: \"interpreter\", reason: \"b\")",
    );
    let out = assert_reparses(&src);
    assert!(
        out.contains("#skip(backend: \"llvm\", reason: \"a\")"),
        "{out}"
    );
    assert!(
        out.contains("#skip(backend: \"interpreter\", reason: \"b\")"),
        "{out}"
    );
}

#[test]
fn compile_fail_empty_string_preserved() {
    let out = assert_reparses(&test_cell("#compile_fail(message: \"\")"));
    assert!(
        out.contains("#compile_fail(\"\")"),
        "empty message preserved:\n{out}"
    );
}

#[test]
fn compile_fail_zero_line_column_emitted() {
    let out = assert_reparses(&test_cell("#compile_fail(line: 0, column: 0)"));
    assert!(
        out.contains("#compile_fail(line: 0, column: 0)"),
        "Some(0) line/column must emit as 0, not drop:\n{out}"
    );
}

// Pins: idempotency across every shape + parse-back.
#[test]
fn compile_fail_idempotent_across_shapes() {
    for attr in [
        "#compile_fail(code: \"E2001\")",
        "#compile_fail(message: \"boom\")",
        "#compile_fail(code: \"E2001\", message: \"boom\", line: 3, column: 7)",
        "#compile_fail(line: 0, column: 0)",
        "#compile_fail(message: \"a\\\"b\\nc\")",
    ] {
        assert_idempotent(&test_cell(attr));
    }
}
