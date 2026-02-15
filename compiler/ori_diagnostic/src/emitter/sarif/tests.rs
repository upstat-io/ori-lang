use super::*;
use crate::{ErrorCode, SourceInfo};
use ori_ir::Span;

/// Test fixture version - intentionally stable for snapshot testing.
/// This is NOT the compiler version; it's a constant for test reproducibility.
const TEST_TOOL_VERSION: &str = "0.1.0";

fn sample_diagnostic() -> Diagnostic {
    Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch: expected `int`, found `str`")
        .with_label(Span::new(10, 15), "expected `int`")
        .with_secondary_label(Span::new(0, 5), "defined here")
        .with_note("int and str are incompatible")
        .with_suggestion("use `int(x)` to convert")
}

#[test]
fn test_sarif_emitter_basic() {
    let mut output = Vec::new();
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION)
        .with_artifact("src/main.ori")
        .with_source("let x = 42\nlet y = \"hello\"");

    emitter.emit(&sample_diagnostic());
    emitter.finish();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    // Check SARIF structure
    assert!(text.contains("\"$schema\":"));
    assert!(text.contains("sarif-schema-2.1.0"));
    assert!(text.contains("\"version\": \"2.1.0\""));
    assert!(text.contains("\"name\": \"oric\""));
    assert!(text.contains("\"ruleId\": \"E2001\""));
    assert!(text.contains("\"level\": \"error\""));
    assert!(text.contains("\"startLine\":"));
    assert!(text.contains("\"startColumn\":"));
}

#[test]
fn test_sarif_emitter_multiple_diagnostics() {
    let mut output = Vec::new();
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION);

    let diag1 = Diagnostic::error(ErrorCode::E1001).with_message("parse error");
    let diag2 = Diagnostic::warning(ErrorCode::E3001).with_message("pattern warning");

    emitter.emit(&diag1);
    emitter.emit(&diag2);
    emitter.finish();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("\"ruleId\": \"E1001\""));
    assert!(text.contains("\"ruleId\": \"E3001\""));
    assert!(text.contains("\"level\": \"error\""));
    assert!(text.contains("\"level\": \"warning\""));
}

#[test]
fn test_sarif_emitter_related_locations() {
    let mut output = Vec::new();
    let source = "let x = 10\nlet y = x + z";
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION).with_source(source);

    let diag = Diagnostic::error(ErrorCode::E2003)
        .with_message("unknown identifier `z`")
        .with_label(Span::new(22, 23), "not found")
        .with_secondary_label(Span::new(4, 5), "similar: `x`");

    emitter.emit(&diag);
    emitter.finish();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("\"locations\":"));
    assert!(text.contains("\"relatedLocations\":"));
    assert!(text.contains("not found"));
    assert!(text.contains("similar: `x`"));
}

#[test]
fn test_sarif_emitter_line_column_conversion() {
    let mut output = Vec::new();
    let source = "line1\nline2\nline3";
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION).with_source(source);

    // Span at "line2" (offset 6-11)
    let diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("error on line 2")
        .with_label(Span::new(6, 11), "here");

    emitter.emit(&diag);
    emitter.finish();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    // line2 starts at line 2, column 1
    assert!(text.contains("\"startLine\": 2"));
    assert!(text.contains("\"startColumn\": 1"));
}

#[test]
fn test_sarif_emitter_rules_deduplication() {
    let mut output = Vec::new();
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION);

    // Two diagnostics with same error code
    let diag1 = Diagnostic::error(ErrorCode::E2001).with_message("error 1");
    let diag2 = Diagnostic::error(ErrorCode::E2001).with_message("error 2");

    emitter.emit(&diag1);
    emitter.emit(&diag2);
    emitter.finish();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    // Should only have one rule definition for E2001
    let rule_count = text.matches("\"id\": \"E2001\"").count();
    assert_eq!(rule_count, 1, "rules should be deduplicated");

    // But should have two results
    let result_count = text.matches("\"ruleId\": \"E2001\"").count();
    assert_eq!(result_count, 2, "should have two results");
}

#[test]
fn test_sarif_emitter_escapes_json() {
    let mut output = Vec::new();
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION);

    let diag =
        Diagnostic::error(ErrorCode::E1001).with_message("error with \"quotes\" and\nnewline");

    emitter.emit(&diag);
    emitter.finish();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    // Should be properly escaped
    assert!(text.contains("\\\"quotes\\\""));
    assert!(text.contains("\\n"));
}

#[test]
fn test_sarif_cross_file_label_positions() {
    // Main file: single line — "let x: int = get_name()"
    // Cross-file: three lines — offsets are relative to THIS content, not the main file
    let main_source = "let x: int = get_name()";
    let cross_source = "module lib\n\n@get_name () -> str = \"hello\"";
    //                   ^0         ^11 ^12       ^21

    let mut output = Vec::new();
    let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION)
        .with_artifact("src/main.ori")
        .with_source(main_source);

    // Primary label in main file at "get_name()" (offset 13-23, line 1)
    // Cross-file label in lib.ori at "@get_name () -> str" (offset 12-31, line 3)
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(13, 23), "expected `int`, found `str`")
        .with_cross_file_secondary_label(
            Span::new(12, 31),
            "return type defined here",
            SourceInfo::new("src/lib.ori", cross_source),
        );

    emitter.emit(&diag);
    emitter.finish();

    let text = String::from_utf8(output).unwrap();

    // Primary label: "get_name()" is at line 1, col 14 in the main file
    assert!(
        text.contains("\"startLine\": 1"),
        "primary label should be on line 1"
    );

    // Cross-file label: "@get_name () -> str" starts at offset 12 in cross_source,
    // which is line 3, col 1 (after "module lib\n\n").
    // If the bug were still present, this would compute positions from main_source
    // and produce wrong results.
    assert!(
        text.contains("\"startLine\": 3"),
        "cross-file label should be on line 3 of lib.ori, got:\n{text}"
    );
    assert!(
        text.contains("src/lib.ori"),
        "cross-file label should reference lib.ori"
    );
}
