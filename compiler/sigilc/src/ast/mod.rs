// AST (Abstract Syntax Tree) definitions for Sigil
//
// The AST is split into several submodules:
// - items.rs: Top-level definitions (functions, tests, configs, types)
// - types.rs: Type expressions
// - expr.rs: Expression enum
// - patterns.rs: Pattern expressions
// - matching.rs: Match expressions
// - operators.rs: Binary and unary operators

pub mod dispatch;
mod expr;
mod items;
mod matching;
mod operators;
mod patterns;
pub mod processor;
mod types;
pub mod visit;

// Re-export all public types
pub use expr::Expr;
// SpannedExpr is defined in this file, no need to re-export
pub use items::{
    AssociatedType, AssociatedTypeImpl, ConfigDef, Field, FunctionDef, ImplBlock, Param,
    TestDef, TraitDef, TraitMethodDef, TypeDef, TypeDefKind, UseDef, UseItem, Variant,
    WhereBound,
};
pub use matching::{MatchArm, MatchExpr, Pattern};
pub use operators::{BinaryOp, UnaryOp};
pub use patterns::{IterDirection, OnError, PatternExpr, RetryBackoff};
pub use types::TypeExpr;
pub use dispatch::{ExprHandler, dispatch_to_handler, EXPR_VARIANTS};
pub use processor::{ExprProcessor, dispatch_to_processor, DefaultExprProcessor};
pub use visit::ExprVisitor;

/// A complete source file / module
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
}

/// Top-level items in a module
#[derive(Debug, Clone)]
pub enum Item {
    /// Config variable: $name = value
    Config(ConfigDef),

    /// Type definition: type Name = ... or type Name { ... }
    TypeDef(TypeDef),

    /// Function definition: @name (...) -> Type = ...
    Function(FunctionDef),

    /// Test definition: @name tests @target (...) -> void = ...
    Test(TestDef),

    /// Use statement: use path { items }
    Use(UseDef),

    /// Trait definition: trait Name<T>: Supertrait { methods }
    Trait(TraitDef),

    /// Implementation block: impl Trait for Type { methods }
    Impl(ImplBlock),
}

/// Source span
pub type Span = std::ops::Range<usize>;

/// An expression with its source span.
/// This wraps the `Expr` enum with location information for error reporting.
#[derive(Debug, Clone)]
pub struct SpannedExpr {
    pub expr: Expr,
    pub span: Span,
}

impl SpannedExpr {
    /// Create a new spanned expression
    pub fn new(expr: Expr, span: Span) -> Self {
        SpannedExpr { expr, span }
    }

    /// Create a spanned expression with no source location (for generated code)
    pub fn no_span(expr: Expr) -> Self {
        SpannedExpr { expr, span: 0..0 }
    }
}
