//! Parse error types.
//!
//! Provides structured error types for the parser with:
//! - Rich error variants capturing context
//! - Contextual hints for common mistakes
//! - Related location tracking for better diagnostics
//! - `ErrorContext` for Elm-style "while parsing X" messages
//!
//! # Error Construction Paths
//!
//! Two construction paths coexist:
//! - **`ParseError::new()`** — 87 call sites; simple (code, message, span) errors.
//! - **`ParseError::from_kind()`** — 8 call sites; rich structured errors via
//!   `ParseErrorKind` with title, empathetic message, hint, and educational note.
//!
//! New error sites should prefer `from_kind()`. Migration of existing `new()` sites
//! to `from_kind()` is a future feature task, not a hygiene issue.

use ori_diagnostic::queue::DiagnosticSeverity;
use ori_diagnostic::{Applicability, Diagnostic, ErrorCode, SourceInfo};
use ori_ir::{Span, TokenKind};

/// Context describing what was being parsed when an error occurred.
///
/// Used for Elm-style error messages like "I ran into a problem while parsing
/// an if expression". This is distinct from `ParseContext` (the bitfield for
/// context-sensitive parsing behavior).
///
/// # Usage
///
/// ```ignore
/// self.in_error_context(ErrorContext::IfExpression, |p| {
///     p.parse_if_expr_inner()
/// })
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorContext {
    // === Top-level ===
    /// Parsing a module (top-level declarations).
    Module,
    /// Parsing a function definition.
    FunctionDef,
    /// Parsing a type definition.
    TypeDef,
    /// Parsing a trait definition.
    TraitDef,
    /// Parsing an impl block.
    ImplBlock,
    /// Parsing a use/import statement.
    UseStatement,
    /// Parsing an extern block.
    ExternBlock,

    // === Expressions ===
    /// Parsing an expression (generic).
    Expression,
    /// Parsing an if expression.
    IfExpression,
    /// Parsing a match expression.
    MatchExpression,
    /// Parsing a for loop.
    ForLoop,
    /// Parsing a while loop.
    WhileLoop,
    /// Parsing a block expression.
    Block,
    /// Parsing a closure/lambda.
    Closure,
    /// Parsing a function call.
    FunctionCall,
    /// Parsing a method call.
    MethodCall,
    /// Parsing a list literal.
    ListLiteral,
    /// Parsing a map literal.
    MapLiteral,
    /// Parsing a struct literal.
    StructLiteral,
    /// Parsing an index expression.
    IndexExpression,
    /// Parsing a binary operation.
    BinaryOp,
    /// Parsing a field access.
    FieldAccess,

    // === Patterns ===
    /// Parsing a pattern (generic).
    Pattern,
    /// Parsing a match arm.
    MatchArm,
    /// Parsing a let binding pattern.
    LetPattern,
    /// Parsing function parameters.
    FunctionParams,

    // === Types ===
    /// Parsing a type annotation.
    TypeAnnotation,
    /// Parsing generic type parameters.
    GenericParams,
    /// Parsing a function signature.
    FunctionSignature,

    // === Other ===
    /// Parsing an attribute.
    Attribute,
    /// Parsing a test definition.
    TestDef,
    /// Parsing a contract (pre/post check).
    Contract,
}

