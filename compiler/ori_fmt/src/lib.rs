//! Ori Formatter
//!
//! Code formatter for the Ori programming language.
//!
//! # Architecture
//!
//! The formatter uses a two-pass, width-based breaking algorithm:
//!
//! 1. **Measure Pass**: Bottom-up traversal calculating inline width of each node
//! 2. **Render Pass**: Top-down rendering deciding inline vs broken based on width
//!
//! Core principle: render inline if it fits (<=100 chars), break otherwise.
//!
//! # Modules
//!
//! - [`width`]: Width calculation for AST nodes
//! - [`emitter`]: Output abstraction for string and file output
//! - [`context`]: Formatting context with indentation and column tracking
//! - [`formatter`]: Core formatting engine

pub mod context;
pub mod declarations;
pub mod emitter;
pub mod formatter;
pub mod width;

pub use context::{FormatContext, INDENT_WIDTH, MAX_LINE_WIDTH};
pub use declarations::{format_module, ModuleFormatter};
pub use emitter::{Emitter, FileEmitter, StringEmitter};
pub use formatter::{format_expr, Formatter};
pub use width::{WidthCalculator, ALWAYS_STACKED};

/// Convert tabs to spaces in source text.
///
/// Each tab character is replaced with spaces to reach the next multiple of 4 columns.
/// This is a preprocessing step for source text normalization.
///
/// # Example
///
/// ```
/// use ori_fmt::tabs_to_spaces;
///
/// let source = "\t@foo () = 42";
/// let normalized = tabs_to_spaces(source);
/// assert_eq!(normalized, "    @foo () = 42");
/// ```
pub fn tabs_to_spaces(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut column = 0;

    for c in source.chars() {
        match c {
            '\t' => {
                // Calculate spaces needed to reach next multiple of INDENT_WIDTH
                let spaces = INDENT_WIDTH - (column % INDENT_WIDTH);
                for _ in 0..spaces {
                    result.push(' ');
                }
                column += spaces;
            }
            '\n' => {
                result.push('\n');
                column = 0;
            }
            '\r' => {
                result.push('\r');
                // Don't reset column for \r alone (handle \r\n case)
            }
            _ => {
                result.push(c);
                column += 1;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_to_spaces_single_tab_at_start() {
        assert_eq!(tabs_to_spaces("\t@foo"), "    @foo");
    }

    #[test]
    fn tabs_to_spaces_tab_after_content() {
        // Tab at column 2 should go to column 4
        assert_eq!(tabs_to_spaces("ab\tc"), "ab  c");
    }

    #[test]
    fn tabs_to_spaces_tab_at_column_4() {
        // Tab at column 4 should go to column 8
        assert_eq!(tabs_to_spaces("abcd\te"), "abcd    e");
    }

    #[test]
    fn tabs_to_spaces_multiple_tabs() {
        assert_eq!(tabs_to_spaces("\t\tfoo"), "        foo");
    }

    #[test]
    fn tabs_to_spaces_mixed_content() {
        let input = "fn main\n\treturn 0\n";
        let expected = "fn main\n    return 0\n";
        assert_eq!(tabs_to_spaces(input), expected);
    }

    #[test]
    fn tabs_to_spaces_no_tabs() {
        let input = "    @foo () = 42";
        assert_eq!(tabs_to_spaces(input), input);
    }

    #[test]
    fn tabs_to_spaces_empty_string() {
        assert_eq!(tabs_to_spaces(""), "");
    }

    #[test]
    fn tabs_to_spaces_only_newlines() {
        assert_eq!(tabs_to_spaces("\n\n\n"), "\n\n\n");
    }

    #[test]
    fn tabs_to_spaces_tab_in_middle_of_line() {
        // "x" at col 0, tab at col 1 -> spaces to col 4
        assert_eq!(tabs_to_spaces("x\ty"), "x   y");
    }

    #[test]
    fn tabs_to_spaces_newline_resets_column() {
        // After newline, column resets, so tab goes to col 4
        assert_eq!(tabs_to_spaces("abc\n\tdef"), "abc\n    def");
    }
}
