use super::*;
use crate::ErrorCode;
use ori_ir::Span;

fn sample_diagnostic() -> Diagnostic {
    Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch: expected `int`, found `str`")
        .with_label(Span::new(10, 15), "expected `int`")
        .with_secondary_label(Span::new(0, 5), "defined here")
        .with_note("int and str are incompatible")
        .with_suggestion("use `int(x)` to convert")
}

// Fallback (no source) tests

#[test]
fn test_terminal_emitter_no_color() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

    emitter.emit(&sample_diagnostic());
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("error"));
    assert!(text.contains("[E2001]"));
    assert!(text.contains("type mismatch"));
    assert!(text.contains("expected `int`"));
    assert!(text.contains("note:"));
    assert!(text.contains("help:"));
}

#[test]
fn test_terminal_emitter_with_color() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, true);

    emitter.emit(&sample_diagnostic());
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\x1b["));
    assert!(text.contains("E2001"));
}

#[test]
fn test_emit_all() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

    let diagnostics = vec![
        Diagnostic::error(ErrorCode::E1001).with_message("error 1"),
        Diagnostic::error(ErrorCode::E2001).with_message("error 2"),
    ];

    emitter.emit_all(&diagnostics);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("error 1"));
    assert!(text.contains("error 2"));
}

#[test]
fn test_emit_summary_errors() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

    emitter.emit_summary(2, 1);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("2 previous errors"));
    assert!(text.contains("1 warning"));
}

#[test]
fn test_emit_summary_single_error() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

    emitter.emit_summary(1, 0);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("previous error"));
    assert!(!text.contains("errors"));
}

#[test]
fn test_emit_summary_warnings_only() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

    emitter.emit_summary(0, 3);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("3 warnings"));
}

// ColorMode tests

#[test]
fn test_color_mode_auto_with_tty() {
    assert!(ColorMode::Auto.should_use_colors(true));
}

#[test]
fn test_color_mode_auto_without_tty() {
    assert!(!ColorMode::Auto.should_use_colors(false));
}

#[test]
fn test_color_mode_always_ignores_tty() {
    assert!(ColorMode::Always.should_use_colors(false));
    assert!(ColorMode::Always.should_use_colors(true));
}

#[test]
fn test_color_mode_never_ignores_tty() {
    assert!(!ColorMode::Never.should_use_colors(false));
    assert!(!ColorMode::Never.should_use_colors(true));
}

#[test]
fn test_color_mode_default_is_auto() {
    assert_eq!(ColorMode::default(), ColorMode::Auto);
}

#[test]
fn test_with_color_mode_always() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, false);

    emitter.emit(&sample_diagnostic());
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\x1b["));
}

#[test]
fn test_with_color_mode_never() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, true);

    emitter.emit(&sample_diagnostic());
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("\x1b["));
}

#[test]
fn test_with_color_mode_auto_tty() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Auto, true);

    emitter.emit(&sample_diagnostic());
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\x1b["));
}

#[test]
fn test_with_color_mode_auto_no_tty() {
    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Auto, false);

    emitter.emit(&sample_diagnostic());
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("\x1b["));
}

// Cross-file label tests (fallback)

#[test]
fn test_terminal_emitter_cross_file_label() {
    use crate::SourceInfo;

    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(10, 20), "expected `int`, found `str`")
        .with_cross_file_secondary_label(
            Span::new(0, 19),
            "return type defined here",
            SourceInfo::new("src/lib.ori", "@get_name () -> str"),
        );

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains(":::"), "Expected ::: marker, got:\n{text}");
    assert!(
        text.contains("src/lib.ori"),
        "Expected file path, got:\n{text}"
    );
    assert!(
        text.contains("return type defined here"),
        "Expected label message, got:\n{text}"
    );
    assert!(text.contains("-->"), "Expected --> marker, got:\n{text}");
}

