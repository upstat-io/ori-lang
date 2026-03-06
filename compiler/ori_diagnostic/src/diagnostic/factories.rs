//! Factory functions for common diagnostic patterns.
//!
//! These create pre-configured [`Diagnostic`] instances for frequently
//! encountered error patterns: type mismatches, unexpected tokens,
//! unclosed delimiters, etc.

use ori_ir::Span;

use super::Diagnostic;
use crate::ErrorCode;

/// Configuration for a type mismatch diagnostic.
///
/// Used by `type_mismatch` to create a diagnostic with all relevant context.
/// This config struct pattern improves API clarity for functions with 4+ parameters.
#[derive(Clone, Debug)]
pub struct TypeMismatchConfig<'a> {
    /// The source location of the mismatch.
    pub span: Span,
    /// The expected type name.
    pub expected: &'a str,
    /// The found type name.
    pub found: &'a str,
    /// Context describing where the mismatch occurred (e.g., "return value").
    pub context: &'a str,
}

impl<'a> TypeMismatchConfig<'a> {
    /// Create a new type mismatch configuration.
    pub fn new(span: Span, expected: &'a str, found: &'a str, context: &'a str) -> Self {
        TypeMismatchConfig {
            span,
            expected,
            found,
            context,
        }
    }

    /// Convert this configuration into a diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::error(ErrorCode::E2001)
            .with_message(format!(
                "type mismatch: expected `{}`, found `{}`",
                self.expected, self.found
            ))
            .with_label(self.span, self.context)
    }
}

/// Create a "type mismatch" diagnostic.
///
/// For more explicit parameter naming, use `TypeMismatchConfig::new(...).into_diagnostic()`.
pub fn type_mismatch(span: Span, expected: &str, found: &str, context: &str) -> Diagnostic {
    TypeMismatchConfig::new(span, expected, found, context).into_diagnostic()
}

/// Create an "unexpected token" diagnostic.
pub fn unexpected_token(span: Span, expected: &str, found: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1001)
        .with_message(format!(
            "unexpected token: expected {expected}, found `{found}`"
        ))
        .with_label(span, format!("expected {expected}"))
}

/// Create an "expected expression" diagnostic.
pub fn expected_expression(span: Span, found: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1002)
        .with_message(format!("expected expression, found `{found}`"))
        .with_label(span, "expected expression here")
}

/// Create an "unclosed delimiter" diagnostic.
pub fn unclosed_delimiter(open_span: Span, close_span: Span, delimiter: char) -> Diagnostic {
    let expected = match delimiter {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => delimiter,
    };
    Diagnostic::error(ErrorCode::E1003)
        .with_message(format!("unclosed delimiter `{delimiter}`"))
        .with_label(close_span, format!("expected `{expected}`"))
        .with_secondary_label(open_span, "unclosed delimiter opened here")
}

/// Create an "unknown identifier" diagnostic.
pub fn unknown_identifier(span: Span, name: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E2003)
        .with_message(format!("unknown identifier `{name}`"))
        .with_label(span, "not found in this scope")
}

/// Create a "missing pattern argument" diagnostic.
pub fn missing_pattern_arg(span: Span, pattern: &str, arg: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1009)
        .with_message(format!(
            "missing required argument `.{arg}:` in `{pattern}` pattern"
        ))
        .with_label(span, format!("missing `.{arg}:`"))
        .with_suggestion(format!("add `.{arg}: <value>` to the pattern arguments"))
}

/// Configuration for an unknown pattern argument diagnostic.
///
/// Used by `unknown_pattern_arg` to create a diagnostic with all relevant context.
/// This config struct pattern improves API clarity for functions with 4+ parameters.
#[derive(Clone, Debug)]
pub struct UnknownPatternArgConfig<'a> {
    /// The source location of the unknown argument.
    pub span: Span,
    /// The pattern name (e.g., "map", "filter").
    pub pattern: &'a str,
    /// The unknown argument name.
    pub arg: &'a str,
    /// The list of valid argument names.
    pub valid: &'a [&'a str],
}

impl<'a> UnknownPatternArgConfig<'a> {
    /// Create a new unknown pattern argument configuration.
    pub fn new(span: Span, pattern: &'a str, arg: &'a str, valid: &'a [&'a str]) -> Self {
        UnknownPatternArgConfig {
            span,
            pattern,
            arg,
            valid,
        }
    }

    /// Convert this configuration into a diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        let valid_list = self.valid.join("`, `.");
        Diagnostic::error(ErrorCode::E1010)
            .with_message(format!(
                "unknown argument `.{}:` in `{}` pattern",
                self.arg, self.pattern
            ))
            .with_label(self.span, "unknown argument")
            .with_note(format!("valid arguments are: `.{valid_list}`"))
    }
}

/// Create an "unknown pattern argument" diagnostic.
///
/// For more explicit parameter naming, use `UnknownPatternArgConfig::new(...).into_diagnostic()`.
pub fn unknown_pattern_arg(span: Span, pattern: &str, arg: &str, valid: &[&str]) -> Diagnostic {
    UnknownPatternArgConfig::new(span, pattern, arg, valid).into_diagnostic()
}
