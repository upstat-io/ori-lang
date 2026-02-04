//! Parse error types.
//!
//! Provides structured error types for the parser with:
//! - Rich error variants capturing context
//! - Contextual hints for common mistakes
//! - Related location tracking for better diagnostics
//! - `ErrorContext` for Elm-style "while parsing X" messages

use ori_diagnostic::{Diagnostic, ErrorCode, Label};
// Re-export SourceInfo from ori_diagnostic for use in cross-file error labels
pub use ori_diagnostic::SourceInfo;
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
    pub fn details(&self, error_span: Span) -> ParseErrorDetails {
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
#[allow(dead_code)] // Infrastructure for multi-span diagnostics
pub struct Note {
    /// The message explaining this location.
    pub message: String,
    /// The related source location, if any.
    pub span: Option<Span>,
}

// ============================================================================
// ParseErrorDetails - Comprehensive Error Information (Section 04.1)
// ============================================================================

/// Confidence level for auto-fix suggestions.
///
/// Inspired by Rust's `Applicability` in `rustc_errors`. This determines
/// whether an IDE or formatter can safely auto-apply a suggestion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Applicability {
    /// Safe to apply automatically.
    ///
    /// The fix is definitely correct and won't change semantics.
    /// Example: Adding a missing semicolon, fixing typos in keywords.
    MachineApplicable,

    /// May need human review (the default).
    ///
    /// The fix is likely correct but involves judgment calls.
    /// Example: Suggesting an alternative spelling for an undefined name.
    #[default]
    MaybeIncorrect,

    /// Just a hint, don't auto-apply.
    ///
    /// The suggestion has placeholders or requires user input.
    /// Example: "Consider adding a type annotation here: `let x: ???`"
    HasPlaceholders,
}

// NOTE: SourceInfo is re-exported from ori_diagnostic at the top of this file.
// This keeps a single source of truth for the cross-file label infrastructure.

