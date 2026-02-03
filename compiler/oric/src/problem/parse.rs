//! Parse-time problem definitions.
//!
//! These problems occur during parsing when the source code doesn't match
//! the expected grammar.

use super::impl_has_span;
use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::ir::Span;
use crate::suggest::suggest_similar;
use ori_ir::StringInterner;

/// Problems that occur during parsing.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParseProblem {
    /// Unexpected token encountered.
    UnexpectedToken {
        span: Span,
        expected: String,
        found: String,
    },

    /// Expected an expression, found something else.
    ExpectedExpression { span: Span, found: String },

    /// Unclosed delimiter (parenthesis, bracket, brace).
    UnclosedDelimiter {
        open_span: Span,
        expected_close: char,
        found_span: Span,
    },

    /// Expected an identifier.
    ExpectedIdentifier { span: Span, found: String },

    /// Expected a type annotation.
    ExpectedType { span: Span, found: String },

    /// Invalid function definition.
    InvalidFunctionDef { span: Span, reason: String },

    /// Missing function body.
    MissingFunctionBody { span: Span, name: String },

    /// Invalid pattern syntax.
    InvalidPatternSyntax {
        span: Span,
        pattern_name: String,
        reason: String,
    },

    /// Missing required pattern argument.
    MissingPatternArg {
        span: Span,
        pattern_name: String,
        arg_name: String,
    },

    /// Unknown pattern argument.
    UnknownPatternArg {
        span: Span,
        pattern_name: String,
        arg_name: String,
        valid_args: Vec<String>,
    },

    /// Multi-arg function call requires named arguments.
    RequiresNamedArgs {
        span: Span,
        func_name: String,
        arg_count: usize,
    },

    /// Invalid `function_seq` syntax.
    InvalidFunctionSeq {
        span: Span,
        seq_name: String,
        reason: String,
    },

    /// `function_exp` requires named properties.
    RequiresNamedProps { span: Span, exp_name: String },

    /// Reserved built-in function name used for user function.
    ReservedBuiltinName { span: Span, name: String },

    /// Unterminated string literal.
    UnterminatedString { span: Span },

    /// Invalid character in source.
    InvalidCharacter { span: Span, char: char },

    /// Invalid number literal.
    InvalidNumber { span: Span, reason: String },

    /// Unterminated character literal.
    UnterminatedChar { span: Span },

    /// Invalid escape sequence.
    InvalidEscape { span: Span, escape: String },
}

// Generate HasSpan implementation using macro.
// Most variants use `span`, but UnclosedDelimiter uses `found_span`.
impl_has_span! {
    ParseProblem {
        found_span: [UnclosedDelimiter],
        span: [
            UnexpectedToken,
            ExpectedExpression,
            ExpectedIdentifier,
            ExpectedType,
            InvalidFunctionDef,
            MissingFunctionBody,
            InvalidPatternSyntax,
            MissingPatternArg,
            UnknownPatternArg,
            RequiresNamedArgs,
            InvalidFunctionSeq,
            RequiresNamedProps,
            ReservedBuiltinName,
            UnterminatedString,
            InvalidCharacter,
            InvalidNumber,
            UnterminatedChar,
            InvalidEscape,
        ],
    }
}

