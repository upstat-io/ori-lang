// AST (Abstract Syntax Tree) definitions for Sigil
//
// The AST is split into several submodules:
// - items.rs: Top-level definitions (functions, tests, configs, types)
// - types.rs: Type expressions
// - expr.rs: Expression enum
// - patterns.rs: Pattern expressions
// - matching.rs: Match expressions
// - operators.rs: Binary and unary operators

mod expr;
mod items;
mod matching;
mod operators;
mod patterns;
mod types;
pub mod visit;

// Re-export all public types
pub use expr::Expr;
pub use items::{ConfigDef, Field, FunctionDef, Param, TestDef, TypeDef, TypeDefKind, UseDef, UseItem, Variant};
pub use matching::{MatchArm, MatchExpr, Pattern};
pub use operators::{BinaryOp, UnaryOp};
pub use patterns::{IterDirection, OnError, PatternExpr};
pub use types::TypeExpr;
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
}

/// Source span
pub type Span = std::ops::Range<usize>;