impl ErrorContext {
    /// Get a human-readable description of this context.
    ///
    /// Returns a phrase suitable for "while parsing {description}".
    pub fn description(self) -> &'static str {
        match self {
            // Top-level
            Self::Module => "a module",
            Self::FunctionDef => "a function definition",
            Self::TypeDef => "a type definition",
            Self::TraitDef => "a trait definition",
            Self::ImplBlock => "an impl block",
            Self::UseStatement => "a use statement",
            Self::ExternBlock => "an extern block",

            // Expressions
            Self::Expression => "an expression",
            Self::IfExpression => "an if expression",
            Self::MatchExpression => "a match expression",
            Self::ForLoop => "a for loop",
            Self::WhileLoop => "a while loop",
            Self::Block => "a block",
            Self::Closure => "a closure",
            Self::FunctionCall => "a function call",
            Self::MethodCall => "a method call",
            Self::ListLiteral => "a list literal",
            Self::MapLiteral => "a map literal",
            Self::StructLiteral => "a struct literal",
            Self::IndexExpression => "an index expression",
            Self::BinaryOp => "a binary operation",
            Self::FieldAccess => "a field access",

            // Patterns
            Self::Pattern => "a pattern",
            Self::MatchArm => "a match arm",
            Self::LetPattern => "a let binding",
            Self::FunctionParams => "function parameters",

            // Types
            Self::TypeAnnotation => "a type annotation",
            Self::GenericParams => "generic parameters",
            Self::FunctionSignature => "a function signature",

            // Other
            Self::Attribute => "an attribute",
            Self::TestDef => "a test definition",
            Self::Contract => "a contract",
        }
    }

    /// Get a short label for this context (for error titles).
    ///
    /// Returns a capitalized noun phrase without article.
    pub fn label(self) -> &'static str {
        match self {
            // Top-level
            Self::Module => "module",
            Self::FunctionDef => "function definition",
            Self::TypeDef => "type definition",
            Self::TraitDef => "trait definition",
            Self::ImplBlock => "impl block",
            Self::UseStatement => "use statement",
            Self::ExternBlock => "extern block",

            // Expressions
            Self::Expression => "expression",
            Self::IfExpression => "if expression",
            Self::MatchExpression => "match expression",
            Self::ForLoop => "for loop",
            Self::WhileLoop => "while loop",
            Self::Block => "block",
            Self::Closure => "closure",
            Self::FunctionCall => "function call",
            Self::MethodCall => "method call",
            Self::ListLiteral => "list literal",
            Self::MapLiteral => "map literal",
            Self::StructLiteral => "struct literal",
            Self::IndexExpression => "index expression",
            Self::BinaryOp => "binary operation",
            Self::FieldAccess => "field access",

            // Patterns
            Self::Pattern => "pattern",
            Self::MatchArm => "match arm",
            Self::LetPattern => "let binding",
            Self::FunctionParams => "function parameters",

            // Types
            Self::TypeAnnotation => "type annotation",
            Self::GenericParams => "generic parameters",
            Self::FunctionSignature => "function signature",

            // Other
            Self::Attribute => "attribute",
            Self::TestDef => "test definition",
            Self::Contract => "contract",
        }
    }
}

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
                     • Functions: `@add (a: int, b: int) -> int = a + b`\n\
                     • Types: `type Point = {{ x: int, y: int }}`\n\
                     • Imports: `use std::math`",
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
            } => Some("Ori doesn't use semicolons. Remove the `;` — expressions flow naturally to the next line."),

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
                     • Functions: `@add (a: int, b: int) -> int = a + b`\n\
                     • Types: `type Point = {{ x: int, y: int }}`\n\
                     • Imports: `use std::math`",
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

/// Detect common mistakes from source text.
///
/// This is used when the lexer produces an `Error` token — we look at the
/// actual source text to provide targeted help for patterns from other languages.
///
/// # Arguments
/// * `source_text` - The slice of source that produced the error token
///
/// # Returns
/// A tuple of (short description, detailed help message) if a pattern is recognized
pub fn detect_common_mistake(source_text: &str) -> Option<(&'static str, &'static str)> {
    match source_text {
        // JavaScript/TypeScript triple equals
        "===" => Some((
            "triple equals",
            "Ori uses `==` for equality comparison. There's no `===` because Ori is \
             statically typed — values are always compared with consistent semantics.",
        )),

        // JavaScript/TypeScript strict not-equals
        "!==" => Some((
            "strict not-equals",
            "Ori uses `!=` for inequality. There's no `!==` because Ori's static typing \
             ensures consistent comparison semantics.",
        )),

        // C/Java increment/decrement
        "++" => Some((
            "increment operator",
            "Ori doesn't have `++`. Use `x = x + 1` or a compound assignment pattern.",
        )),
        "--" => Some((
            "decrement operator",
            "Ori doesn't have `--`. Use `x = x - 1` or a compound assignment pattern.",
        )),

        // Pascal/SQL not-equals
        "<>" => Some(("not-equals", "Ori uses `!=` for inequality, not `<>`.")),

        // Assignment operators from other languages
        "+=" | "-=" | "*=" | "/=" | "%=" | "&&=" | "||=" | "??=" => Some((
            "compound assignment",
            "Ori doesn't have compound assignment operators. Use `x = x + y` instead of `x += y`.",
        )),

        // Spread/rest from JavaScript
        "..." if source_text == "..." => Some((
            "spread operator",
            "For rest patterns in lists, use `..rest` (two dots). For struct rest, use `..`.",
        )),

        // These ARE valid in Ori - `??` (nullish coalescing) and `=>` (fat arrow)
        "??" | "=>" => None,

        _ => {
            // Check for common keyword-like identifiers
            check_common_keyword_mistake(source_text)
        }
    }
}