impl ParseProblem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        <Self as super::HasSpan>::span(self)
    }

    /// Convert this problem into a diagnostic.
    ///
    /// The interner parameter is reserved for future Name field lookups.
    #[expect(
        unused_variables,
        reason = "interner reserved for future Name field conversions"
    )]
    pub fn into_diagnostic(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            ParseProblem::UnexpectedToken {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E1001)
                .with_message(format!(
                    "unexpected token: expected {expected}, found `{found}`"
                ))
                .with_label(*span, format!("expected {expected}")),

            ParseProblem::ExpectedExpression { span, found } => Diagnostic::error(ErrorCode::E1002)
                .with_message(format!("expected expression, found `{found}`"))
                .with_label(*span, "expected expression here"),

            ParseProblem::UnclosedDelimiter {
                open_span,
                expected_close,
                found_span,
            } => {
                let opener = match expected_close {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '?',
                };
                Diagnostic::error(ErrorCode::E1003)
                    .with_message(format!("unclosed delimiter `{opener}`"))
                    .with_label(*found_span, format!("expected `{expected_close}`"))
                    .with_secondary_label(*open_span, "unclosed delimiter opened here")
            }

            ParseProblem::ExpectedIdentifier { span, found } => Diagnostic::error(ErrorCode::E1004)
                .with_message(format!("expected identifier, found `{found}`"))
                .with_label(*span, "expected identifier"),

            ParseProblem::ExpectedType { span, found } => Diagnostic::error(ErrorCode::E1005)
                .with_message(format!("expected type, found `{found}`"))
                .with_label(*span, "expected type annotation"),

            ParseProblem::InvalidFunctionDef { span, reason } => {
                Diagnostic::error(ErrorCode::E1006)
                    .with_message(format!("invalid function definition: {reason}"))
                    .with_label(*span, reason.clone())
            }

            ParseProblem::MissingFunctionBody { span, name } => Diagnostic::error(ErrorCode::E1007)
                .with_message(format!("function `@{name}` is missing its body"))
                .with_label(*span, "expected `=` followed by function body")
                .with_suggestion(format!(
                    "add function body after `=`: @{name} (...) -> Type = <expression>"
                )),

            ParseProblem::InvalidPatternSyntax {
                span,
                pattern_name,
                reason,
            } => Diagnostic::error(ErrorCode::E1008)
                .with_message(format!(
                    "invalid syntax in `{pattern_name}` pattern: {reason}"
                ))
                .with_label(*span, reason.clone()),

            ParseProblem::MissingPatternArg {
                span,
                pattern_name,
                arg_name,
            } => Diagnostic::error(ErrorCode::E1009)
                .with_message(format!(
                    "missing required argument `.{arg_name}:` in `{pattern_name}` pattern"
                ))
                .with_label(*span, format!("missing `.{arg_name}:`"))
                .with_suggestion(format!(
                    "add `.{arg_name}: <value>` to the pattern arguments"
                )),

            ParseProblem::UnknownPatternArg {
                span,
                pattern_name,
                arg_name,
                valid_args,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E1010)
                    .with_message(format!(
                        "unknown argument `.{arg_name}:` in `{pattern_name}` pattern"
                    ))
                    .with_label(*span, "unknown argument");
                // Try to find a similar argument name
                if let Some(suggestion) =
                    suggest_similar(arg_name, valid_args.iter().map(String::as_str))
                {
                    diag = diag.with_suggestion(format!("try using `.{suggestion}:`"));
                } else {
                    let valid_list = valid_args.join("`, `.");
                    diag = diag.with_note(format!("valid arguments are: `.{valid_list}`"));
                }
                diag
            }

            ParseProblem::RequiresNamedArgs {
                span,
                func_name,
                arg_count,
            } => Diagnostic::error(ErrorCode::E1011)
                .with_message(format!(
                    "function `{func_name}` with {arg_count} arguments requires named arguments"
                ))
                .with_label(*span, "use named arguments")
                .with_suggestion("use arg: value syntax for each argument"),

            ParseProblem::InvalidFunctionSeq {
                span,
                seq_name,
                reason,
            } => Diagnostic::error(ErrorCode::E1012)
                .with_message(format!("invalid `{seq_name}` expression: {reason}"))
                .with_label(*span, reason.clone()),

            ParseProblem::RequiresNamedProps { span, exp_name } => {
                Diagnostic::error(ErrorCode::E1013)
                    .with_message(format!(
                        "`{exp_name}` requires named properties (`name: value`)"
                    ))
                    .with_label(*span, "use named properties")
                    .with_suggestion("use named properties for all arguments (e.g., `name: value`)")
            }

            ParseProblem::ReservedBuiltinName { span, name } => Diagnostic::error(ErrorCode::E1014)
                .with_message(format!("`{name}` is a reserved built-in function name"))
                .with_label(*span, "cannot use this name for user-defined functions")
                .with_note("built-in names are reserved in call position"),

            ParseProblem::UnterminatedString { span } => Diagnostic::error(ErrorCode::E0001)
                .with_message("unterminated string literal")
                .with_label(*span, "string not closed")
                .with_suggestion("add closing `\"`"),

            ParseProblem::InvalidCharacter { span, char } => Diagnostic::error(ErrorCode::E0002)
                .with_message(format!("invalid character `{char}`"))
                .with_label(*span, "unexpected character"),

            ParseProblem::InvalidNumber { span, reason } => Diagnostic::error(ErrorCode::E0003)
                .with_message(format!("invalid number literal: {reason}"))
                .with_label(*span, reason.clone()),

            ParseProblem::UnterminatedChar { span } => Diagnostic::error(ErrorCode::E0004)
                .with_message("unterminated character literal")
                .with_label(*span, "character literal not closed")
                .with_suggestion("add closing `'`"),

            ParseProblem::InvalidEscape { span, escape } => Diagnostic::error(ErrorCode::E0005)
                .with_message(format!("invalid escape sequence `{escape}`"))
                .with_label(*span, "unknown escape")
                .with_note("valid escapes are: \\n, \\t, \\r, \\\", \\\\, \\'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unexpected_token() {
        let problem = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        assert_eq!(problem.span(), Span::new(0, 5));
    }

    #[test]
    fn test_unclosed_delimiter() {
        let problem = ParseProblem::UnclosedDelimiter {
            open_span: Span::new(0, 1),
            expected_close: ')',
            found_span: Span::new(10, 10),
        };

        // Primary span is where we expected the close
        assert_eq!(problem.span(), Span::new(10, 10));
    }

    #[test]
    fn test_missing_pattern_arg() {
        let problem = ParseProblem::MissingPatternArg {
            span: Span::new(0, 10),
            pattern_name: "map".into(),
            arg_name: "over".into(),
        };

        assert_eq!(problem.span(), Span::new(0, 10));
    }

    #[test]
    fn test_unknown_pattern_arg() {
        let problem = ParseProblem::UnknownPatternArg {
            span: Span::new(5, 10),
            pattern_name: "map".into(),
            arg_name: "foo".into(),
            valid_args: vec!["over".into(), "transform".into()],
        };

        assert_eq!(problem.span(), Span::new(5, 10));
    }

    #[test]
    fn test_problem_equality() {
        let p1 = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let p2 = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let p3 = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "statement".into(),
            found: "}".into(),
        };

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_problem_hash() {
        use std::collections::HashSet;

        let p1 = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let p2 = p1.clone();
        let p3 = ParseProblem::ExpectedExpression {
            span: Span::new(0, 5),
            found: "}".into(),
        };

        let mut set = HashSet::new();
        set.insert(p1);
        set.insert(p2); // duplicate
        set.insert(p3);

        assert_eq!(set.len(), 2);
    }
}
