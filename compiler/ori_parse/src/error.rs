//! Parse error types.
//!
//! Provides structured error types for the parser with:
//! - Rich error variants capturing context
//! - Contextual hints for common mistakes
//! - Related location tracking for better diagnostics

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, TokenKind};

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
    /// Get a contextual hint for this error, if applicable.
    ///
    /// Hints provide guidance for common mistakes.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::UnexpectedToken {
                found: TokenKind::Semicolon,
                ..
            } => Some("Ori doesn't use semicolons to terminate statements"),
            Self::UnexpectedToken {
                found: TokenKind::Return,
                ..
            } => Some("Ori has no `return` keyword; the last expression in a block is its value"),
            Self::TrailingOperator {
                operator: TokenKind::Plus,
                ..
            } => Some("Add another operand after `+`"),
            Self::TrailingOperator {
                operator: TokenKind::Minus,
                ..
            } => Some("Add another operand after `-`"),
            Self::ExpectedExpression {
                found: TokenKind::RBrace,
                ..
            } => Some("Blocks must end with an expression, not be empty"),
            Self::UnsupportedKeyword {
                keyword: TokenKind::Return,
                ..
            } => Some("Use the last expression as the block's value instead"),
            Self::UnsupportedKeyword {
                keyword: TokenKind::Mut,
                ..
            } => Some("Variables are mutable by default in Ori; use `let $x` for immutable"),
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
}

/// Get the closing delimiter for an opening delimiter.
fn closing_delimiter(open: &TokenKind) -> TokenKind {
    match open {
        TokenKind::LParen => TokenKind::RParen,
        TokenKind::LBracket => TokenKind::RBracket,
        TokenKind::LBrace => TokenKind::RBrace,
        TokenKind::Lt => TokenKind::Gt,
        _ => TokenKind::Eof, // fallback
    }
}

/// A related location for richer error context.
///
/// Used to point to related code, like where a delimiter was opened.
/// This will be used for multi-span diagnostics in a future enhancement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Infrastructure for multi-span diagnostics
pub struct Note {
    /// The message explaining this location.
    pub message: String,
    /// The related source location, if any.
    pub span: Option<Span>,
}

/// Parse error with error code for rich diagnostics.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseError {
    /// Error code for searchability.
    pub code: ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Location of the error.
    pub span: Span,
    /// Optional context for suggestions.
    pub context: Option<String>,
    /// Optional help messages.
    pub help: Vec<String>,
}

impl ParseError {
    /// Create a new parse error.
    #[cold]
    pub fn new(code: ori_diagnostic::ErrorCode, message: impl Into<String>, span: Span) -> Self {
        ParseError {
            code,
            message: message.into(),
            span,
            context: None,
            help: Vec::new(),
        }
    }

    // --- Series Combinator Helpers ---

    /// Error when expecting an item in a series but none was found.
    #[cold]
    pub fn expected_item(span: Span, terminator: &TokenKind) -> Self {
        ParseError::new(
            ErrorCode::E1002,
            format!("expected item before `{}`", terminator.display_name()),
            span,
        )
    }

    /// Error when a trailing separator was found but not allowed.
    #[cold]
    pub fn unexpected_trailing_separator(span: Span, separator: &TokenKind) -> Self {
        ParseError::new(
            ErrorCode::E1001,
            format!("unexpected trailing `{}`", separator.display_name()),
            span,
        )
    }

    /// Error when expecting separator or terminator but found something else.
    #[cold]
    pub fn expected_separator_or_terminator(
        span: Span,
        separator: &TokenKind,
        terminator: &TokenKind,
    ) -> Self {
        ParseError::new(
            ErrorCode::E1001,
            format!(
                "expected `{}` or `{}`",
                separator.display_name(),
                terminator.display_name()
            ),
            span,
        )
    }

    /// Error when a series has too few items.
    #[cold]
    pub fn too_few_items(span: Span, min: usize, actual: usize) -> Self {
        ParseError::new(
            ErrorCode::E1002,
            format!("expected at least {min} items, found {actual}"),
            span,
        )
    }

