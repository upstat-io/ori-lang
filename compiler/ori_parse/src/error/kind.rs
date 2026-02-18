//! Structured parse error kinds with contextual data.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, TokenKind};

use super::details::{CodeSuggestion, ExtraLabel, ParseErrorDetails};
use super::mistakes::{closing_delimiter, delimiter_name};

/// Structured parse error kinds with contextual data.
///
/// Each variant captures the specific information needed to generate
/// helpful error messages and suggestions. Inspired by Gleam's 50+
/// error variants and Roc's nested error context.
#[derive(Clone, Debug)]
pub enum ParseErrorKind {
    // === Token-level errors ===
    /// Expected a specific token, found something else.
    UnexpectedToken {
        /// The token that was found.
        found: TokenKind,
        /// Description of what was expected.
        expected: &'static str,
        /// Parsing context for better messages.
        context: Option<&'static str>,
    },

    /// Unexpected end of file.
    UnexpectedEof {
        /// Description of what was expected.
        expected: &'static str,
        /// If EOF was reached while looking for a closing delimiter.
        unclosed: Option<(TokenKind, Span)>,
    },

    // === Expression errors ===
    /// Expected an expression but found something else.
    ExpectedExpression {
        /// The token that was found.
        found: TokenKind,
        /// Position in the expression (primary, operand, etc.).
        position: ExprPosition,
    },

    /// Operator without right-hand operand.
    TrailingOperator {
        /// The dangling operator.
        operator: TokenKind,
    },

    // === Declaration errors ===
    /// Expected a declaration (function, type, etc.).
    ExpectedDeclaration {
        /// The token that was found.
        found: TokenKind,
    },

    /// Expected an identifier.
    ExpectedIdentifier {
        /// The token that was found.
        found: TokenKind,
        /// Context: function name, parameter, variable, etc.
        context: IdentContext,
    },

    /// Invalid function clause.
    InvalidFunctionClause {
        /// Why the clause is invalid.
        reason: &'static str,
    },

    // === Pattern errors ===
    /// Invalid pattern syntax.
    InvalidPattern {
        /// The token that was found.
        found: TokenKind,
        /// Pattern context: match, let, function param.
        context: PatternContext,
    },

    /// Pattern argument issues.
    PatternArgumentError {
        /// The pattern name (e.g., "recurse", "cache").
        pattern_name: &'static str,
        /// What's wrong.
        reason: PatternArgError,
    },

    // === Type errors (parsing) ===
    /// Expected a type annotation.
    ExpectedType {
        /// The token that was found.
        found: TokenKind,
    },

    // === Delimiter errors ===
    /// Unclosed delimiter.
    UnclosedDelimiter {
        /// The opening delimiter.
        open: TokenKind,
        /// Where it was opened.
        open_span: Span,
        /// The expected closing delimiter.
        expected_close: TokenKind,
    },

    // === Attribute errors ===
    /// Invalid attribute syntax.
    InvalidAttribute {
        /// What's wrong with the attribute.
        reason: &'static str,
    },

    // === Keyword errors ===
    /// Unsupported or misplaced keyword.
    UnsupportedKeyword {
        /// The keyword that was found.
        keyword: TokenKind,
        /// Why it's not allowed here.
        reason: &'static str,
    },
}

/// Position in an expression where an error occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExprPosition {
    /// Start of a primary expression (literal, identifier, etc.).
    Primary,
    /// After an operator, expecting operand.
    Operand,
    /// In a function call argument.
    CallArgument,
    /// In a list literal element.
    ListElement,
    /// In a map literal entry.
    MapEntry,
    /// In a match arm pattern.
    MatchArm,
    /// In a conditional (if/then/else).
    Conditional,
}

/// Context for identifier expectation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentContext {
    /// Function name (after @).
    FunctionName,
    /// Type name.
    TypeName,
    /// Variable name.
    VariableName,
    /// Parameter name.
    ParameterName,
    /// Field name.
    FieldName,
    /// Named argument.
    NamedArgument,
    /// Generic type parameter.
    GenericParam,
    /// Trait name.
    TraitName,
    /// Capability name.
    CapabilityName,
}

/// Context for pattern parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternContext {
    /// Match expression arm.
    Match,
    /// Let binding.
    Let,
    /// Function parameter.
    FunctionParam,
    /// For loop binding.
    ForLoop,
}

