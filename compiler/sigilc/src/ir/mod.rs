// Typed Intermediate Representation (TIR) for Sigil
//
// The TIR provides a fully typed representation where every expression
// carries its resolved type. This enables:
// - Direct type access during codegen (no re-inference)
// - Pass-based optimizations and transformations
// - Pattern lowering to loops
//
// Pipeline: AST -> TypeChecker+Lower -> TIR -> Passes -> Codegen
//
// Module structure:
// - types.rs: Resolved type enum (Type)
// - expr.rs: Typed expressions (TExpr, TExprKind, LocalId)
// - patterns.rs: Typed patterns (TPattern)
// - module.rs: Module structure (TModule, TFunction, LocalTable)
// - display.rs: Pretty printing for debug

mod display;
mod expr;
pub mod fold;
mod module;
mod patterns;
mod types;
pub mod visit;

// Re-export all public types
pub use display::{dump_tir, DisplayConfig, TIRPrinter};
pub use expr::{FuncRef, LocalId, TExpr, TExprKind, TMatch, TMatchArm, TMatchPattern, TStmt};
pub use fold::Folder;
pub use module::{
    LocalInfo, LocalTable, TConfig, TField, TFunction, TImport, TImportItem, TModule, TParam,
    TTest, TTypeDef, TTypeDefKind, TVariant,
};
pub use patterns::{IterDirection, OnError, RetryBackoff, TPattern};
pub use types::Type;
pub use visit::Visitor;
