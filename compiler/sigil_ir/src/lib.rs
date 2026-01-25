//! Sigil IR - Intermediate Representation Types
//!
//! This crate contains the core data structures for the Sigil compiler:
//! - Spans for source locations
//! - Names for interned identifiers
//! - Tokens and `TokenList` for lexer output
//! - AST nodes (Expr, Stmt, Function, etc.)
//! - Arena allocation for expressions
//!
//! # Salsa Compatibility
//!
//! Every type has the required traits for Salsa:
//! - Clone: Required for Salsa storage
//! - Eq + `PartialEq`: Required for early cutoff
//! - Hash: Required for memoization keys
//! - Debug: Required for error messages
//!
//! # Design Philosophy
//!
//! - **Intern Everything**: Strings → Name(u32), Types → TypeId(u32)
//! - **Flatten Everything**: No Box<Expr>, use ExprId(u32) indices
//! - **Interface Segregation**: Focused traits (Spanned, Named)
//!
//! Types that contain floats store them as u64 bits for Hash compatibility.
//! Types that contain strings use interned Name for O(1) equality.

/// Compile-time assertion that a type has a specific size.
///
/// Used to prevent accidental size regressions in frequently-allocated types.
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

mod span;
mod name;
mod token;
mod interner;
mod type_id;
mod expr_id;
mod traits;
mod parsed_type;
pub mod ast;
mod arena;
pub mod visitor;

pub use span::Span;
pub use name::Name;
pub use token::{Token, TokenKind, TokenList, DurationUnit, SizeUnit};
pub use interner::{StringInterner, SharedInterner};
pub use type_id::TypeId;
pub use expr_id::{ExprId, ExprRange, StmtId, StmtRange};
pub use parsed_type::ParsedType;
pub use traits::{Spanned, Named, Typed};
pub use ast::{
    Expr, ExprKind, Stmt, StmtKind, Param, ParamRange,
    BinaryOp, UnaryOp, Function, Module, TestDef, ExpectedError, ConfigDef,
    BindingPattern, MatchPattern, MatchArm,
    MapEntry, FieldInit,
    ArmRange, MapEntryRange, FieldInitRange,
    // function_seq types
    SeqBinding, SeqBindingRange, FunctionSeq,
    // function_exp types
    NamedExpr, NamedExprRange, FunctionExpKind, FunctionExp,
    // CallNamed types
    CallArg, CallArgRange,
    // Import types
    UseDef, UseItem, ImportPath,
    // Generic types
    GenericParam, GenericParamRange, TraitBound, WhereClause,
    // Trait types
    TraitDef, TraitItem, TraitMethodSig, TraitDefaultMethod, TraitAssocType,
    // Impl types
    ImplDef, ImplMethod, ImplAssocType,
    // Extension types
    ExtendDef,
    // Type declaration types
    TypeDecl, TypeDeclKind, StructField, Variant, VariantField,
};
pub use arena::{ExprArena, SharedArena};
