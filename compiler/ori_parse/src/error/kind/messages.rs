//! Empathetic messages, hints, and educational notes for parse errors.

use ori_ir::TokenKind;

use super::{ExprPosition, IdentContext, ParseErrorKind, PatternArgError, PatternContext};
use crate::error::mistakes::{closing_delimiter, delimiter_name};

impl ParseErrorKind {
    /// Get an empathetic, conversational explanation of this error.
    ///
    /// Uses first-person phrasing ("I found...", "I was expecting...")
    /// inspired by Elm's error messages.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ParseErrorKind -> user message dispatch"
    )]
    pub fn empathetic_message(&self) -> String {
        match self {
            Self::UnexpectedToken {
                found,
                expected,
                context,
            } => {
                let ctx_phrase = context
                    .map(|c| format!(" while parsing {c}"))
                    .unwrap_or_default();
                format!(
                    "I ran into something unexpected{ctx_phrase}.\n\n\
                     I was expecting {expected}, but I found `{}`.",
                    found.display_name()
                )
            }

            Self::UnexpectedEof { expected, unclosed } => {
                if let Some((delim, _)) = unclosed {
                    format!(
                        "I reached the end of the file while looking for a closing `{}`.\n\n\
                         It looks like you may have forgotten to close this {}.",
                        closing_delimiter(delim).display_name(),
                        delimiter_name(delim)
                    )
                } else {
                    format!(
                        "I reached the end of the file unexpectedly.\n\n\
                         I was expecting {expected} here."
                    )
                }
            }

            Self::ExpectedExpression { found, position } => {
                let where_phrase = match position {
                    ExprPosition::Primary => "here",
                    ExprPosition::Operand => "after this operator",
                    ExprPosition::CallArgument => "in this function call",
                    ExprPosition::ListElement => "in this list",
                    ExprPosition::MapEntry => "in this map",
                    ExprPosition::MatchArm => "in this match arm",
                    ExprPosition::Conditional => "in this condition",
                };
                format!(
                    "I was expecting an expression {where_phrase}, but I found `{}`.\n\n\
                     Expressions include things like: numbers, strings, function calls, \
                     variables, and operators.",
                    found.display_name()
                )
            }

            Self::TrailingOperator { operator } => {
                format!(
                    "I found an operator `{}` without a right-hand side.\n\n\
                     Every operator needs something on both sides, like `a {} b`.",
                    operator.display_name(),
                    operator.display_name()
                )
            }

            Self::ExpectedDeclaration { found } => {
                format!(
                    "I was expecting a declaration here, but I found `{}`.\n\n\
                     At the top level of a file, I expect things like:\n\
                     \u{2022} Functions: `@add (a: int, b: int) -> int = a + b`\n\
                     \u{2022} Types: `type Point = {{ x: int, y: int }}`\n\
                     \u{2022} Imports: `use std::math`",
                    found.display_name()
                )
            }

            Self::ExpectedIdentifier { found, context } => {
                let what = match context {
                    IdentContext::FunctionName => "a function name",
                    IdentContext::TypeName => "a type name",
                    IdentContext::VariableName => "a variable name",
                    IdentContext::ParameterName => "a parameter name",
                    IdentContext::FieldName => "a field name",
                    IdentContext::NamedArgument => "an argument name",
                    IdentContext::GenericParam => "a type parameter name",
                    IdentContext::TraitName => "a trait name",
                    IdentContext::CapabilityName => "a capability name",
                };
                format!(
                    "I was expecting {what} here, but I found `{}`.\n\n\
                     Names must start with a letter or underscore, followed by \
                     letters, numbers, or underscores.",
                    found.display_name()
                )
            }

            Self::InvalidFunctionClause { reason } => {
                format!("There's a problem with this function definition.\n\n{reason}")
            }

            Self::InvalidPattern { found, context } => {
                let where_str = match context {
                    PatternContext::Match => "in this match expression",
                    PatternContext::Let => "in this let binding",
                    PatternContext::FunctionParam => "in this function parameter",
                    PatternContext::ForLoop => "in this for loop",
                };
                format!(
                    "I found an invalid pattern {where_str}.\n\n\
                     I was expecting a pattern like `x`, `(a, b)`, or `Some(value)`, \
                     but I found `{}`.",
                    found.display_name()
                )
            }

            Self::PatternArgumentError {
                pattern_name,
                reason,
            } => match reason {
                PatternArgError::Missing { name } => {
                    format!(
                        "The `{pattern_name}` pattern requires a `{name}:` argument.\n\n\
                         Try adding `{name}: <value>` to the pattern arguments."
                    )
                }
                PatternArgError::Unknown { name } => {
                    format!(
                        "The `{pattern_name}` pattern doesn't have an argument called `{name}`.\n\n\
                         Check the documentation for valid arguments."
                    )
                }
                PatternArgError::Invalid { name, reason } => {
                    format!(
                        "The `{name}:` argument in this `{pattern_name}` pattern is invalid.\n\n\
                         {reason}"
                    )
                }
                PatternArgError::Duplicate { name } => {
                    format!(
                        "The `{name}:` argument is specified more than once.\n\n\
                         Each argument should only appear once in a pattern."
                    )
                }
            },

            Self::ExpectedType { found } => {
                format!(
                    "I was expecting a type here, but I found `{}`.\n\n\
                     Types include: `int`, `str`, `bool`, `[T]` (list), and custom types.",
                    found.display_name()
                )
            }

            Self::UnclosedDelimiter {
                open,
                expected_close,
                ..
            } => {
                format!(
                    "I found an unclosed `{}`.\n\n\
                     Every `{}` needs a matching `{}` to close it.",
                    open.display_name(),
                    open.display_name(),
                    expected_close.display_name()
                )
            }

            Self::InvalidAttribute { reason } => {
                format!("There's a problem with this attribute.\n\n{reason}")
            }

            Self::UnsupportedKeyword { keyword, reason } => {
                format!(
                    "The keyword `{}` isn't supported here.\n\n{reason}",
                    keyword.display_name()
                )
            }
        }
    }

    /// Get a contextual hint for this error, if applicable.
    ///
    /// Hints provide guidance for common mistakes, especially for users
    /// coming from other programming languages.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            // === Semicolons ===
            Self::UnexpectedToken {
                found: TokenKind::Semicolon,
                ..
            } => Some("Semicolons separate statements inside block expressions `{ ... }` and terminate top-level items."),

            // === Return keyword ===
            Self::UnexpectedToken {
                found: TokenKind::Return,
                ..
            }
            | Self::UnsupportedKeyword {
                keyword: TokenKind::Return,
                ..
            } => Some("Ori has no `return` keyword. The last expression in a block is automatically its value."),

            // === Trailing operators ===
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

            // === Empty blocks ===
            Self::ExpectedExpression {
                found: TokenKind::RBrace,
                ..
            } => Some("Blocks must end with an expression. Try adding `void` if no value is needed."),

            // === For loop ===
            Self::ExpectedExpression {
                found: TokenKind::For,
                position: ExprPosition::Primary,
            } => Some("For loops in Ori use `for item in collection { ... }` syntax."),

            // === Common type keywords in wrong positions ===
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
