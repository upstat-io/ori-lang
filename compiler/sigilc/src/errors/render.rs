// Error rendering for the Sigil compiler
//
// Provides both simple text rendering and rich terminal rendering with
// colors and source context.

use super::{Diagnostic, LabelStyle, Level};

/// Render a diagnostic as a simple text string (no colors)
pub fn render_simple(diag: &Diagnostic) -> String {
    let mut output = String::new();

    // Header line: error[E0001]: message
    output.push_str(&format!(
        "{}[{}]: {}\n",
        diag.level,
        diag.code.as_string(),
        diag.message
    ));

    // Labels with location
    for label in &diag.labels {
        let prefix = match label.style {
            LabelStyle::Primary => "-->",
            LabelStyle::Secondary => "   ",
        };
        output.push_str(&format!(
            " {} {}:{}..{}\n",
            prefix, label.span.filename, label.span.range.start, label.span.range.end
        ));
        if !label.message.is_empty() {
            output.push_str(&format!("     | {}\n", label.message));
        }
    }

    // Notes
    for note in &diag.notes {
        output.push_str(&format!(" = note: {}\n", note));
    }

    // Help
    for help in &diag.help {
        output.push_str(&format!(" = help: {}\n", help));
    }

    output
}

/// Render a diagnostic with source context
pub fn render_with_source(diag: &Diagnostic, source: &str) -> String {
    let mut output = String::new();

    // Header line
    output.push_str(&format!(
        "{}[{}]: {}\n",
        diag.level,
        diag.code.as_string(),
        diag.message
    ));

    // Process labels
    for label in &diag.labels {
        // Calculate line and column
        let (line_num, col, line_start, line_end) = find_line_info(source, label.span.range.start);

        let prefix = match label.style {
            LabelStyle::Primary => "-->",
            LabelStyle::Secondary => "   ",
        };

        // Location
        output.push_str(&format!(
            " {} {}:{}:{}\n",
            prefix, label.span.filename, line_num, col
        ));

        // Source line
        let line_content = &source[line_start..line_end];
        let line_num_width = format!("{}", line_num).len();

        output.push_str(&format!("{:>width$} |\n", "", width = line_num_width));
        output.push_str(&format!(
            "{:>width$} | {}\n",
            line_num,
            line_content,
            width = line_num_width
        ));

        // Underline
        let underline_start = label.span.range.start - line_start;
        let underline_len = (label.span.range.end - label.span.range.start).max(1);
        let underline_char = match label.style {
            LabelStyle::Primary => '^',
            LabelStyle::Secondary => '-',
        };

        output.push_str(&format!(
            "{:>width$} | {:>start$}{} {}\n",
            "",
            "",
            std::iter::repeat_n(underline_char, underline_len).collect::<String>(),
            label.message,
            width = line_num_width,
            start = underline_start
        ));
    }

    // Notes
    for note in &diag.notes {
        output.push_str(&format!(" = note: {}\n", note));
    }

    // Help
    for help in &diag.help {
        output.push_str(&format!(" = help: {}\n", help));
    }

    output
}

/// Find line number, column, and line boundaries for a byte offset
fn find_line_info(source: &str, offset: usize) -> (usize, usize, usize, usize) {
    let offset = offset.min(source.len());
    let mut line_num = 1;
    let mut line_start = 0;

    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line_num += 1;
            line_start = i + 1;
        }
    }

    let col = offset - line_start + 1;

    // Find end of current line
    let line_end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());

    (line_num, col, line_start, line_end)
}

