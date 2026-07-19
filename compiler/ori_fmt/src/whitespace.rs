//! Whitespace Normalization
//!
//! Standalone text-pass helpers for whitespace normalization, independent of
//! the AST-based formatter.

use crate::context::INDENT_WIDTH;

/// Convert tabs to spaces in arbitrary text.
///
/// Each tab character is replaced with spaces to reach the next multiple of 4 columns.
///
/// This is a naive text pass with NO awareness of string/template/char literals
/// or comments. It MUST NOT be applied to Ori source before formatting: a tab
/// inside a literal is content, and expanding it corrupts the value. The
/// formatter re-generates all indentation itself, so source tabs need no
/// pre-normalization.
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