    /// Error when a series has too many items.
    #[cold]
    pub fn too_many_items(span: Span, max: usize, actual: usize) -> Self {
        ParseError::new(
            ErrorCode::E1002,
            format!("expected at most {max} items, found {actual}"),
            span,
        )
    }

    /// Add context for better error messages.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Add a help message.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }

    /// Convert to a full Diagnostic for rich error reporting.
    pub fn to_diagnostic(&self) -> ori_diagnostic::Diagnostic {
        let mut diag = ori_diagnostic::Diagnostic::error(self.code)
            .with_message(&self.message)
            .with_label(self.span, self.context.as_deref().unwrap_or("here"));

        for help in &self.help {
            diag = diag.with_note(help);
        }

        diag
    }

    /// Create a [`ParseError`] from a structured [`ParseErrorKind`].
    ///
    /// This is the preferred way to create errors in new code.
    /// The kind provides all context needed to generate helpful messages.
    #[cold]
    pub fn from_kind(kind: &ParseErrorKind, span: Span) -> Self {
        let code = kind.error_code();
        let message = kind.message();
        let hint = kind.hint();

        let mut error = ParseError {
            code,
            message,
            span,
            context: None,
            help: Vec::new(),
        };

        if let Some(hint) = hint {
            error.help.push(hint.to_string());
        }

        // Add related location context for unclosed delimiters
        if let ParseErrorKind::UnclosedDelimiter {
            open_span, open, ..
        } = &kind
        {
            error.context = Some(format!("`{}` opened here", open.display_name()));
            // Note: the open_span would be used in a multi-span diagnostic
            let _ = open_span; // Mark as intentionally unused for now
        }

        error
    }
}

impl From<(ParseErrorKind, Span)> for ParseError {
    fn from((kind, span): (ParseErrorKind, Span)) -> Self {
        ParseError::from_kind(&kind, span)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_unexpected_token_message() {
        let kind = ParseErrorKind::UnexpectedToken {
            found: TokenKind::Semicolon,
            expected: "expression",
            context: Some("function body"),
        };
        assert_eq!(
            kind.message(),
            "expected expression, found `;` in function body"
        );
        assert!(kind.hint().is_some());
    }

    #[test]
    fn test_expected_expression_message() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::RParen,
            position: ExprPosition::CallArgument,
        };
        assert_eq!(
            kind.message(),
            "expected expression in function call, found `)`"
        );
    }

    #[test]
    fn test_pattern_error_message() {
        let kind = ParseErrorKind::PatternArgumentError {
            pattern_name: "cache",
            reason: PatternArgError::Missing { name: "key" },
        };
        assert_eq!(kind.message(), "cache requires `key:` argument");
    }

    #[test]
    fn test_unsupported_keyword_hint() {
        let kind = ParseErrorKind::UnsupportedKeyword {
            keyword: TokenKind::Return,
            reason: "Ori is expression-based",
        };
        assert!(kind.hint().is_some());
        assert!(kind.hint().unwrap().contains("last expression"));
    }

    #[test]
    fn test_error_code_mapping() {
        assert_eq!(
            ParseErrorKind::UnexpectedToken {
                found: TokenKind::Plus,
                expected: "identifier",
                context: None
            }
            .error_code(),
            ErrorCode::E1001
        );
        assert_eq!(
            ParseErrorKind::ExpectedExpression {
                found: TokenKind::Eof,
                position: ExprPosition::Primary
            }
            .error_code(),
            ErrorCode::E1002
        );
        assert_eq!(
            ParseErrorKind::ExpectedIdentifier {
                found: TokenKind::Plus,
                context: IdentContext::FunctionName
            }
            .error_code(),
            ErrorCode::E1004
        );
    }

    #[test]
    fn test_from_kind() {
        let kind = ParseErrorKind::UnexpectedToken {
            found: TokenKind::Semicolon,
            expected: "expression",
            context: None,
        };
        let error = ParseError::from_kind(&kind, Span::new(0, 1));

        assert_eq!(error.code, ErrorCode::E1001);
        assert!(error.message.contains("expected expression"));
        assert!(!error.help.is_empty()); // Has hint about semicolons
    }
}
