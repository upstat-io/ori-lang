//! Empathetic messages, hints, and educational notes for parse errors.

use ori_ir::TokenKind;

use super::{ExprPosition, ParseErrorKind, PatternContext};

impl ParseErrorKind {
    /// Get a contextual hint for this error, if applicable.
    ///
    /// Hints provide guidance for common mistakes, especially for users
    /// coming from other programming languages.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            // Semicolons
            Self::UnexpectedToken {
                found: TokenKind::Semicolon,
                ..
            } => Some("Semicolons separate statements inside block expressions `{ ... }` and terminate top-level items."),

            // Return keyword
            Self::UnexpectedToken {
                found: TokenKind::Return,
                ..
            }
            | Self::UnsupportedKeyword {
                keyword: TokenKind::Return,
                ..
            } => Some("Ori has no `return` keyword. The last expression in a block is automatically its value."),

            // Trailing operators
            Self::TrailingOperator {
                operator: TokenKind::Plus,
                ..
            } => Some("The `+` operator needs a value on both sides, like `a + b`."),
            Self::TrailingOperator {
                operator: TokenKind::Minus,
                ..
            } => Some("The `-` operator needs a value on both sides, like `a - b`. For negation, use `-x` at the start."),
            Self::TrailingOperator {
                operator: TokenKind::Star,
                ..
            } => Some("The `*` operator needs a value on both sides, like `a * b`."),
            Self::TrailingOperator {
                operator: TokenKind::Slash,
                ..
            } => Some("The `/` operator needs a value on both sides, like `a / b`."),
            Self::TrailingOperator { .. } => Some("Binary operators need values on both sides."),

            // Empty blocks
            Self::ExpectedExpression {
                found: TokenKind::RBrace,
                ..
            } => Some("Blocks must end with an expression. Try adding `void` if no value is needed."),

            // For loop
            Self::ExpectedExpression {
                found: TokenKind::For,
                position: ExprPosition::Primary,
            } => Some("For loops in Ori use `for item in collection { ... }` syntax."),

            // Common type keywords in wrong positions
            Self::UnexpectedToken {
                found: TokenKind::Void,
                context: Some("expression"),
                ..
            } => Some("`void` is a type, not a value. Use it in type annotations: `-> void`."),

            _ => None,
        }
    }

    /// Get educational context for this error's parsing position.
    ///
    /// Returns language-learning notes to help users understand Ori's
    /// design philosophy and syntax patterns.
    pub fn educational_note(&self) -> Option<&'static str> {
        match self {
            Self::ExpectedExpression { position, .. } => match position {
                ExprPosition::Conditional => Some(
                    "In Ori, `if` is an expression that returns a value. \
                     Both branches must have the same type, and neither can be empty.",
                ),
                ExprPosition::MatchArm => Some(
                    "Match arms must return values. Ori's match is an expression, \
                     not a statement, so every arm needs a result.",
                ),
                ExprPosition::CallArgument => Some(
                    "Function arguments must be expressions. Named arguments use \
                     `name: value` syntax.",
                ),
                _ => None,
            },

            Self::InvalidPattern { context, .. } => match context {
                PatternContext::Match => Some(
                    "Match patterns include: literals (`42`), bindings (`x`), \
                     wildcards (`_`), variants (`Some(x)`), and ranges (`1..10`).",
                ),
                PatternContext::Let => Some(
                    "Let bindings support destructuring: `let {x, y} = point` or \
                     `let [first, ..rest] = list`.",
                ),
                PatternContext::FunctionParam => Some(
                    "Function parameters can use patterns for destructuring: \
                     `@process ({x, y}: Point) -> int`.",
                ),
                PatternContext::ForLoop => {
                    Some("For loops can destructure: `for {key, value} in map { ... }`.")
                }
            },

            Self::ExpectedDeclaration { .. } => Some(
                "Top-level declarations in Ori: functions (`@name`), types (`type`), \
                 traits (`trait`), and imports (`use`).",
            ),

            Self::UnclosedDelimiter { open, .. } => match open {
                TokenKind::LBrace => Some(
                    "Braces `{ }` in Ori define blocks (for control flow) and \
                     record literals (for data). Every `{` needs a matching `}`.",
                ),
                TokenKind::LBracket => Some(
                    "Brackets `[ ]` define list literals and list patterns. \
                     Every `[` needs a matching `]`.",
                ),
                TokenKind::LParen => Some(
                    "Parentheses `( )` are used for function calls, grouping, \
                     and tuple patterns. Every `(` needs a matching `)`.",
                ),
                _ => None,
            },

            _ => None,
        }
    }
}
