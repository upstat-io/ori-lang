//! Parse-time problem definitions.
//!
//! These problems occur during parsing when the source code doesn't match
//! the expected grammar.

use crate::ir::Span;

/// Problems that occur during parsing.
///
/// # Salsa Compatibility
/// Has Clone, Eq, PartialEq, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParseProblem {
    /// Unexpected token encountered.
    UnexpectedToken {
        span: Span,
        expected: String,
        found: String,
    },

    /// Expected an expression, found something else.
    ExpectedExpression {
        span: Span,
        found: String,
    },

    /// Unclosed delimiter (parenthesis, bracket, brace).
    UnclosedDelimiter {
        open_span: Span,
        expected_close: char,
        found_span: Span,
    },

    /// Expected an identifier.
    ExpectedIdentifier {
        span: Span,
        found: String,
    },

    /// Expected a type annotation.
    ExpectedType {
        span: Span,
        found: String,
    },

    /// Invalid function definition.
    InvalidFunctionDef {
        span: Span,
        reason: String,
    },

    /// Missing function body.
    MissingFunctionBody {
        span: Span,
        name: String,
    },

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

    /// Invalid function_seq syntax.
    InvalidFunctionSeq {
        span: Span,
        seq_name: String,
        reason: String,
    },

    /// function_exp requires named properties.
    RequiresNamedProps {
        span: Span,
        exp_name: String,
    },

    /// Reserved built-in function name used for user function.
    ReservedBuiltinName {
        span: Span,
        name: String,
    },

    /// Unterminated string literal.
    UnterminatedString {
        span: Span,
    },

    /// Invalid character in source.
    InvalidCharacter {
        span: Span,
        char: char,
    },

    /// Invalid number literal.
    InvalidNumber {
        span: Span,
        reason: String,
    },

    /// Unterminated character literal.
    UnterminatedChar {
        span: Span,
    },

    /// Invalid escape sequence.
    InvalidEscape {
        span: Span,
        escape: String,
    },
}

impl ParseProblem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        match self {
            ParseProblem::UnexpectedToken { span, .. } => *span,
            ParseProblem::ExpectedExpression { span, .. } => *span,
            ParseProblem::UnclosedDelimiter { found_span, .. } => *found_span,
            ParseProblem::ExpectedIdentifier { span, .. } => *span,
            ParseProblem::ExpectedType { span, .. } => *span,
            ParseProblem::InvalidFunctionDef { span, .. } => *span,
            ParseProblem::MissingFunctionBody { span, .. } => *span,
            ParseProblem::InvalidPatternSyntax { span, .. } => *span,
            ParseProblem::MissingPatternArg { span, .. } => *span,
            ParseProblem::UnknownPatternArg { span, .. } => *span,
            ParseProblem::RequiresNamedArgs { span, .. } => *span,
            ParseProblem::InvalidFunctionSeq { span, .. } => *span,
            ParseProblem::RequiresNamedProps { span, .. } => *span,
            ParseProblem::ReservedBuiltinName { span, .. } => *span,
            ParseProblem::UnterminatedString { span } => *span,
            ParseProblem::InvalidCharacter { span, .. } => *span,
            ParseProblem::InvalidNumber { span, .. } => *span,
            ParseProblem::UnterminatedChar { span } => *span,
            ParseProblem::InvalidEscape { span, .. } => *span,
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