#[test]
fn test_terminal_emitter_cross_file_with_colors() {
    use crate::SourceInfo;

    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(10, 20), "expected `int`")
        .with_cross_file_secondary_label(
            Span::new(0, 19),
            "defined here",
            SourceInfo::new("src/lib.ori", "@foo () -> str"),
        );

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, true);
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains(":::"));
    assert!(text.contains("src/lib.ori"));
    assert!(text.contains("\x1b[1m")); // Bold ANSI code
}

// Snippet rendering tests

#[test]
fn test_snippet_single_line() {
    // Line 1: "let x = 42\n"      (11 bytes: 0..11)
    // Line 2: "let y = \"hello\"\n" (16 bytes: 11..27)
    // Line 3: "let z = x + y"     (13 bytes: 27..40)
    //                  ^^^^^       span 35..40 = "x + y" (col 9..14)
    let source = "let x = 42\nlet y = \"hello\"\nlet z = x + y";
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(35, 40), "expected `int`, found `str`");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("demo.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();

    // Should contain file:line:col header (col 9 = 'x' in "x + y")
    assert!(
        text.contains("--> demo.ori:3:9"),
        "Expected location header, got:\n{text}"
    );
    // Should contain the source line
    assert!(
        text.contains("let z = x + y"),
        "Expected source line, got:\n{text}"
    );
    // Should contain line number
    assert!(text.contains("3 |"), "Expected line number, got:\n{text}");
    // Should contain underline carets
    assert!(text.contains("^^^^^"), "Expected underline, got:\n{text}");
    // Should contain label message
    assert!(
        text.contains("expected `int`, found `str`"),
        "Expected label message, got:\n{text}"
    );
    // Should NOT contain byte offsets
    assert!(
        !text.contains("35..40"),
        "Should not contain byte offsets, got:\n{text}"
    );
}

#[test]
fn test_snippet_point_span() {
    let source = "let x = 42";
    let diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected")
        .with_label(Span::new(4, 4), "here");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    // Point span should still render at least one caret
    assert!(
        text.contains('^'),
        "Expected at least one caret, got:\n{text}"
    );
}

#[test]
fn test_snippet_multiple_labels_same_line() {
    let source = "let result = add(x, y)";
    //                               ^  ^  <- two labels on same line
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(17, 18), "this is `str`")
        .with_secondary_label(Span::new(20, 21), "this is `int`");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(
        text.contains("this is `str`"),
        "Expected primary label, got:\n{text}"
    );
    assert!(
        text.contains("this is `int`"),
        "Expected secondary label, got:\n{text}"
    );
    // Primary uses ^, secondary uses -
    assert!(text.contains('^'), "Expected ^ for primary, got:\n{text}");
    assert!(text.contains('-'), "Expected - for secondary, got:\n{text}");
}

#[test]
fn test_snippet_multiple_labels_different_lines() {
    let source = "let x: int = 42\nlet y: str = x";
    //            ^^^^^             ^^^^^^^^^^^^^^
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(16, 30), "expected `str`, found `int`")
        .with_secondary_label(Span::new(0, 15), "defined as `int` here");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(
        text.contains("1 |") && text.contains("2 |"),
        "Expected both line numbers, got:\n{text}"
    );
}

#[test]
fn test_snippet_cross_file_with_source() {
    use crate::SourceInfo;

    let source = "let x: int = get_name()";
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(13, 23), "expected `int`, found `str`")
        .with_cross_file_secondary_label(
            Span::new(0, 19),
            "return type defined here",
            SourceInfo::new("src/lib.ori", "@get_name () -> str"),
        );

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("src/main.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    // Should have --> for main file
    assert!(
        text.contains("--> src/main.ori:1:14"),
        "Expected main file location, got:\n{text}"
    );
    // Should have ::: for cross-file
    assert!(
        text.contains("::: src/lib.ori:1:1"),
        "Expected cross-file location, got:\n{text}"
    );
    // Should show cross-file source
    assert!(
        text.contains("@get_name () -> str"),
        "Expected cross-file source line, got:\n{text}"
    );
    assert!(
        text.contains("return type defined here"),
        "Expected cross-file label, got:\n{text}"
    );
}