/// Check if a source fragment looks like a common keyword from other languages.
fn check_common_keyword_mistake(text: &str) -> Option<(&'static str, &'static str)> {
    match text {
        // OOP keywords
        "class" => Some((
            "class keyword",
            "Ori doesn't have classes. Use `type` for data structures and `trait` for \
             shared behavior. Ori favors composition over inheritance.",
        )),
        "extends" | "extends " => Some((
            "extends keyword",
            "Ori doesn't have inheritance. Use `trait` for shared behavior and \
             composition for combining types.",
        )),
        "implements" => Some((
            "implements keyword",
            "In Ori, use `impl Trait for Type { ... }` to implement a trait for a type.",
        )),
        "interface" => Some((
            "interface keyword",
            "Ori uses `trait` instead of `interface`. Traits define shared behavior \
             that types can implement.",
        )),
        "abstract" => Some((
            "abstract keyword",
            "Ori doesn't have abstract classes. Use traits for polymorphic behavior.",
        )),
        "virtual" | "override" => Some((
            "virtual/override keyword",
            "Ori doesn't have virtual methods or override. Traits provide polymorphism \
             without inheritance hierarchies.",
        )),

        // Control flow from other languages
        "switch" => Some((
            "switch keyword",
            "Ori uses `match` instead of `switch`. Match expressions are exhaustive \
             and support pattern matching.",
        )),
        "case" => Some((
            "case keyword",
            "In Ori's `match`, use `pattern -> expression` instead of `case:`. \
             Example: `match(x, 1 -> \"one\", _ -> \"other\")`.",
        )),
        "default" => Some((
            "default keyword",
            "In Ori's `match`, use `_` (underscore) as the wildcard/default pattern.",
        )),
        "elif" => Some((
            "elif keyword",
            "Ori uses `else if` (two words), not `elif`.",
        )),
        "elsif" => Some((
            "elsif keyword",
            "Ori uses `else if` (two words), not `elsif`.",
        )),
        "elseif" => Some((
            "elseif keyword",
            "Ori uses `else if` (two words, with space), not `elseif`.",
        )),

        // Function keywords
        "function" | "func" | "fn" => Some((
            "function keyword",
            "Ori functions are declared with `@` prefix: `@add (a: int, b: int) -> int = a + b`.",
        )),
        "lambda" => Some((
            "lambda keyword",
            "Ori uses `|args| body` for anonymous functions: `|x| x * 2`.",
        )),

        // Variable keywords
        "var" | "const" => Some((
            "var/const keyword",
            "Ori uses `let` for variable binding. Variables are mutable by default; \
             use `$name` for immutable bindings.",
        )),
        "final" => Some((
            "final keyword",
            "Ori uses `$name` (dollar prefix) for immutable bindings instead of `final`.",
        )),

        // Module keywords
        "import" | "from" => Some((
            "import keyword",
            "Ori uses `use` for imports: `use std::math` or `use std::io::{read, write}`.",
        )),
        "require" => Some((
            "require keyword",
            "Ori uses `use` for imports, not `require`.",
        )),
        "export" | "module" => Some((
            "export/module keyword",
            "In Ori, items are public by default. Use `::` prefix for private items.",
        )),

        // Exception handling
        "throw" | "throws" | "raise" => Some((
            "throw/raise keyword",
            "Ori uses `Result` types for error handling. Return `Err(value)` instead \
             of throwing. Use `?` to propagate errors.",
        )),
        "except" | "catch" if text != "catch" => Some((
            "except keyword",
            "Ori uses `catch { ... }` pattern for error handling, which wraps the \
             result in a `Result` type.",
        )),
        "finally" => Some((
            "finally keyword",
            "Ori doesn't have `finally`. Use RAII patterns or explicit cleanup.",
        )),

        // Null/None
        "null" | "nil" | "NULL" => Some((
            "null keyword",
            "Ori uses `None` (capital N) for absent values in `Option` types. \
             Use `Some(value)` for present values.",
        )),
        "undefined" => Some((
            "undefined keyword",
            "Ori doesn't have `undefined`. Use `Option` types with `Some`/`None`.",
        )),

        // Boolean literals (case sensitivity)
        "True" | "TRUE" | "False" | "FALSE" => Some((
            "boolean literal",
            "Ori booleans are lowercase: `true` and `false`.",
        )),

        // Note: Valid Ori type keywords (int, float, str, bool, char, byte, void)
        // are handled by the wildcard arm below, returning None.

        // Common type names from other languages
        "string" | "String" => Some((
            "string type",
            "Ori uses `str` (lowercase, three letters) for the string type.",
        )),
        "integer" | "Integer" | "Int" => Some((
            "integer type",
            "Ori uses `int` (lowercase, three letters) for the integer type.",
        )),
        "boolean" | "Boolean" | "Bool" => Some((
            "boolean type",
            "Ori uses `bool` (lowercase, four letters) for the boolean type.",
        )),
        "double" | "Double" | "Float" => Some((
            "float type",
            "Ori uses `float` (lowercase) for floating-point numbers.",
        )),
        "Void" => Some((
            "void type",
            "Ori uses `void` (lowercase) for the unit type.",
        )),

        _ => None,
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

/// Get a human-readable name for a delimiter type.
fn delimiter_name(open: &TokenKind) -> &'static str {
    match open {
        TokenKind::LParen => "parenthesis",
        TokenKind::LBracket => "bracket",
        TokenKind::LBrace => "brace",
        TokenKind::Lt => "angle bracket",
        _ => "delimiter",
    }
}

/// A related location for richer error context.
///
/// Used to point to related code, like where a delimiter was opened.
/// This will be used for multi-span diagnostics in a future enhancement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code, reason = "infrastructure for multi-span diagnostics")]
pub(crate) struct Note {
    /// The message explaining this location.
    pub message: String,
    /// The related source location, if any.
    pub span: Option<Span>,
}

// Applicability and SourceInfo are imported from ori_diagnostic at the top of this file.

/// A secondary label pointing to related code.
///
/// Extra labels provide additional context by highlighting related locations.
/// They're particularly useful for errors like:
/// - "unclosed delimiter" → pointing to where it was opened
/// - "type mismatch" → pointing to the expected type declaration
/// - "duplicate definition" → pointing to the first definition
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
pub(crate) struct ExtraLabel {
    /// The source location to highlight.
    pub span: Span,
    /// Optional source info if this label is in a different file.
    pub src_info: Option<SourceInfo>,
    /// The label text explaining this location.
    pub text: String,
}

#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
impl ExtraLabel {
    /// Create a label in the same file.
    pub fn same_file(span: Span, text: impl Into<String>) -> Self {
        Self {
            span,
            src_info: None,
            text: text.into(),
        }
    }

    /// Create a label in a different file.
    pub fn cross_file(
        span: Span,
        path: impl Into<String>,
        content: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            span,
            src_info: Some(SourceInfo {
                path: path.into(),
                content: content.into(),
            }),
            text: text.into(),
        }
    }
}

