//! Error codes and primary messages for parse errors.

use ori_diagnostic::ErrorCode;

use super::{IdentContext, ParseErrorKind, PatternArgError, PatternContext};
use crate::error::mistakes::closing_delimiter;

impl ParseErrorKind {
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
                    super::ExprPosition::Primary => "",
                    super::ExprPosition::Operand => " after operator",
                    super::ExprPosition::CallArgument => " in function call",
                    super::ExprPosition::ListElement => " in list",
                    super::ExprPosition::MapEntry => " in map",
                    super::ExprPosition::MatchArm => " in match arm",
                    super::ExprPosition::Conditional => " in conditional",
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
