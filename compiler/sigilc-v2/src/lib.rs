//! Sigil V2 Compiler
//!
//! A high-performance, incremental compiler for the Sigil programming language
//! using Semantic Structural Compilation (SSC) and Salsa-based query system.

pub mod intern;
pub mod syntax;
pub mod db;
pub mod errors;
pub mod hir;
pub mod check;
pub mod patterns;
pub mod eval;
pub mod parallel;

// Re-exports
pub use db::{CompilerDb, Db};
pub use intern::{Name, StringInterner, TypeId, TypeInterner, TypeKind, TypeRange};
pub use syntax::{ExprId, ExprRange, Span, Lexer, Parser, TokenList};
pub use hir::{Scopes, ScopeId, DefinitionRegistry, Resolver, ResolvedName};
pub use check::{TypeContext, Unifier, TypeError, TypeErrorKind};
pub use patterns::{PatternDefinition, PatternRegistry, PatternSignature};
pub use eval::{Value, Environment};