#[test]
fn test_snippet_unicode_alignment() {
    // Greek letters: each is 2 bytes in UTF-8, but 1 character column
    // Line 1: "let αβ = 42\n"  (14 bytes: l=1, e=1, t=1, ' '=1, α=2, β=2, ' '=1, '='=1, ' '=1, 4=1, 2=1, \n=1)
    // Line 2: "let γ = αβ + \"hello\""
    //   l=1 e=1 t=1 ' '=1 γ=2 ' '=1 '='=1 ' '=1 α=2 β=2 ' '=1 '+'=1 ' '=1 '"'=1 h=1 e=1 l=1 l=1 o=1 '"'=1
    //   Line 2 starts at byte 14
    //   "hello" (with quotes) starts at byte 14 + 16 = 30, ends at 30 + 7 = 37
    let source = "let αβ = 42\nlet γ = αβ + \"hello\"";
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(30, 37), "expected `int`");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    // Should contain the source line with unicode
    assert!(
        text.contains("let γ = αβ + \"hello\""),
        "Expected unicode source line, got:\n{text}"
    );
    // Underline should be 7 chars wide (for "hello" including quotes)
    assert!(text.contains("^^^^^^^"), "Expected 7 carets, got:\n{text}");
}

#[test]
fn test_snippet_gutter_width_two_digits() {
    // Create source with 10+ lines so gutter needs 2 digits
    let lines: Vec<String> = (1..=12).map(|i| format!("let x{i} = {i}")).collect();
    let source = lines.join("\n");
    // Error on line 12
    let line12_start = source.rfind("let x12").unwrap() as u32;
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("error on line 12")
        .with_label(Span::new(line12_start, line12_start + 7), "here");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(&source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    // Line 12 should be right-aligned with 2-digit gutter
    assert!(
        text.contains("12 |"),
        "Expected 2-digit line number, got:\n{text}"
    );
}

#[test]
fn test_snippet_with_colors() {
    let source = "let x = 42\nlet y = x + \"hello\"";
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(12, 19), "expected `int`");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(
        text.contains("\x1b["),
        "Expected ANSI color codes, got:\n{text}"
    );
    assert!(text.contains("expected `int`"));
}

#[test]
fn test_snippet_no_colors() {
    let source = "let x = 42\nlet y = x + \"hello\"";
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(12, 19), "expected `int`");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(
        !text.contains("\x1b["),
        "Should not have ANSI codes, got:\n{text}"
    );
}

#[test]
fn test_fallback_without_source() {
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(10, 15), "expected `int`");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(
        text.contains("10..15"),
        "Expected byte offset fallback, got:\n{text}"
    );
    assert!(
        !text.contains(" | "),
        "Should not have gutter in fallback, got:\n{text}"
    );
}

#[test]
fn test_snippet_notes_and_suggestions() {
    let source = "let x: int = \"hello\"";
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch")
        .with_label(Span::new(13, 20), "expected `int`, found `str`")
        .with_note("int and str are incompatible")
        .with_suggestion("use `int()` to convert");

    let mut output = Vec::new();
    let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
        .with_source(source)
        .with_file_path("test.ori");
    emitter.emit(&diag);
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(
        text.contains("= note: int and str are incompatible"),
        "Expected note, got:\n{text}"
    );
    assert!(
        text.contains("= help: use `int()` to convert"),
        "Expected suggestion, got:\n{text}"
    );
}

// digit_count tests

#[test]
fn test_digit_count() {
    assert_eq!(digit_count(0), 1);
    assert_eq!(digit_count(1), 1);
    assert_eq!(digit_count(9), 1);
    assert_eq!(digit_count(10), 2);
    assert_eq!(digit_count(99), 2);
    assert_eq!(digit_count(100), 3);
    assert_eq!(digit_count(999), 3);
    assert_eq!(digit_count(1000), 4);
}
