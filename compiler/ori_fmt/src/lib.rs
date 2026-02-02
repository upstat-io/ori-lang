//! Ori Formatter
//!
//! Code formatter for the Ori programming language.
//!
//! # Quick Start
//!
//! ```ignore
//! use ori_fmt::{format_module, FormatConfig};
//!
//! let formatted = format_module(&module, &arena, &interner);
//! ```
//!
//! # API Stability
//!
//! ## Stable API (safe to use in production)
//!
//! - [`format_module`], [`format_module_with_comments`], [`format_module_with_config`]
//! - [`format_expr`], [`Formatter`]
//! - [`format_incremental`], [`apply_regions`], [`IncrementalResult`]
//! - [`FormatConfig`], [`TrailingCommas`]
//! - [`tabs_to_spaces`]
//!
//! ## Advanced API (subject to change)
//!
//! These modules are public for extensibility and debugging but may change
//! between minor versions:
//!
//! - [`spacing`]: Token spacing rules (Layer 1)
//! - [`packing`]: Container packing decisions (Layer 2)
//! - [`shape`]: Width tracking (Layer 3)
//! - [`rules`]: Breaking rules (Layer 4)
//! - [`width`]: Width calculation
//!
//! # Architecture
//!
//! The formatter uses a 5-layer architecture:
//!
//! 1. **Layer 1 (Spacing)**: Declarative O(1) token spacing rules
//! 2. **Layer 2 (Packing)**: Container packing decisions (fit/break)
//! 3. **Layer 3 (Shape)**: Width tracking through recursion
//! 4. **Layer 4 (Breaking)**: Ori-specific breaking rules
//! 5. **Layer 5 (Orchestration)**: Main formatter coordinating all layers
//!
//! The core algorithm is two-pass, width-based breaking:
//!
//! 1. **Measure Pass**: Bottom-up traversal calculating inline width of each node
//! 2. **Render Pass**: Top-down rendering deciding inline vs broken based on width
//!
//! Core principle: render inline if it fits (<=100 chars), break otherwise.

pub mod comments;
pub mod context;
pub mod declarations;
pub mod emitter;
pub mod formatter;
pub mod incremental;
pub mod packing;
pub mod rules;
pub mod shape;
pub mod spacing;
pub mod width;

pub use comments::{format_comment, CommentIndex};
pub use context::{FormatConfig, FormatContext, TrailingCommas, INDENT_WIDTH, MAX_LINE_WIDTH};
pub use declarations::{
    format_module, format_module_with_comments, format_module_with_comments_and_config,
    format_module_with_config, ModuleFormatter,
};
pub use emitter::{Emitter, StringEmitter};
pub use formatter::{format_expr, Formatter};
pub use incremental::{apply_regions, format_incremental, FormattedRegion, IncrementalResult};
pub use packing::{
    all_items_simple, determine_packing, is_simple_item, list_construct_kind, separator_for,
    ConstructKind, Packing, Separator,
};
pub use rules::{
    needs_parens, BooleanBreakRule, BreakPoint, ChainedElseIfRule, ElseIfBranch, ForChain,
    ForLevel, IfChain, LoopRule, MethodChainRule, NestedForRule, ParenPosition, ParenthesesRule,
    RunContext, RunRule, ShortBodyRule,
};
pub use shape::Shape;
pub use spacing::{lookup_spacing, SpaceAction, TokenCategory, TokenMatcher, SPACE_RULES};
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
