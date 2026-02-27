//! Parse error formatting with source code snippets, colored output, and suggestions.

use ori_diagnostic::{span_utils, ErrorCode};

use crate::parser::ParseError;
use std::fmt::Write as _;
use std::io::IsTerminal;

/// ANSI color codes for terminal output.
mod colors {
    pub const ERROR: &str = "\x1b[1;31m"; // Bold red
    pub const NOTE: &str = "\x1b[1;36m"; // Bold cyan
    pub const HELP: &str = "\x1b[1;32m"; // Bold green
    pub const BOLD: &str = "\x1b[1m";
    pub const BLUE: &str = "\x1b[1;34m"; // Bold blue
    pub const RESET: &str = "\x1b[0m";
}

/// Check if stderr is a terminal (for color output).
fn use_colors() -> bool {
    std::io::stderr().is_terminal()
}

/// Get the source line containing the given byte offset.
pub(super) fn get_source_line(source: &str, offset: u32) -> Option<(&str, usize)> {
    let offset = offset as usize;
    if offset > source.len() {
        return None;
    }

    // Find line start
    let line_start = source[..offset].rfind('\n').map_or(0, |pos| pos + 1);

    // Find line end
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |pos| offset + pos);

    let line = &source[line_start..line_end];
    Some((line, line_start))
}

/// Generate a suggestion for common formatting errors.
pub(super) fn get_suggestion(error: &ParseError) -> Option<String> {
    let msg = error.message();
    let ctx = error.context().unwrap_or("");

    // Suggestions based on error code
    match error.code() {
        ErrorCode::E1003 => {
            // Unclosed delimiter
            Some("check for missing closing bracket, parenthesis, or brace".to_string())
        }
        ErrorCode::E1001 => {
            // Unexpected token - check for common mistakes in message and context
            if msg.contains("expected )") || msg.contains("expected `)") || ctx.contains(")") {
                Some("check for missing closing parenthesis or comma".to_string())
            } else if msg.contains("expected }") || msg.contains("expected `}") || ctx.contains("}")
            {
                Some("check for missing closing brace".to_string())
            } else if msg.contains("expected ]") || msg.contains("expected `]") || ctx.contains("]")
            {
                Some("check for missing closing bracket".to_string())
            } else if msg.contains("expected =")
                || msg.contains("expected `=`")
                || ctx.contains("=")
            {
                Some("function definitions require `=` before the body".to_string())
            } else if msg.contains("expected ,") || msg.contains("expected `,") || ctx.contains(",")
            {
                Some("check for missing comma or colon between items".to_string())
            } else if msg.contains("expected :")
                || msg.contains("expected `:`")
                || ctx.contains(":")
            {
                Some("parameter types use: name: Type".to_string())
            } else {
                None
            }
        }
        ErrorCode::E1002 => {
            // Expected expression
            Some("an expression is required here".to_string())
        }
        ErrorCode::E1004 => {
            // Expected identifier
            Some("identifiers must start with a letter or underscore".to_string())
        }
        ErrorCode::E1005 => {
            // Expected type
            Some("type annotations use: name: Type".to_string())
        }
        ErrorCode::E1006 => {
            // Invalid function definition
            Some("function definitions use: @name (params) -> ReturnType = body".to_string())
        }
        ErrorCode::E1007 => {
            // Missing function body
            Some("add `= expression` after the function signature".to_string())
        }
        ErrorCode::E1011 => {
            // Multi-arg function call requires named arguments
            Some("use named arguments: func(arg1: val1, arg2: val2)".to_string())
        }
        ErrorCode::E0001 => {
            // Unterminated string
            Some("add a closing `\"` to terminate the string".to_string())
        }
        ErrorCode::E0004 => {
            // Unterminated character literal
            Some("character literals use single quotes: 'a'".to_string())
        }
        ErrorCode::E0005 => {
            // Invalid escape sequence
            Some("valid escape sequences: \\n, \\t, \\r, \\\\, \\\", \\'".to_string())
        }
        _ => {
            // Check error context for additional hints
            if ctx.contains("attribute") {
                return Some("attributes use: #name(args)".to_string());
            }
            // Check message for patterns
            if msg.contains("expected identifier") {
                return Some("identifiers must start with a letter or underscore".to_string());
            }
            if msg.contains("expected type") {
                return Some("type annotations use: name: Type".to_string());
            }
            None
        }
    }
}

