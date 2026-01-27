//! Parse problem rendering.
//!
//! Renders ParseProblem variants into user-facing Diagnostic messages.

use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::problem::ParseProblem;
use super::Render;

impl Render for ParseProblem {
    fn render(&self) -> Diagnostic {
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

            ParseProblem::ExpectedExpression { span, found } => {
                Diagnostic::error(ErrorCode::E1002)
                    .with_message(format!("expected expression, found `{found}`"))
                    .with_label(*span, "expected expression here")
            }

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

            ParseProblem::ExpectedIdentifier { span, found } => {
                Diagnostic::error(ErrorCode::E1004)
                    .with_message(format!("expected identifier, found `{found}`"))
                    .with_label(*span, "expected identifier")
            }

            ParseProblem::ExpectedType { span, found } => Diagnostic::error(ErrorCode::E1005)
                .with_message(format!("expected type, found `{found}`"))
                .with_label(*span, "expected type annotation"),

            ParseProblem::InvalidFunctionDef { span, reason } => {
                Diagnostic::error(ErrorCode::E1006)
                    .with_message(format!("invalid function definition: {reason}"))
                    .with_label(*span, reason.clone())
            }

            ParseProblem::MissingFunctionBody { span, name } => {
                Diagnostic::error(ErrorCode::E1007)
                    .with_message(format!("function `@{name}` is missing its body"))
                    .with_label(*span, "expected `=` followed by function body")
                    .with_suggestion("add a body: @{name} (...) -> Type = expression")
            }

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
                let valid_list = valid_args.join("`, `.");
                Diagnostic::error(ErrorCode::E1010)
                    .with_message(format!(
                        "unknown argument `.{arg_name}:` in `{pattern_name}` pattern"
                    ))
                    .with_label(*span, "unknown argument")
                    .with_note(format!("valid arguments are: `.{valid_list}`"))
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
                    .with_suggestion(format!(
                        "example: {exp_name}(over: items, transform: fn)"
                    ))
            }

            ParseProblem::ReservedBuiltinName { span, name } => {
                Diagnostic::error(ErrorCode::E1014)
                    .with_message(format!("`{name}` is a reserved built-in function name"))
                    .with_label(*span, "cannot use this name for user-defined functions")
                    .with_note("built-in names are reserved in call position")
            }

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
