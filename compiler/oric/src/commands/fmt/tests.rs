use super::*;
use crate::ir::Span;

#[test]
fn test_get_source_line_single_line() {
    let source = "let x = 42";
    let (line, start) = get_source_line(source, 4).unwrap();
    assert_eq!(line, "let x = 42");
    assert_eq!(start, 0);
}

#[test]
fn test_get_source_line_multi_line() {
    let source = "line1\nline2\nline3";
    // Offset 6 is start of "line2"
    let (line, start) = get_source_line(source, 6).unwrap();
    assert_eq!(line, "line2");
    assert_eq!(start, 6);

    // Offset 12 is start of "line3"
    let (line, start) = get_source_line(source, 12).unwrap();
    assert_eq!(line, "line3");
    assert_eq!(start, 12);
}

#[test]
fn test_get_source_line_offset_beyond_end() {
    let source = "short";
    assert!(get_source_line(source, 100).is_none());
}

#[test]
fn test_get_suggestion_unclosed_delimiter() {
    let error = ParseError::new(ErrorCode::E1003, "unclosed delimiter", Span::new(0, 1));
    let suggestion = get_suggestion(&error);
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("closing"));
}

#[test]
fn test_get_suggestion_missing_equals() {
    let error = ParseError::new(
        ErrorCode::E1001,
        "expected =, found integer",
        Span::new(0, 1),
    );
    let suggestion = get_suggestion(&error);
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("="));
}

#[test]
fn test_get_suggestion_missing_comma() {
    let error = ParseError::new(ErrorCode::E1001, "expected ,, found int", Span::new(0, 1));
    let suggestion = get_suggestion(&error);
    assert!(suggestion.is_some());
    let msg = suggestion.unwrap();
    assert!(msg.contains("comma") || msg.contains("colon"));
}

#[test]
fn test_get_suggestion_unterminated_string() {
    let error = ParseError::new(ErrorCode::E0001, "unterminated string", Span::new(0, 1));
    let suggestion = get_suggestion(&error);
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("\""));
}

#[test]
fn test_get_suggestion_expected_expression() {
    let error = ParseError::new(ErrorCode::E1002, "expected expression", Span::new(0, 1));
    let suggestion = get_suggestion(&error);
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("expression"));
}

#[test]
fn test_get_suggestion_function_definition() {
    let error = ParseError::new(
        ErrorCode::E1006,
        "invalid function definition",
        Span::new(0, 1),
    );
    let suggestion = get_suggestion(&error);
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("@name"));
}

#[test]
fn test_format_parse_error_contains_location() {
    let source = "@broken () -> int 42\n";
    let error = ParseError::new(
        ErrorCode::E1001,
        "expected =, found integer",
        Span::new(18, 20),
    );

    let output = format_parse_error("test.ori", &error, source);

    // Should contain file path
    assert!(output.contains("test.ori"));
    // Should contain line:column
    assert!(output.contains("1:19") || output.contains(":19"));
    // Should contain error code
    assert!(output.contains("E1001"));
    // Should contain the error message
    assert!(output.contains("expected ="));
    // Should contain underline
    assert!(output.contains("^"));
}

#[test]
fn test_format_parse_error_contains_suggestion() {
    let source = "@broken () -> int 42\n";
    let error = ParseError::new(
        ErrorCode::E1001,
        "expected =, found integer",
        Span::new(18, 20),
    );

    let output = format_parse_error("test.ori", &error, source);

    // Should contain help with suggestion
    // Note: "help:" may have ANSI escape codes between "help" and ":"
    assert!(
        output.contains("help:") || output.contains("help\x1b[0m:"),
        "expected 'help:' in output: {output}"
    );
    assert!(output.contains("="));
}

#[test]
fn test_format_parse_errors_summary_single() {
    let source = "@broken () -> int 42\n";
    let errors = vec![ParseError::new(
        ErrorCode::E1001,
        "expected =, found integer",
        Span::new(18, 20),
    )];

    let output = format_parse_errors("test.ori", &errors, source);

    // Should contain the note about fixing syntax errors
    assert!(output.contains("fix the syntax error"));
}

#[test]
fn test_format_parse_errors_summary_multiple() {
    let source = "@broken () 42\n@also () 1\n";
    let errors = vec![
        ParseError::new(ErrorCode::E1001, "error 1", Span::new(10, 12)),
        ParseError::new(ErrorCode::E1001, "error 2", Span::new(24, 25)),
    ];

    let output = format_parse_errors("test.ori", &errors, source);

    // Should mention 2 errors
    assert!(output.contains("2 syntax errors"));
}
