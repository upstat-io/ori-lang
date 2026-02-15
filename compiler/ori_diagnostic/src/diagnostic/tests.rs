use super::*;

#[test]
fn test_diagnostic_builder() {
    let diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("test error")
        .with_label(Span::new(0, 5), "here")
        .with_note("some context")
        .with_suggestion("try this");

    assert_eq!(diag.code, ErrorCode::E1001);
    assert_eq!(diag.message, "test error");
    assert!(diag.is_error());
    assert_eq!(diag.labels.len(), 1);
    assert!(diag.labels[0].is_primary);
    assert_eq!(diag.notes.len(), 1);
    assert_eq!(diag.suggestions.len(), 1);
}

#[test]
fn test_type_mismatch_helper() {
    let diag = type_mismatch(Span::new(10, 15), "int", "str", "return value");

    assert_eq!(diag.code, ErrorCode::E2001);
    assert!(diag.message.contains("int"));
    assert!(diag.message.contains("str"));
    assert_eq!(diag.primary_span(), Some(Span::new(10, 15)));
}

#[test]
fn test_unclosed_delimiter() {
    let diag = unclosed_delimiter(Span::new(0, 1), Span::new(10, 10), '(');

    assert_eq!(diag.code, ErrorCode::E1003);
    assert_eq!(diag.labels.len(), 2);
    assert!(diag.labels[0].is_primary);
    assert!(!diag.labels[1].is_primary);
}

#[test]
fn test_missing_pattern_arg() {
    let diag = missing_pattern_arg(Span::new(0, 10), "map", "over");

    assert_eq!(diag.code, ErrorCode::E1009);
    assert!(diag.message.contains("over"));
    assert!(diag.message.contains("map"));
    assert!(!diag.suggestions.is_empty());
}

#[test]
fn test_diagnostic_display() {
    let diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("test error")
        .with_label(Span::new(0, 5), "here");

    let output = diag.to_string();
    assert!(output.contains("error"));
    assert!(output.contains("E1001"));
    assert!(output.contains("test error"));
}

#[test]
fn test_diagnostic_display_format() {
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("test error")
        .with_label(Span::new(0, 5), "primary")
        .with_secondary_label(Span::new(10, 15), "secondary")
        .with_note("a note")
        .with_suggestion("a suggestion");

    let output = diag.to_string();
    assert!(output.contains("error [E2001]: test error"));
    assert!(output.contains("--> "));
    assert!(output.contains("primary"));
    assert!(output.contains("secondary"));
    assert!(output.contains("= note: a note"));
    assert!(output.contains("= help: a suggestion"));
}

#[test]
fn test_diagnostic_salsa_traits() {
    use std::collections::HashSet;

    let d1 = Diagnostic::error(ErrorCode::E1001).with_message("test");
    let d2 = Diagnostic::error(ErrorCode::E1001).with_message("test");
    let d3 = Diagnostic::error(ErrorCode::E1002).with_message("other");

    // Eq
    assert_eq!(d1, d2);
    assert_ne!(d1, d3);

    // Hash
    let mut set = HashSet::new();
    set.insert(d1.clone());
    set.insert(d2); // duplicate
    set.insert(d3);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_source_info_creation() {
    let info = SourceInfo::new("src/lib.ori", "let x = 42");
    assert_eq!(info.path, "src/lib.ori");
    assert_eq!(info.content, "let x = 42");
}

#[test]
fn test_label_cross_file() {
    let same_file = Label::primary(Span::new(0, 5), "in this file");
    assert!(!same_file.is_cross_file());
    assert!(same_file.source_info.is_none());

    let cross_file = Label::secondary_cross_file(
        Span::new(10, 20),
        "defined here",
        SourceInfo::new("src/lib.ori", "@foo () -> int"),
    );
    assert!(cross_file.is_cross_file());
    assert!(!cross_file.is_primary);
    assert_eq!(cross_file.source_info.as_ref().unwrap().path, "src/lib.ori");
}

#[test]
fn test_diagnostic_with_cross_file_label() {
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(0, 10), "expected `int`, found `str`")
        .with_cross_file_secondary_label(
            Span::new(0, 20),
            "return type defined here",
            SourceInfo::new("src/lib.ori", "@get_name () -> str"),
        );

    assert_eq!(diag.labels.len(), 2);
    assert!(!diag.labels[0].is_cross_file()); // same-file primary
    assert!(diag.labels[1].is_cross_file()); // cross-file secondary
}

#[test]
fn test_diagnostic_display_cross_file() {
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(0, 10), "expected `int`")
        .with_cross_file_secondary_label(
            Span::new(0, 20),
            "defined here",
            SourceInfo::new("src/lib.ori", "@foo () -> str"),
        );

    let output = diag.to_string();
    // Should contain ::: marker for cross-file labels
    assert!(output.contains(":::"));
    // Should contain the file path
    assert!(output.contains("src/lib.ori"));
    // Should still have --> for same-file primary
    assert!(output.contains("-->"));
}
