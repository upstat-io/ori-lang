//! Salsa-Compatible IR Types
//!
//! Every type in this module has the required traits for Salsa:
//! - Clone: Required for Salsa storage
//! - Eq + PartialEq: Required for early cutoff
//! - Hash: Required for memoization keys
//! - Debug: Required for error messages
//!
//! # Design Philosophy (per 02-design-principles.md)
//!
//! - **Intern Everything**: Strings → Name(u32), Types → TypeId(u32)
//! - **Flatten Everything**: No Box<Expr>, use ExprId(u32) indices
//! - **Interface Segregation**: Focused traits (Spanned, Named)
//!
//! Types that contain floats store them as u64 bits for Hash compatibility.
//! Types that contain strings use interned Name for O(1) equality.

mod span;
mod name;
mod token;
mod interner;
mod type_id;
mod expr_id;
mod traits;
pub mod ast;
mod arena;
pub mod visitor;

pub use span::Span;
pub use name::Name;
pub use token::{Token, TokenKind, TokenList, DurationUnit, SizeUnit};
pub use interner::{StringInterner, SharedInterner};
pub use type_id::TypeId;
pub use expr_id::{ExprId, ExprRange, StmtId, StmtRange};
pub use traits::{Spanned, Named, Typed};
pub use ast::{
    Expr, ExprKind, Stmt, StmtKind, Param, ParamRange,
    BinaryOp, UnaryOp, Function, Module, TestDef,
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
    ImplDef, ImplMethod,
    // Extension types
    ExtendDef,
    // Type declaration types
    TypeDecl, TypeDeclKind, StructField, Variant, VariantField,
};
pub use arena::{ExprArena, SharedArena};