/// A code suggestion for auto-fixing an error.
///
/// Suggestions are machine-readable fix instructions that can be:
/// - Applied automatically by formatters/IDEs
/// - Shown to users as "quick fixes"
/// - Used in batch refactoring tools
///
/// # Example
///
/// For the error "use `==` instead of `===`":
/// ```ignore
/// CodeSuggestion {
///     span: Span::new(10, 3), // The "===" location
///     replacement: "==".to_string(),
///     message: "Replace `===` with `==`".to_string(),
///     applicability: Applicability::MachineApplicable,
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
pub(crate) struct CodeSuggestion {
    /// The span to replace (what to remove).
    pub span: Span,
    /// The replacement text (what to insert).
    pub replacement: String,
    /// Human-readable description of the fix.
    pub message: String,
    /// Confidence level for auto-application.
    pub applicability: Applicability,
}

#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
impl CodeSuggestion {
    /// Create a machine-applicable suggestion (safe to auto-apply).
    pub fn machine_applicable(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
            applicability: Applicability::MachineApplicable,
        }
    }

    /// Create a suggestion that may need review.
    pub fn maybe_incorrect(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
            applicability: Applicability::MaybeIncorrect,
        }
    }

    /// Create a suggestion with placeholders (don't auto-apply).
    pub fn with_placeholders(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
            applicability: Applicability::HasPlaceholders,
        }
    }
}