/// Render diagnostic with ANSI colors for terminal output
pub fn render_colored(diag: &Diagnostic, source: &str) -> String {
    // ANSI color codes
    const RED: &str = "\x1b[31m";
    const YELLOW: &str = "\x1b[33m";
    const BLUE: &str = "\x1b[34m";
    const CYAN: &str = "\x1b[36m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";

    let level_color = match diag.level {
        Level::Error => RED,
        Level::Warning => YELLOW,
        Level::Note => CYAN,
        Level::Help => BLUE,
    };

    let mut output = String::new();

    // Header line with color
    output.push_str(&format!(
        "{}{}{}{}: {}{}{}\n",
        BOLD, level_color, diag.level, RESET, BOLD, diag.message, RESET
    ));

    // Process labels
    for label in &diag.labels {
        let (line_num, col, line_start, line_end) = find_line_info(source, label.span.range.start);

        let prefix = match label.style {
            LabelStyle::Primary => "-->",
            LabelStyle::Secondary => "   ",
        };

        // Location
        output.push_str(&format!(
            " {}{}{}{}:{}:{}{}\n",
            BLUE, prefix, RESET, label.span.filename, line_num, col, RESET
        ));

        // Source line
        let line_content = &source[line_start..line_end];
        let line_num_width = format!("{}", line_num).len();

        output.push_str(&format!(
            "{}{:>width$}{} {}{}{}\n",
            BLUE,
            "",
            "|",
            RESET,
            "",
            RESET,
            width = line_num_width
        ));
        output.push_str(&format!(
            "{}{:>width$}{} {}{}{}\n",
            BLUE,
            line_num,
            "|",
            RESET,
            line_content,
            RESET,
            width = line_num_width
        ));

        // Underline with color
        let underline_start = label.span.range.start - line_start;
        let underline_len = (label.span.range.end - label.span.range.start).max(1);
        let (underline_char, underline_color) = match label.style {
            LabelStyle::Primary => ('^', RED),
            LabelStyle::Secondary => ('-', BLUE),
        };

        output.push_str(&format!(
            "{}{:>width$}{} {:>start$}{}{}{} {}\n",
            BLUE,
            "",
            "|",
            "",
            underline_color,
            std::iter::repeat_n(underline_char, underline_len).collect::<String>(),
            RESET,
            label.message,
            width = line_num_width,
            start = underline_start
        ));
    }

    // Notes
    for note in &diag.notes {
        output.push_str(&format!(" {} = note:{} {}\n", CYAN, RESET, note));
    }

    // Help
    for help in &diag.help {
        output.push_str(&format!(" {} = help:{} {}\n", BLUE, RESET, help));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{codes::ErrorCode, Span};

    #[test]
    fn test_render_simple() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch")
            .with_label(Span::new("test.si", 10..15), "expected int");

        let rendered = render_simple(&diag);
        assert!(rendered.contains("error[E3001]"));
        assert!(rendered.contains("type mismatch"));
        assert!(rendered.contains("test.si"));
    }

    #[test]
    fn test_render_with_source() {
        let source = "@f () -> int = \"hello\"";
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch")
            .with_label(Span::new("test.si", 15..22), "expected int, found str");

        let rendered = render_with_source(&diag, source);
        assert!(rendered.contains("error[E3001]"));
        assert!(rendered.contains("type mismatch"));
        assert!(rendered.contains("\"hello\""));
        assert!(rendered.contains("^"));
    }

    #[test]
    fn test_find_line_info_single_line() {
        let source = "hello world";
        let (line, col, start, end) = find_line_info(source, 6);
        assert_eq!(line, 1);
        assert_eq!(col, 7);
        assert_eq!(start, 0);
        assert_eq!(end, 11);
    }

    #[test]
    fn test_find_line_info_multi_line() {
        let source = "line 1\nline 2\nline 3";
        let (line, col, _, _) = find_line_info(source, 10);
        assert_eq!(line, 2);
        assert_eq!(col, 4);
    }

    #[test]
    fn test_render_with_notes_and_help() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch")
            .with_note("types must match exactly")
            .with_help("consider using str() to convert");

        let rendered = render_simple(&diag);
        assert!(rendered.contains("note: types must match"));
        assert!(rendered.contains("help: consider using"));
    }
}