/// A secondary label pointing to related code.
///
/// Extra labels provide additional context by highlighting related locations.
/// They're particularly useful for errors like:
/// - "unclosed delimiter" → pointing to where it was opened
/// - "type mismatch" → pointing to the expected type declaration
/// - "duplicate definition" → pointing to the first definition
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExtraLabel {
    /// The source location to highlight.
    pub span: Span,
    /// Optional source info if this label is in a different file.
    pub src_info: Option<SourceInfo>,
    /// The label text explaining this location.
    pub text: String,
}

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
pub struct CodeSuggestion {
    /// The span to replace (what to remove).
    pub span: Span,
    /// The replacement text (what to insert).
    pub replacement: String,
    /// Human-readable description of the fix.
    pub message: String,
    /// Confidence level for auto-application.
    pub applicability: Applicability,
}

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
pub struct ParseErrorDetails {
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
                // Cross-file label
                diag.labels.push(Label::secondary_cross_file(
                    extra.span,
                    &extra.text,
                    src_info.clone(),
                ));
            } else {
                // Same-file label
                diag.labels.push(Label::secondary(extra.span, &extra.text));
            }
        }

        // Add hint as a suggestion
        if let Some(ref hint) = self.hint {
            diag = diag.with_suggestion(hint);
        }

        // Add code suggestion as structured fix
        if let Some(ref suggestion) = self.suggestion {
            let applicability = match suggestion.applicability {
                Applicability::MachineApplicable => {
                    ori_diagnostic::Applicability::MachineApplicable
                }
                Applicability::MaybeIncorrect => ori_diagnostic::Applicability::MaybeIncorrect,
                Applicability::HasPlaceholders => ori_diagnostic::Applicability::HasPlaceholders,
            };
            diag = diag.with_structured_suggestion(ori_diagnostic::Suggestion::new(
                &suggestion.message,
                suggestion.span,
                &suggestion.replacement,
                applicability,
            ));
        }

        diag
    }
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

    /// Create a parse error from a set of expected tokens.
    ///
    /// Used by `ParseOutcome::EmptyErr` when converting to `ParseError`.
    /// The position is converted to a zero-length span at that location.
    #[cold]
    pub fn from_expected_tokens(expected: &crate::TokenSet, position: usize) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "position fits in u32 for source files"
        )]
        let span = Span::new(position as u32, 0);
        let expected_str = expected.format_expected();
        ParseError::new(ErrorCode::E1001, format!("expected {expected_str}"), span)
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
        let span = Span::new(position as u32, 0);
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
        let educational = kind.educational_note();

        let mut error = ParseError {
            code,
            message,
            span,
            context: None,
            help: Vec::new(),
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
            }
        } else {
            ParseError {
                code: ErrorCode::E1001,
                message: format!("unrecognized token: `{source_text}`"),
                span,
                context: None,
                help: Vec::new(),
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
}

impl ParseWarning {
    /// Create a warning for a detached doc comment.
    pub fn detached_doc_comment(span: Span, reason: DetachmentReason) -> Self {
        ParseWarning::DetachedDocComment { span, reason }
    }

    /// Get the span of the warning.
    pub fn span(&self) -> Span {
        match self {
            ParseWarning::DetachedDocComment { span, .. } => *span,
        }
    }

    /// Get a title for the warning.
    pub fn title(&self) -> &'static str {
        match self {
            ParseWarning::DetachedDocComment { .. } => "DETACHED DOC COMMENT",
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
        }
    }

    /// Convert to a diagnostic for display.
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::warning(ErrorCode::W1001)
            .with_message(self.message())
            .with_label(self.span(), "detached doc comment")
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

    #[test]
    fn test_title() {
        assert_eq!(
            ParseErrorKind::UnexpectedToken {
                found: TokenKind::Plus,
                expected: "identifier",
                context: None
            }
            .title(),
            "UNEXPECTED TOKEN"
        );
        assert_eq!(
            ParseErrorKind::UnclosedDelimiter {
                open: TokenKind::LParen,
                open_span: Span::DUMMY,
                expected_close: TokenKind::RParen
            }
            .title(),
            "UNCLOSED DELIMITER"
        );
    }

    #[test]
    fn test_empathetic_unexpected_token() {
        let kind = ParseErrorKind::UnexpectedToken {
            found: TokenKind::Semicolon,
            expected: "an expression",
            context: Some("function body"),
        };
        let msg = kind.empathetic_message();

        // Check for empathetic phrasing
        assert!(msg.contains("I ran into"));
        assert!(msg.contains("while parsing function body"));
        assert!(msg.contains("I was expecting"));
        assert!(msg.contains("`;\u{60}")); // backtick-semicolon-backtick
    }

    #[test]
    fn test_empathetic_expected_expression() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::Plus,
            position: ExprPosition::Operand,
        };
        let msg = kind.empathetic_message();

        assert!(msg.contains("I was expecting an expression"));
        assert!(msg.contains("after this operator"));
        assert!(msg.contains("Expressions include"));
    }

    #[test]
    fn test_empathetic_trailing_operator() {
        let kind = ParseErrorKind::TrailingOperator {
            operator: TokenKind::Plus,
        };
        let msg = kind.empathetic_message();

        assert!(msg.contains("without a right-hand side"));
        assert!(msg.contains("a + b"));
    }

    #[test]
    fn test_empathetic_unclosed_delimiter() {
        let kind = ParseErrorKind::UnclosedDelimiter {
            open: TokenKind::LParen,
            open_span: Span::new(10, 11),
            expected_close: TokenKind::RParen,
        };
        let msg = kind.empathetic_message();

        assert!(msg.contains("unclosed `(`"));
        assert!(msg.contains("matching `)`"));
    }

    #[test]
    fn test_empathetic_expected_declaration() {
        let kind = ParseErrorKind::ExpectedDeclaration {
            found: TokenKind::Plus,
        };
        let msg = kind.empathetic_message();

        assert!(msg.contains("I was expecting a declaration"));
        assert!(msg.contains("Functions:"));
        assert!(msg.contains("Types:"));
        assert!(msg.contains("Imports:"));
    }

    #[test]
    fn test_empathetic_unsupported_keyword() {
        let kind = ParseErrorKind::UnsupportedKeyword {
            keyword: TokenKind::Return,
            reason: "Ori is expression-based",
        };
        let msg = kind.empathetic_message();

        assert!(msg.contains("`return` isn't supported"));
        assert!(msg.contains("Ori is expression-based"));
    }

    // === Common Mistake Detection Tests ===

    #[test]
    fn test_detect_triple_equals() {
        let (desc, help) = detect_common_mistake("===").unwrap();
        assert_eq!(desc, "triple equals");
        assert!(help.contains("=="));
        assert!(help.contains("statically typed"));
    }

    #[test]
    fn test_detect_increment_operator() {
        let (desc, help) = detect_common_mistake("++").unwrap();
        assert_eq!(desc, "increment operator");
        assert!(help.contains("x = x + 1"));
    }

    #[test]
    fn test_detect_decrement_operator() {
        let (desc, help) = detect_common_mistake("--").unwrap();
        assert_eq!(desc, "decrement operator");
        assert!(help.contains("x = x - 1"));
    }

    #[test]
    fn test_detect_compound_assignment() {
        for op in &["+=", "-=", "*=", "/=", "%="] {
            let result = detect_common_mistake(op);
            assert!(result.is_some(), "Should detect {op}");
            let (desc, help) = result.unwrap();
            assert_eq!(desc, "compound assignment");
            assert!(help.contains("x = x"));
        }
    }

    #[test]
    fn test_detect_class_keyword() {
        let (desc, help) = check_common_keyword_mistake("class").unwrap();
        assert_eq!(desc, "class keyword");
        assert!(help.contains("type"));
        assert!(help.contains("trait"));
    }

    #[test]
    fn test_detect_switch_keyword() {
        let (desc, help) = check_common_keyword_mistake("switch").unwrap();
        assert_eq!(desc, "switch keyword");
        assert!(help.contains("match"));
    }

    #[test]
    fn test_detect_function_keyword() {
        for keyword in &["function", "func", "fn"] {
            let result = check_common_keyword_mistake(keyword);
            assert!(result.is_some(), "Should detect {keyword}");
            let (desc, help) = result.unwrap();
            assert_eq!(desc, "function keyword");
            assert!(help.contains('@'));
        }
    }

    #[test]
    fn test_detect_null_variants() {
        for keyword in &["null", "nil", "NULL"] {
            let result = check_common_keyword_mistake(keyword);
            assert!(result.is_some(), "Should detect {keyword}");
            let (_, help) = result.unwrap();
            assert!(help.contains("None"));
        }
    }

    #[test]
    fn test_detect_string_type() {
        let (desc, help) = check_common_keyword_mistake("String").unwrap();
        assert_eq!(desc, "string type");
        assert!(help.contains("str"));
    }

    #[test]
    fn test_detect_boolean_case() {
        let (desc, help) = check_common_keyword_mistake("True").unwrap();
        assert_eq!(desc, "boolean literal");
        assert!(help.contains("true"));
        assert!(help.contains("false"));
    }

    #[test]
    fn test_valid_tokens_not_detected() {
        // These should NOT be detected as mistakes (they're valid in Ori)
        assert!(detect_common_mistake("??").is_none());
        assert!(detect_common_mistake("=>").is_none());
        assert!(check_common_keyword_mistake("int").is_none());
        assert!(check_common_keyword_mistake("float").is_none());
        assert!(check_common_keyword_mistake("str").is_none());
    }

    // === Educational Note Tests ===

    #[test]
    fn test_educational_note_conditional() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::RBrace,
            position: ExprPosition::Conditional,
        };
        let note = kind.educational_note();
        assert!(note.is_some());
        assert!(note.unwrap().contains("expression"));
        assert!(note.unwrap().contains("same type"));
    }

    #[test]
    fn test_educational_note_match_arm() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::Comma,
            position: ExprPosition::MatchArm,
        };
        let note = kind.educational_note();
        assert!(note.is_some());
        assert!(note.unwrap().contains("match"));
    }

    #[test]
    fn test_educational_note_let_pattern() {
        let kind = ParseErrorKind::InvalidPattern {
            found: TokenKind::Plus,
            context: PatternContext::Let,
        };
        let note = kind.educational_note();
        assert!(note.is_some());
        assert!(note.unwrap().contains("destructuring"));
    }

    #[test]
    fn test_educational_note_unclosed_brace() {
        let kind = ParseErrorKind::UnclosedDelimiter {
            open: TokenKind::LBrace,
            open_span: Span::DUMMY,
            expected_close: TokenKind::RBrace,
        };
        let note = kind.educational_note();
        assert!(note.is_some());
        assert!(note.unwrap().contains("blocks"));
    }

    #[test]
    fn test_educational_note_unclosed_bracket() {
        let kind = ParseErrorKind::UnclosedDelimiter {
            open: TokenKind::LBracket,
            open_span: Span::DUMMY,
            expected_close: TokenKind::RBracket,
        };
        let note = kind.educational_note();
        assert!(note.is_some());
        assert!(note.unwrap().contains("list"));
    }

    // === From Error Token Tests ===

    #[test]
    fn test_from_error_token_with_known_mistake() {
        let error = ParseError::from_error_token(Span::new(0, 3), "===");
        assert!(error.message.contains("triple equals"));
        assert!(!error.help.is_empty());
        assert!(error.help[0].contains("=="));
    }

    #[test]
    fn test_from_error_token_with_unknown() {
        let error = ParseError::from_error_token(Span::new(0, 3), "xyz");
        assert!(error.message.contains("unrecognized token"));
        assert!(error.help.is_empty());
    }

    // === Enhanced Hint Tests ===

    #[test]
    fn test_enhanced_hint_semicolon() {
        let kind = ParseErrorKind::UnexpectedToken {
            found: TokenKind::Semicolon,
            expected: "expression",
            context: None,
        };
        let hint = kind.hint().unwrap();
        assert!(hint.contains("semicolons"));
        assert!(hint.contains("Remove"));
    }

    #[test]
    fn test_enhanced_hint_trailing_star() {
        let kind = ParseErrorKind::TrailingOperator {
            operator: TokenKind::Star,
        };
        let hint = kind.hint().unwrap();
        assert!(hint.contains('*'));
        assert!(hint.contains("both sides"));
    }

    #[test]
    fn test_enhanced_hint_empty_block() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::RBrace,
            position: ExprPosition::Primary,
        };
        let hint = kind.hint().unwrap();
        assert!(hint.contains("void"));
    }

    // === Integration: from_kind with educational notes ===

    #[test]
    fn test_from_kind_includes_educational_note() {
        let kind = ParseErrorKind::InvalidPattern {
            found: TokenKind::Plus,
            context: PatternContext::Match,
        };
        let error = ParseError::from_kind(&kind, Span::new(0, 1));

        // Should have both hint (if any) and educational note
        // For InvalidPattern in Match context, we have an educational note
        assert!(
            !error.help.is_empty(),
            "Should have at least educational note"
        );
        let combined_help = error.help.join(" ");
        assert!(
            combined_help.contains("pattern"),
            "Help should mention patterns"
        );
    }

    #[test]
    fn test_from_kind_includes_hint_and_educational() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::RBrace,
            position: ExprPosition::Conditional,
        };
        let error = ParseError::from_kind(&kind, Span::new(0, 1));

        // Should have both hint (for empty block) and educational note (for conditional)
        assert!(!error.help.is_empty(), "Should have help messages");
    }

    // === ErrorContext Tests ===

    #[test]
    fn test_error_context_description() {
        assert_eq!(ErrorContext::IfExpression.description(), "an if expression");
        assert_eq!(
            ErrorContext::MatchExpression.description(),
            "a match expression"
        );
        assert_eq!(
            ErrorContext::FunctionDef.description(),
            "a function definition"
        );
        assert_eq!(ErrorContext::Pattern.description(), "a pattern");
    }

    #[test]
    fn test_error_context_label() {
        assert_eq!(ErrorContext::IfExpression.label(), "if expression");
        assert_eq!(ErrorContext::MatchExpression.label(), "match expression");
        assert_eq!(ErrorContext::FunctionDef.label(), "function definition");
        assert_eq!(ErrorContext::Pattern.label(), "pattern");
    }

    #[test]
    fn test_error_context_all_variants_have_description() {
        // Ensure all variants have non-empty descriptions
        let contexts = [
            ErrorContext::Module,
            ErrorContext::FunctionDef,
            ErrorContext::TypeDef,
            ErrorContext::TraitDef,
            ErrorContext::ImplBlock,
            ErrorContext::UseStatement,
            ErrorContext::Expression,
            ErrorContext::IfExpression,
            ErrorContext::MatchExpression,
            ErrorContext::ForLoop,
            ErrorContext::WhileLoop,
            ErrorContext::Block,
            ErrorContext::Closure,
            ErrorContext::FunctionCall,
            ErrorContext::MethodCall,
            ErrorContext::ListLiteral,
            ErrorContext::MapLiteral,
            ErrorContext::StructLiteral,
            ErrorContext::IndexExpression,
            ErrorContext::BinaryOp,
            ErrorContext::FieldAccess,
            ErrorContext::Pattern,
            ErrorContext::MatchArm,
            ErrorContext::LetPattern,
            ErrorContext::FunctionParams,
            ErrorContext::TypeAnnotation,
            ErrorContext::GenericParams,
            ErrorContext::FunctionSignature,
            ErrorContext::Attribute,
            ErrorContext::TestDef,
            ErrorContext::Contract,
        ];

        for ctx in &contexts {
            let desc = ctx.description();
            assert!(
                !desc.is_empty(),
                "Description for {ctx:?} should not be empty"
            );
            // Descriptions should read naturally after "while parsing"
            // e.g., "while parsing an if expression" or "while parsing function parameters"
            assert!(
                desc.starts_with("a ")
                    || desc.starts_with("an ")
                    || !desc.contains(' ')
                    || desc.ends_with('s'),
                "Description for {ctx:?} should be grammatically correct: {desc}"
            );

            let label = ctx.label();
            assert!(!label.is_empty(), "Label for {ctx:?} should not be empty");
        }
    }

    // === ParseErrorDetails Tests ===

    #[test]
    fn test_details_unexpected_token() {
        let kind = ParseErrorKind::UnexpectedToken {
            found: TokenKind::Semicolon,
            expected: "an expression",
            context: Some("function body"),
        };
        let details = kind.details(Span::new(10, 1));

        assert_eq!(details.title, "UNEXPECTED TOKEN");
        assert!(details.text.contains("I ran into"));
        assert!(details.text.contains("function body"));
        assert!(details.label_text.contains("expected"));
        assert!(details.hint.is_some()); // Has semicolon hint
        assert_eq!(details.error_code, ErrorCode::E1001);
    }

    #[test]
    fn test_details_unclosed_delimiter() {
        let kind = ParseErrorKind::UnclosedDelimiter {
            open: TokenKind::LParen,
            open_span: Span::new(5, 1),
            expected_close: TokenKind::RParen,
        };
        let details = kind.details(Span::new(20, 0));

        assert_eq!(details.title, "UNCLOSED DELIMITER");
        assert!(details.text.contains("unclosed"));
        assert!(!details.extra_labels.is_empty());
        assert!(details.extra_labels[0].text.contains("opened here"));
        assert!(details.suggestion.is_some());
        assert_eq!(
            details.suggestion.as_ref().unwrap().applicability,
            Applicability::MachineApplicable
        );
    }

    #[test]
    fn test_details_expected_expression() {
        let kind = ParseErrorKind::ExpectedExpression {
            found: TokenKind::RBrace,
            position: ExprPosition::Conditional,
        };
        let details = kind.details(Span::new(15, 1));

        assert_eq!(details.title, "EXPECTED EXPRESSION");
        assert!(details.text.contains("condition"));
        assert!(details.hint.is_some()); // Has educational note or hint
    }

    #[test]
    fn test_details_trailing_operator() {
        let kind = ParseErrorKind::TrailingOperator {
            operator: TokenKind::Plus,
        };
        let details = kind.details(Span::new(8, 1));

        assert_eq!(details.title, "INCOMPLETE EXPRESSION");
        assert!(details.text.contains("right-hand side"));
        assert!(details.label_text.contains("needs a right operand"));
    }

    #[test]
    fn test_details_pattern_error_missing() {
        let kind = ParseErrorKind::PatternArgumentError {
            pattern_name: "cache",
            reason: PatternArgError::Missing { name: "key" },
        };
        let details = kind.details(Span::new(0, 5));

        assert_eq!(details.title, "PATTERN ERROR");
        assert!(details.text.contains("cache"));
        assert!(details.text.contains("key"));
        assert!(details.label_text.contains("missing"));
    }

    #[test]
    fn test_details_unexpected_eof_with_unclosed() {
        let kind = ParseErrorKind::UnexpectedEof {
            expected: "expression",
            unclosed: Some((TokenKind::LBrace, Span::new(2, 1))),
        };
        let details = kind.details(Span::new(50, 0));

        assert_eq!(details.title, "UNEXPECTED END OF FILE");
        assert!(details.text.contains("closing"));
        assert!(!details.extra_labels.is_empty());
        assert!(details.extra_labels[0].text.contains("opened"));
    }

    // === Applicability Tests ===

    #[test]
    fn test_applicability_default() {
        assert_eq!(Applicability::default(), Applicability::MaybeIncorrect);
    }

    // === CodeSuggestion Tests ===

    #[test]
    fn test_code_suggestion_machine_applicable() {
        let suggestion =
            CodeSuggestion::machine_applicable(Span::new(10, 3), "==", "Replace `===` with `==`");
        assert_eq!(suggestion.replacement, "==");
        assert_eq!(suggestion.applicability, Applicability::MachineApplicable);
    }

    #[test]
    fn test_code_suggestion_with_placeholders() {
        let suggestion =
            CodeSuggestion::with_placeholders(Span::new(10, 0), ": ???", "Add type annotation");
        assert_eq!(suggestion.applicability, Applicability::HasPlaceholders);
    }

    // === ExtraLabel Tests ===

    #[test]
    fn test_extra_label_same_file() {
        let label = ExtraLabel::same_file(Span::new(5, 1), "opened here");
        assert!(label.src_info.is_none());
        assert_eq!(label.text, "opened here");
    }

    #[test]
    fn test_extra_label_cross_file() {
        let label = ExtraLabel::cross_file(
            Span::new(10, 5),
            "src/lib.ori",
            "fn foo() { }",
            "defined here",
        );
        assert!(label.src_info.is_some());
        let info = label.src_info.unwrap();
        assert_eq!(info.path, "src/lib.ori");
        assert!(info.content.contains("foo"));
    }

    // === ParseErrorDetails Builder Tests ===

    #[test]
    fn test_parse_error_details_builder() {
        let details = ParseErrorDetails::new(
            "TEST ERROR",
            "Test explanation",
            "test label",
            ErrorCode::E1001,
        )
        .with_hint("Try this fix")
        .with_extra_label(ExtraLabel::same_file(Span::new(0, 1), "related"))
        .with_suggestion(CodeSuggestion::machine_applicable(
            Span::new(5, 2),
            "fix",
            "Apply fix",
        ));

        assert_eq!(details.title, "TEST ERROR");
        assert!(details.has_extra_context());
        assert!(details.hint.is_some());
        assert!(!details.extra_labels.is_empty());
        assert!(details.suggestion.is_some());
    }

    #[test]
    fn test_parse_error_details_has_extra_context() {
        let basic = ParseErrorDetails::new("TEST", "text", "label", ErrorCode::E1001);
        assert!(!basic.has_extra_context());

        let with_hint = basic.clone().with_hint("hint");
        assert!(with_hint.has_extra_context());
    }

    // === Integration: details() generates complete information ===

    #[test]
    fn test_details_all_variants_produce_output() {
        // Ensure all error variants produce valid details
        let variants: Vec<ParseErrorKind> = vec![
            ParseErrorKind::UnexpectedToken {
                found: TokenKind::Plus,
                expected: "identifier",
                context: None,
            },
            ParseErrorKind::UnexpectedEof {
                expected: "expression",
                unclosed: None,
            },
            ParseErrorKind::ExpectedExpression {
                found: TokenKind::Plus,
                position: ExprPosition::Primary,
            },
            ParseErrorKind::TrailingOperator {
                operator: TokenKind::Star,
            },
            ParseErrorKind::ExpectedDeclaration {
                found: TokenKind::Plus,
            },
            ParseErrorKind::ExpectedIdentifier {
                found: TokenKind::Plus,
                context: IdentContext::FunctionName,
            },
            ParseErrorKind::InvalidFunctionClause {
                reason: "test reason",
            },
            ParseErrorKind::InvalidPattern {
                found: TokenKind::Plus,
                context: PatternContext::Match,
            },
            ParseErrorKind::PatternArgumentError {
                pattern_name: "test",
                reason: PatternArgError::Missing { name: "arg" },
            },
            ParseErrorKind::ExpectedType {
                found: TokenKind::Plus,
            },
            ParseErrorKind::UnclosedDelimiter {
                open: TokenKind::LBrace,
                open_span: Span::new(0, 1),
                expected_close: TokenKind::RBrace,
            },
            ParseErrorKind::InvalidAttribute {
                reason: "test reason",
            },
            ParseErrorKind::UnsupportedKeyword {
                keyword: TokenKind::Return,
                reason: "test reason",
            },
        ];

        for kind in &variants {
            let details = kind.details(Span::new(0, 1));
            assert!(
                !details.title.is_empty(),
                "Title should not be empty for {kind:?}"
            );
            assert!(
                !details.text.is_empty(),
                "Text should not be empty for {kind:?}"
            );
            assert!(
                !details.label_text.is_empty(),
                "Label text should not be empty for {kind:?}"
            );
        }
    }

    // === Diagnostic Conversion Tests ===

    #[test]
    fn test_parse_error_details_to_diagnostic() {
        let details = ParseErrorDetails::new(
            "UNEXPECTED TOKEN",
            "I ran into something unexpected",
            "expected expression",
            ErrorCode::E1001,
        )
        .with_hint("Try removing this");

        let diag = details.to_diagnostic(Span::new(10, 20));

        assert_eq!(diag.code, ErrorCode::E1001);
        assert!(diag.message.contains("I ran into"));
        assert_eq!(diag.labels.len(), 1);
        assert!(diag.labels[0].is_primary);
        assert_eq!(diag.labels[0].span, Span::new(10, 20));
        assert!(diag.labels[0].message.contains("expected expression"));
        assert!(!diag.suggestions.is_empty());
    }

    #[test]
    fn test_parse_error_details_to_diagnostic_with_extra_labels() {
        let details = ParseErrorDetails::new(
            "UNCLOSED DELIMITER",
            "I found an unclosed `{`",
            "expected `}` here",
            ErrorCode::E1003,
        )
        .with_extra_label(ExtraLabel::same_file(
            Span::new(0, 1),
            "the `{` was opened here",
        ));

        let diag = details.to_diagnostic(Span::new(50, 50));

        assert_eq!(diag.labels.len(), 2);
        assert!(diag.labels[0].is_primary);
        assert!(!diag.labels[1].is_primary);
        assert_eq!(diag.labels[1].span, Span::new(0, 1));
        assert!(diag.labels[1].message.contains("opened here"));
    }

    #[test]
    fn test_parse_error_details_to_diagnostic_cross_file() {
        let details = ParseErrorDetails::new(
            "TYPE MISMATCH",
            "Expected `int`, found `str`",
            "this expression is `str`",
            ErrorCode::E2001,
        )
        .with_extra_label(ExtraLabel::cross_file(
            Span::new(0, 19),
            "src/lib.ori",
            "@get_name () -> str",
            "return type defined here",
        ));

        let diag = details.to_diagnostic(Span::new(100, 110));

        assert_eq!(diag.labels.len(), 2);
        // Primary label should not be cross-file
        assert!(!diag.labels[0].is_cross_file());
        // Secondary label should be cross-file
        assert!(diag.labels[1].is_cross_file());
        assert_eq!(
            diag.labels[1].source_info.as_ref().unwrap().path,
            "src/lib.ori"
        );
    }

    #[test]
    fn test_parse_error_details_to_diagnostic_with_suggestion() {
        let details = ParseErrorDetails::new(
            "SYNTAX ERROR",
            "Use `==` for equality",
            "found `===`",
            ErrorCode::E1001,
        )
        .with_suggestion(CodeSuggestion::machine_applicable(
            Span::new(5, 8),
            "==",
            "Replace `===` with `==`",
        ));

        let diag = details.to_diagnostic(Span::new(5, 8));

        assert!(!diag.structured_suggestions.is_empty());
        let suggestion = &diag.structured_suggestions[0];
        assert_eq!(suggestion.substitutions[0].snippet, "==");
        assert!(suggestion.applicability.is_machine_applicable());
    }
}