/// What's wrong with a pattern argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternArgError {
    /// Required argument is missing.
    Missing { name: &'static str },
    /// Unknown argument provided.
    Unknown { name: String },
    /// Argument has wrong type/format.
    Invalid {
        name: &'static str,
        reason: &'static str,
    },
    /// Duplicate argument.
    Duplicate { name: String },
}

impl ParseErrorKind {
    /// Get a short title for this error (e.g., "UNEXPECTED TOKEN").
    ///
    /// Used as the headline in error reports.
    pub fn title(&self) -> &'static str {
        match self {
            Self::UnexpectedToken { .. } => "UNEXPECTED TOKEN",
            Self::UnexpectedEof { .. } => "UNEXPECTED END OF FILE",
            Self::ExpectedExpression { .. } => "EXPECTED EXPRESSION",
            Self::TrailingOperator { .. } => "INCOMPLETE EXPRESSION",
            Self::ExpectedDeclaration { .. } => "EXPECTED DECLARATION",
            Self::ExpectedIdentifier { .. } => "EXPECTED IDENTIFIER",
            Self::InvalidFunctionClause { .. } => "INVALID FUNCTION",
            Self::InvalidPattern { .. } => "INVALID PATTERN",
            Self::PatternArgumentError { .. } => "PATTERN ERROR",
            Self::ExpectedType { .. } => "EXPECTED TYPE",
            Self::UnclosedDelimiter { .. } => "UNCLOSED DELIMITER",
            Self::InvalidAttribute { .. } => "INVALID ATTRIBUTE",
            Self::UnsupportedKeyword { .. } => "UNSUPPORTED KEYWORD",
        }
    }

    /// Get an empathetic, conversational explanation of this error.
    ///
    /// Uses first-person phrasing ("I found...", "I was expecting...")
    /// inspired by Elm's error messages.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ParseErrorKind → user message dispatch"
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
            } => Some("Ori doesn't use semicolons. Remove the `;` \u{2014} expressions flow naturally to the next line."),

            // === Return keyword ===
            Self::UnexpectedToken {
                found: TokenKind::Return,
                ..
            }
            | Self::UnsupportedKeyword {
                keyword: TokenKind::Return,
                ..
            } => Some("Ori has no `return` keyword. The last expression in a block is automatically its value."),

            // === Mutability ===
            Self::UnexpectedToken {
                found: TokenKind::Mut,
                ..
            }
            | Self::UnsupportedKeyword {
                keyword: TokenKind::Mut,
                ..
            } => Some("In Ori, variables are mutable by default. Use `$name` (dollar prefix) to create an immutable binding."),

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

    /// Get the error code for this kind.
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::UnexpectedToken { .. } | Self::UnexpectedEof { .. } => ErrorCode::E1001,
            Self::ExpectedExpression { .. }
            | Self::TrailingOperator { .. }
            | Self::ExpectedDeclaration { .. } => ErrorCode::E1002,
            Self::UnclosedDelimiter { .. } => ErrorCode::E1003,
            Self::ExpectedIdentifier { .. } => ErrorCode::E1004,
            Self::ExpectedType { .. } => ErrorCode::E1005,
            Self::InvalidFunctionClause { .. } | Self::InvalidAttribute { .. } => ErrorCode::E1006,
            Self::InvalidPattern { .. } => ErrorCode::E1008,
            Self::PatternArgumentError { .. } => ErrorCode::E1009,
            Self::UnsupportedKeyword { .. } => ErrorCode::E1015,
        }
    }

    /// Generate the primary error message.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ParseErrorKind message dispatch"
    )]
    pub fn message(&self) -> String {
        match self {
            Self::UnexpectedToken {
                found,
                expected,
                context,
            } => {
                let ctx = context.map(|c| format!(" in {c}")).unwrap_or_default();
                format!("expected {expected}, found `{}`{ctx}", found.display_name())
            }
            Self::UnexpectedEof { expected, unclosed } => {
                if let Some((delim, _)) = unclosed {
                    format!(
                        "unexpected end of file while looking for `{}`",
                        closing_delimiter(delim).display_name()
                    )
                } else {
                    format!("unexpected end of file, expected {expected}")
                }
            }
            Self::ExpectedExpression { found, position } => {
                let pos = match position {
                    ExprPosition::Primary => "",
                    ExprPosition::Operand => " after operator",
                    ExprPosition::CallArgument => " in function call",
                    ExprPosition::ListElement => " in list",
                    ExprPosition::MapEntry => " in map",
                    ExprPosition::MatchArm => " in match arm",
                    ExprPosition::Conditional => " in conditional",
                };
                format!("expected expression{pos}, found `{}`", found.display_name())
            }
            Self::TrailingOperator { operator } => {
                format!(
                    "operator `{}` requires a right-hand operand",
                    operator.display_name()
                )
            }
            Self::ExpectedDeclaration { found } => {
                format!(
                    "expected declaration (function, type, trait, or import), found `{}`",
                    found.display_name()
                )
            }
            Self::ExpectedIdentifier { found, context } => {
                let ctx = match context {
                    IdentContext::FunctionName => "function name",
                    IdentContext::TypeName => "type name",
                    IdentContext::VariableName => "variable name",
                    IdentContext::ParameterName => "parameter name",
                    IdentContext::FieldName => "field name",
                    IdentContext::NamedArgument => "argument name",
                    IdentContext::GenericParam => "generic type parameter",
                    IdentContext::TraitName => "trait name",
                    IdentContext::CapabilityName => "capability name",
                };
                format!("expected {ctx}, found `{}`", found.display_name())
            }
            Self::InvalidFunctionClause { reason } => {
                format!("invalid function clause: {reason}")
            }
            Self::InvalidPattern { found, context } => {
                let ctx = match context {
                    PatternContext::Match => "match expression",
                    PatternContext::Let => "let binding",
                    PatternContext::FunctionParam => "function parameter",
                    PatternContext::ForLoop => "for loop",
                };
                format!("invalid pattern in {ctx}: found `{}`", found.display_name())
            }
            Self::PatternArgumentError {
                pattern_name,
                reason,
            } => match reason {
                PatternArgError::Missing { name } => {
                    format!("{pattern_name} requires `{name}:` argument")
                }
                PatternArgError::Unknown { name } => {
                    format!("{pattern_name} has no argument named `{name}`")
                }
                PatternArgError::Invalid { name, reason } => {
                    format!("{pattern_name} argument `{name}`: {reason}")
                }
                PatternArgError::Duplicate { name } => {
                    format!("{pattern_name} argument `{name}` specified multiple times")
                }
            },
            Self::ExpectedType { found } => {
                format!("expected type, found `{}`", found.display_name())
            }
            Self::UnclosedDelimiter {
                open,
                expected_close,
                ..
            } => {
                format!(
                    "unclosed `{}`; expected `{}`",
                    open.display_name(),
                    expected_close.display_name()
                )
            }
            Self::InvalidAttribute { reason } => {
                format!("invalid attribute: {reason}")
            }
            Self::UnsupportedKeyword { keyword, reason } => {
                format!("`{}` is not supported: {reason}", keyword.display_name())
            }
        }
    }

    /// Generate comprehensive error details for rich rendering.
    ///
    /// This method produces a complete [`ParseErrorDetails`] struct containing
    /// all information needed for Elm-quality error messages: title, empathetic
    /// explanation, code labels, hints, and auto-fix suggestions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let kind = ParseErrorKind::UnclosedDelimiter {
    ///     open: TokenKind::LParen,
    ///     open_span: Span::new(5, 1),
    ///     expected_close: TokenKind::RParen,
    /// };
    /// let details = kind.details(error_span);
    /// println!("{}", details.text); // "I found an unclosed `(`..."
    /// ```
    #[allow(
        dead_code,
        reason = "infrastructure for ParseErrorKind rich diagnostic migration"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ParseErrorKind → diagnostic dispatch"
    )]
    pub(crate) fn details(&self, error_span: Span) -> ParseErrorDetails {
        match self {
            Self::UnexpectedToken {
                found,
                expected,
                context,
            } => {
                let ctx_phrase = context
                    .map(|c| format!(" while parsing {c}"))
                    .unwrap_or_default();

                let text = format!(
                    "I ran into something unexpected{ctx_phrase}.\n\n\
                     I was expecting {expected}, but I found `{}`.",
                    found.display_name()
                );

                let label_text = format!("found `{}`, expected {expected}", found.display_name());

                let mut details =
                    ParseErrorDetails::new("UNEXPECTED TOKEN", text, label_text, self.error_code());

                // Add hint from hint() method
                if let Some(hint) = self.hint() {
                    details = details.with_hint(hint);
                }

                details
            }

            Self::UnexpectedEof { expected, unclosed } => {
                let (text, label_text, extra) = if let Some((delim, open_span)) = unclosed {
                    let close = closing_delimiter(delim);
                    (
                        format!(
                            "I reached the end of the file while looking for a closing `{}`.\n\n\
                             It looks like you may have forgotten to close this {}.",
                            close.display_name(),
                            delimiter_name(delim)
                        ),
                        format!("expected `{}` before end of file", close.display_name()),
                        Some(ExtraLabel::same_file(
                            *open_span,
                            format!("the `{}` was opened here", delim.display_name()),
                        )),
                    )
                } else {
                    (
                        format!(
                            "I reached the end of the file unexpectedly.\n\n\
                             I was expecting {expected} here."
                        ),
                        format!("expected {expected}"),
                        None,
                    )
                };

                let mut details = ParseErrorDetails::new(
                    "UNEXPECTED END OF FILE",
                    text,
                    label_text,
                    self.error_code(),
                );

                if let Some(label) = extra {
                    details = details.with_extra_label(label);
                }

                details
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

                let text = format!(
                    "I was expecting an expression {where_phrase}, but I found `{}`.\n\n\
                     Expressions include things like: numbers, strings, function calls, \
                     variables, and operators.",
                    found.display_name()
                );

                let label_text = format!("expected expression, found `{}`", found.display_name());

                let mut details = ParseErrorDetails::new(
                    "EXPECTED EXPRESSION",
                    text,
                    label_text,
                    self.error_code(),
                );

                if let Some(hint) = self.hint() {
                    details = details.with_hint(hint);
                }
                if let Some(note) = self.educational_note() {
                    // Add educational note as secondary hint
                    if details.hint.is_none() {
                        details = details.with_hint(note);
                    }
                }

                details
            }

            Self::TrailingOperator { operator } => {
                let text = format!(
                    "I found an operator `{}` without a right-hand side.\n\n\
                     Every operator needs something on both sides, like `a {} b`.",
                    operator.display_name(),
                    operator.display_name()
                );

                let label_text = format!(
                    "operator `{}` needs a right operand",
                    operator.display_name()
                );

                let mut details = ParseErrorDetails::new(
                    "INCOMPLETE EXPRESSION",
                    text,
                    label_text,
                    self.error_code(),
                );

                if let Some(hint) = self.hint() {
                    details = details.with_hint(hint);
                }

                details
            }

            Self::ExpectedDeclaration { found } => {
                let text = format!(
                    "I was expecting a declaration here, but I found `{}`.\n\n\
                     At the top level of a file, I expect things like:\n\
                     \u{2022} Functions: `@add (a: int, b: int) -> int = a + b`\n\
                     \u{2022} Types: `type Point = {{ x: int, y: int }}`\n\
                     \u{2022} Imports: `use std::math`",
                    found.display_name()
                );

                let label_text = format!("expected declaration, found `{}`", found.display_name());

                let mut details = ParseErrorDetails::new(
                    "EXPECTED DECLARATION",
                    text,
                    label_text,
                    self.error_code(),
                );

                if let Some(note) = self.educational_note() {
                    details = details.with_hint(note);
                }

                details
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

                let text = format!(
                    "I was expecting {what} here, but I found `{}`.\n\n\
                     Names must start with a letter or underscore, followed by \
                     letters, numbers, or underscores.",
                    found.display_name()
                );

                let label_text = format!("expected identifier, found `{}`", found.display_name());

                ParseErrorDetails::new("EXPECTED IDENTIFIER", text, label_text, self.error_code())
            }

            Self::InvalidFunctionClause { reason } => {
                let text = format!("There's a problem with this function definition.\n\n{reason}");

                ParseErrorDetails::new(
                    "INVALID FUNCTION",
                    text,
                    "invalid function clause",
                    self.error_code(),
                )
            }

            Self::InvalidPattern { found, context } => {
                let where_str = match context {
                    PatternContext::Match => "in this match expression",
                    PatternContext::Let => "in this let binding",
                    PatternContext::FunctionParam => "in this function parameter",
                    PatternContext::ForLoop => "in this for loop",
                };

                let text = format!(
                    "I found an invalid pattern {where_str}.\n\n\
                     I was expecting a pattern like `x`, `(a, b)`, or `Some(value)`, \
                     but I found `{}`.",
                    found.display_name()
                );

                let label_text = format!("invalid pattern: `{}`", found.display_name());

                let mut details =
                    ParseErrorDetails::new("INVALID PATTERN", text, label_text, self.error_code());

                if let Some(note) = self.educational_note() {
                    details = details.with_hint(note);
                }

                details
            }

            Self::PatternArgumentError {
                pattern_name,
                reason,
            } => {
                let (text, label_text) = match reason {
                    PatternArgError::Missing { name } => (
                        format!(
                            "The `{pattern_name}` pattern requires a `{name}:` argument.\n\n\
                             Try adding `{name}: <value>` to the pattern arguments."
                        ),
                        format!("missing required argument `{name}`"),
                    ),
                    PatternArgError::Unknown { name } => (
                        format!(
                            "The `{pattern_name}` pattern doesn't have an argument called `{name}`.\n\n\
                             Check the documentation for valid arguments."
                        ),
                        format!("unknown argument `{name}`"),
                    ),
                    PatternArgError::Invalid { name, reason: r } => (
                        format!(
                            "The `{name}:` argument in this `{pattern_name}` pattern is invalid.\n\n{r}"
                        ),
                        format!("invalid argument `{name}`"),
                    ),
                    PatternArgError::Duplicate { name } => (
                        format!(
                            "The `{name}:` argument is specified more than once.\n\n\
                             Each argument should only appear once in a pattern."
                        ),
                        format!("duplicate argument `{name}`"),
                    ),
                };

                ParseErrorDetails::new("PATTERN ERROR", text, label_text, self.error_code())
            }

            Self::ExpectedType { found } => {
                let text = format!(
                    "I was expecting a type here, but I found `{}`.\n\n\
                     Types include: `int`, `str`, `bool`, `[T]` (list), and custom types.",
                    found.display_name()
                );

                let label_text = format!("expected type, found `{}`", found.display_name());

                ParseErrorDetails::new("EXPECTED TYPE", text, label_text, self.error_code())
            }

            Self::UnclosedDelimiter {
                open,
                open_span,
                expected_close,
            } => {
                let text = format!(
                    "I found an unclosed `{}`.\n\n\
                     Every `{}` needs a matching `{}` to close it.",
                    open.display_name(),
                    open.display_name(),
                    expected_close.display_name()
                );

                let label_text = format!("expected `{}` here", expected_close.display_name());

                let mut details = ParseErrorDetails::new(
                    "UNCLOSED DELIMITER",
                    text,
                    label_text,
                    self.error_code(),
                );

                // Add extra label showing where the delimiter was opened
                details = details.with_extra_label(ExtraLabel::same_file(
                    *open_span,
                    format!("the `{}` was opened here", open.display_name()),
                ));

                // Add suggestion to insert closing delimiter
                details = details.with_suggestion(CodeSuggestion::machine_applicable(
                    error_span,
                    expected_close.display_name(),
                    format!(
                        "Add `{}` to close the {}",
                        expected_close.display_name(),
                        delimiter_name(open)
                    ),
                ));

                if let Some(note) = self.educational_note() {
                    details = details.with_hint(note);
                }

                details
            }

            Self::InvalidAttribute { reason } => {
                let text = format!("There's a problem with this attribute.\n\n{reason}");

                ParseErrorDetails::new(
                    "INVALID ATTRIBUTE",
                    text,
                    "invalid attribute",
                    self.error_code(),
                )
            }

            Self::UnsupportedKeyword { keyword, reason } => {
                let text = format!(
                    "The keyword `{}` isn't supported here.\n\n{reason}",
                    keyword.display_name()
                );

                let label_text = format!("`{}` not supported", keyword.display_name());

                let mut details = ParseErrorDetails::new(
                    "UNSUPPORTED KEYWORD",
                    text,
                    label_text,
                    self.error_code(),
                );

                if let Some(hint) = self.hint() {
                    details = details.with_hint(hint);
                }

                details
            }
        }
    }
}