/// Format a parse error with source code snippet.
pub(super) fn format_parse_error(path: &str, error: &ParseError, source: &str) -> String {
    let use_color = use_colors();
    let mut output = String::new();

    // Get line/column information
    let span = error.span();
    let (line, col) = span_utils::offset_to_line_col(source, span.start);
    let end_col = if span.end > span.start {
        let (_, ec) = span_utils::offset_to_line_col(source, span.end);
        ec
    } else {
        col + 1
    };

    // Error header
    if use_color {
        let _ = writeln!(
            output,
            "{}error{}{}{}: {}",
            colors::ERROR,
            colors::RESET,
            colors::BOLD,
            format!("[{}]", error.code()),
            error.message()
        );
    } else {
        let _ = writeln!(output, "error[{}]: {}", error.code(), error.message());
    }

    // File location
    if use_color {
        let _ = writeln!(
            output,
            "  {}-->{} {}:{}:{}",
            colors::BLUE,
            colors::RESET,
            path,
            line,
            col
        );
    } else {
        let _ = writeln!(output, "  --> {}:{}:{}", path, line, col);
    }

    // Source line with line number
    if let Some((source_line, _)) = get_source_line(source, span.start) {
        let line_num_str = line.to_string();
        let padding = " ".repeat(line_num_str.len());

        // Empty line before source
        if use_color {
            let _ = writeln!(output, "  {} {}|{}", padding, colors::BLUE, colors::RESET);
        } else {
            let _ = writeln!(output, "  {} |", padding);
        }

        // Source line
        if use_color {
            let _ = writeln!(
                output,
                "  {}{} |{} {}",
                colors::BLUE,
                line_num_str,
                colors::RESET,
                source_line
            );
        } else {
            let _ = writeln!(output, "  {} | {}", line_num_str, source_line);
        }

        // Underline
        let underline_start = col.saturating_sub(1) as usize;
        let underline_len = (end_col.saturating_sub(col) as usize).max(1);
        let underline_padding = " ".repeat(underline_start);
        let underline = "^".repeat(underline_len);

        if use_color {
            let _ = writeln!(
                output,
                "  {} {}|{} {}{}{}{}",
                padding,
                colors::BLUE,
                colors::RESET,
                underline_padding,
                colors::ERROR,
                underline,
                colors::RESET
            );
        } else {
            let _ = writeln!(output, "  {} | {}{}", padding, underline_padding, underline);
        }
    }

    // Suggestion
    if let Some(suggestion) = get_suggestion(error) {
        if use_color {
            let _ = writeln!(
                output,
                "  = {}help{}: {}",
                colors::HELP,
                colors::RESET,
                suggestion
            );
        } else {
            let _ = writeln!(output, "  = help: {}", suggestion);
        }
    }

    // Context (if available)
    if let Some(ctx) = error.context() {
        if use_color {
            let _ = writeln!(output, "  = {}note{}: {}", colors::NOTE, colors::RESET, ctx);
        } else {
            let _ = writeln!(output, "  = note: {}", ctx);
        }
    }

    output
}

/// Format multiple parse errors with source code snippets.
pub(super) fn format_parse_errors(path: &str, errors: &[ParseError], source: &str) -> String {
    let use_color = use_colors();
    let mut output = String::new();

    for error in errors {
        output.push_str(&format_parse_error(path, error, source));
        output.push('\n');
    }

    // Add a summary note for multiple errors or a hint about formatting
    if errors.len() == 1 {
        if use_color {
            let _ = writeln!(
                output,
                "{}note{}: fix the syntax error to enable formatting",
                colors::NOTE,
                colors::RESET
            );
        } else {
            let _ = writeln!(output, "note: fix the syntax error to enable formatting");
        }
    } else {
        if use_color {
            let _ = writeln!(
                output,
                "{}note{}: fix {} syntax errors to enable formatting",
                colors::NOTE,
                colors::RESET,
                errors.len()
            );
        } else {
            let _ = writeln!(
                output,
                "note: fix {} syntax errors to enable formatting",
                errors.len()
            );
        }
    }

    output
}