/// Comprehensive error details for rich diagnostics.
///
/// This struct contains all the information needed to render a beautiful,
/// Elm-quality error message. It separates the error's semantic content
/// from its presentation, enabling:
///
/// - CLI rendering with colors and underlines
/// - IDE integration (LSP diagnostics + quick fixes)
/// - Machine-readable error reports
///
/// # Example Output
///
/// ```text
/// -- UNEXPECTED TOKEN -------------------------------- src/main.ori:10:5
///
/// I ran into something unexpected while parsing an if expression:
///
///    9 |     if count > 0 then
///   10 |         "positive"
///   11 |     else
///   12 |         count
///        ^^^^^
///        found identifier, expected expression
///
/// Hint: The else branch needs an expression. Did you mean to add
/// something after `count`?
/// ```
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
pub(crate) struct ParseErrorDetails {
    /// Error title (e.g., "UNEXPECTED TOKEN", "UNCLOSED DELIMITER").
    ///
    /// Displayed prominently at the top of the error message.
    pub title: &'static str,

    /// Main explanation text using empathetic phrasing.
    ///
    /// This is the human-readable description using first-person language
    /// like "I ran into..." or "I was expecting...".
    pub text: String,

    /// Inline label at the primary error location.
    ///
    /// Appears directly below the code snippet, explaining what was found
    /// or expected at this exact position.
    pub label_text: String,

    /// Additional labels for related locations.
    ///
    /// Used to show context like "the `{` was opened here" for unclosed
    /// delimiter errors. May reference other files.
    pub extra_labels: Vec<ExtraLabel>,

    /// Actionable hint for fixing the error.
    ///
    /// Provides concrete guidance on how to resolve the issue.
    pub hint: Option<String>,

    /// Machine-applicable code suggestion for auto-fix.
    ///
    /// When present, IDEs can offer "quick fix" functionality.
    pub suggestion: Option<CodeSuggestion>,

    /// Structured error code (e.g., E1001).
    pub error_code: ErrorCode,
}

#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
impl ParseErrorDetails {
    /// Create new error details with required fields.
    pub fn new(
        title: &'static str,
        text: impl Into<String>,
        label_text: impl Into<String>,
        error_code: ErrorCode,
    ) -> Self {
        Self {
            title,
            text: text.into(),
            label_text: label_text.into(),
            extra_labels: Vec::new(),
            hint: None,
            suggestion: None,
            error_code,
        }
    }

    /// Add an extra label for related context.
    #[must_use]
    pub fn with_extra_label(mut self, label: ExtraLabel) -> Self {
        self.extra_labels.push(label);
        self
    }

    /// Add a hint for fixing the error.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Add a code suggestion for auto-fix.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: CodeSuggestion) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    /// Check if this error has any extra context (labels, hints, or suggestions).
    pub fn has_extra_context(&self) -> bool {
        !self.extra_labels.is_empty() || self.hint.is_some() || self.suggestion.is_some()
    }

    /// Convert to a `Diagnostic` for rendering.
    ///
    /// This bridges the parser's rich error infrastructure with the diagnostic
    /// system, enabling cross-file labels and structured suggestions to flow
    /// through to the terminal/JSON/SARIF emitters.
    ///
    /// # Arguments
    ///
    /// * `primary_span` - The span of the primary error location
    pub fn to_diagnostic(&self, primary_span: Span) -> Diagnostic {
        let mut diag = Diagnostic::error(self.error_code)
            .with_message(&self.text)
            .with_label(primary_span, &self.label_text);

        // Add extra labels (supports both same-file and cross-file)
        for extra in &self.extra_labels {
            if let Some(ref src_info) = extra.src_info {
                diag =
                    diag.with_cross_file_secondary_label(extra.span, &extra.text, src_info.clone());
            } else {
                diag = diag.with_secondary_label(extra.span, &extra.text);
            }
        }

        // Add hint as a suggestion
        if let Some(ref hint) = self.hint {
            diag = diag.with_suggestion(hint);
        }

        // Add code suggestion as structured fix
        if let Some(ref suggestion) = self.suggestion {
            diag = diag.with_structured_suggestion(ori_diagnostic::Suggestion::new(
                &suggestion.message,
                suggestion.span,
                &suggestion.replacement,
                suggestion.applicability,
                0,
            ));
        }

        diag
    }
}

