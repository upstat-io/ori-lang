//! Rich diagnostic details generation for parse errors.

use ori_ir::Span;

use super::{ExprPosition, IdentContext, ParseErrorKind, PatternArgError, PatternContext};
use crate::error::details::{CodeSuggestion, ExtraLabel, ParseErrorDetails};
use crate::error::mistakes::{closing_delimiter, delimiter_name};

impl ParseErrorKind {
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
        reason = "exhaustive ParseErrorKind -> diagnostic dispatch"
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
