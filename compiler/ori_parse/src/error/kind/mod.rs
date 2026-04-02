//! Structured parse error kinds with contextual data.

mod classification;
mod details;
mod messages;

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
}