/// Parse error with error code for rich diagnostics.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseError {
    /// Error code for searchability.
    pub(crate) code: ErrorCode,
    /// Human-readable message.
    pub(crate) message: String,
    /// Location of the error.
    pub(crate) span: Span,
    /// Optional context for suggestions.
    pub(crate) context: Option<String>,
    /// Optional help messages.
    pub(crate) help: Vec<String>,
    /// Severity level for diagnostic queue suppression.
    ///
    /// `Hard` errors are always reported; `Soft` errors (from `EmptyErr` — the parser
    /// didn't consume any tokens) can be suppressed after a hard error to reduce noise.
    pub(crate) severity: DiagnosticSeverity,
}

impl ParseError {
    // --- Accessors ---

    /// Error code for searchability.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Location of the error.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Optional context for suggestions.
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Optional help messages.
    pub fn help(&self) -> &[String] {
        &self.help
    }

    /// Severity level for diagnostic queue suppression.
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    // --- Constructors ---

    /// Create a new parse error.
    ///
    /// Defaults to `Hard` severity (always reported).
    #[cold]
    pub fn new(code: ori_diagnostic::ErrorCode, message: impl Into<String>, span: Span) -> Self {
        ParseError {
            code,
            message: message.into(),
            span,
            context: None,
            help: Vec::new(),
            severity: DiagnosticSeverity::Hard,
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

    /// Create a parse error from a set of expected tokens.
    ///
    /// Used by `ParseOutcome::EmptyErr` when converting to `ParseError`.
    /// The `position` is a **byte offset** in the source, converted to a
    /// zero-length span at that location.
    ///
    /// Returns a `Soft` error: the parser didn't consume any tokens, so this
    /// is a speculative failure that can be suppressed after a hard error.
    #[cold]
    pub fn from_expected_tokens(expected: &crate::TokenSet, position: usize) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "position fits in u32 for source files"
        )]
        let span = Span::point(position as u32);
        let expected_str = expected.format_expected();
        ParseError::new(ErrorCode::E1001, format!("expected {expected_str}"), span).as_soft()
    }

    /// Create a parse error from expected tokens with additional context.
    ///
    /// Used by the `require!` macro to convert soft errors to hard errors
    /// with context about what was being parsed.
    #[cold]
    pub fn from_expected_tokens_with_context(
        expected: &crate::TokenSet,
        position: usize,
        context: &str,
    ) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "position fits in u32 for source files"
        )]
        let span = Span::point(position as u32);
        let expected_str = expected.format_expected();
        ParseError::new(ErrorCode::E1002, format!("expected {expected_str}"), span)
            .with_context(format!("while parsing {context}"))
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

    /// Mark this error as soft (suppressible after a hard error).
    #[must_use]
    pub fn as_soft(mut self) -> Self {
        self.severity = DiagnosticSeverity::Soft;
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

    /// Convert to a Diagnostic bundled with severity for `DiagnosticQueue` routing.
    ///
    /// Convenience method that avoids callers needing to separately access
    /// `to_diagnostic()` and `severity()` when routing through a `DiagnosticQueue`.
    pub fn to_queued_diagnostic(&self) -> (ori_diagnostic::Diagnostic, DiagnosticSeverity) {
        (self.to_diagnostic(), self.severity)
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
        let educational = kind.educational_note();

        let mut error = ParseError {
            code,
            message,
            span,
            context: None,
            help: Vec::new(),
            severity: DiagnosticSeverity::Hard,
        };

        // Add hint first (most actionable)
        if let Some(hint) = hint {
            error.help.push(hint.to_string());
        }

        // Add educational note (more context)
        if let Some(note) = educational {
            error.help.push(note.to_string());
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

    /// Create a [`ParseError`] for an error token with source-based mistake detection.
    ///
    /// This examines the actual source text that caused the lexer error
    /// to provide targeted help for common patterns from other languages.
    #[cold]
    pub fn from_error_token(span: Span, source_text: &str) -> Self {
        if let Some((description, help)) = detect_common_mistake(source_text) {
            ParseError {
                code: ErrorCode::E1001,
                message: format!("unrecognized {description}: `{source_text}`"),
                span,
                context: None,
                help: vec![help.to_string()],
                severity: DiagnosticSeverity::Hard,
            }
        } else {
            ParseError {
                code: ErrorCode::E1001,
                message: format!("unrecognized token: `{source_text}`"),
                span,
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            }
        }
    }
}

impl From<(ParseErrorKind, Span)> for ParseError {
    fn from((kind, span): (ParseErrorKind, Span)) -> Self {
        ParseError::from_kind(&kind, span)
    }
}

/// Reason why a doc comment is detached from any declaration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DetachmentReason {
    /// A blank line separates the comment from the next declaration.
    BlankLine,
    /// A regular (non-doc) comment interrupts between this doc comment
    /// and the declaration.
    RegularCommentInterrupting,
    /// The doc comment appears at end of file with no following declaration.
    NoFollowingDeclaration,
    /// Multiple blank lines or other content separates from declaration.
    TooFarFromDeclaration,
}

impl DetachmentReason {
    /// Get a user-friendly hint explaining why the comment is detached.
    pub fn hint(&self) -> &'static str {
        match self {
            DetachmentReason::BlankLine => {
                "There's a blank line between this doc comment and the next \
                 declaration. Remove the blank line to attach the comment."
            }
            DetachmentReason::RegularCommentInterrupting => {
                "A regular comment (`//`) appears between this doc comment and \
                 the declaration. Doc comments must be immediately before the \
                 declaration they document."
            }
            DetachmentReason::NoFollowingDeclaration => {
                "This doc comment isn't followed by any declaration. Doc comments \
                 should appear immediately before functions, types, or other \
                 declarations."
            }
            DetachmentReason::TooFarFromDeclaration => {
                "This doc comment is too far from the next declaration. Move it \
                 directly above the item you want to document."
            }
        }
    }
}

