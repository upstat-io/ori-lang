//! Error context for Elm-style "while parsing X" messages.

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
    /// Parsing a for pattern (`for(over:, match:, default:)`).
    ForPattern,
    /// Parsing a try expression.
    TryExpression,
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
            Self::ForPattern => "a for pattern",
            Self::TryExpression => "a try expression",
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
            Self::ForPattern => "for pattern",
            Self::TryExpression => "try expression",
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