/// A parse warning (non-fatal diagnostic).
///
/// Warnings don't prevent compilation but indicate potential issues
/// like detached doc comments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ParseWarning {
    /// A doc comment that isn't attached to any declaration.
    DetachedDocComment {
        /// Location of the doc comment.
        span: Span,
        /// Why the comment is considered detached.
        reason: DetachmentReason,
    },
    /// An unknown calling convention string in an `extern` block.
    UnknownCallingConvention {
        /// Location of the convention string literal.
        span: Span,
        /// The convention string that was used.
        convention: String,
    },
}

impl ParseWarning {
    /// Create a warning for a detached doc comment.
    pub fn detached_doc_comment(span: Span, reason: DetachmentReason) -> Self {
        ParseWarning::DetachedDocComment { span, reason }
    }

    /// Get the span of the warning.
    pub fn span(&self) -> Span {
        match self {
            ParseWarning::DetachedDocComment { span, .. }
            | ParseWarning::UnknownCallingConvention { span, .. } => *span,
        }
    }

    /// Get a title for the warning.
    pub fn title(&self) -> &'static str {
        match self {
            ParseWarning::DetachedDocComment { .. } => "DETACHED DOC COMMENT",
            ParseWarning::UnknownCallingConvention { .. } => "UNKNOWN CALLING CONVENTION",
        }
    }

    /// Get the warning message.
    pub fn message(&self) -> String {
        match self {
            ParseWarning::DetachedDocComment { reason, .. } => {
                format!(
                    "This doc comment isn't attached to any declaration. {}",
                    reason.hint()
                )
            }
            ParseWarning::UnknownCallingConvention { convention, .. } => {
                format!("unknown calling convention \"{convention}\"; expected \"c\" or \"js\"")
            }
        }
    }

    /// Convert to a diagnostic for display.
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            ParseWarning::DetachedDocComment { .. } => Diagnostic::warning(ErrorCode::W1001)
                .with_message(self.message())
                .with_label(self.span(), "detached doc comment"),
            ParseWarning::UnknownCallingConvention { convention, .. } => {
                Diagnostic::warning(ErrorCode::W1002)
                    .with_message(self.message())
                    .with_label(self.span(), format!("unknown convention \"{convention}\""))
            }
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
