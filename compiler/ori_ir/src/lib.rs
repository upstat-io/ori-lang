//! Ori IR - Intermediate Representation Types
//!
//! This crate contains the core data structures for the Ori compiler:
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
//! - **Flatten Everything**: No `Box<Expr>`, use `ExprId(u32)` indices
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

mod arena;
pub mod ast;
pub mod builtin_constants;
pub mod builtin_methods;
mod builtin_type;
mod comment;
mod derives;
mod expr_id;
mod inline_list;
mod interner;
mod name;
mod parsed_type;
mod span;
mod token;
mod traits;
mod type_id;
pub mod visitor;

pub use arena::{ExprArena, SharedArena};
pub use ast::{
    ArmRange,
    BinaryOp,
    BindingPattern,
    // CallNamed types
    CallArg,
    CallArgRange,
    // Capability types
    CapabilityRef,
    ConfigDef,
    // Default implementation types
    DefImplDef,
    ExpectedError,
    Expr,
    ExprKind,
    // Extension types
    ExtendDef,
    FieldInit,
    FieldInitRange,
    Function,
    FunctionExp,
    FunctionExpKind,
    FunctionSeq,
    // Generic types
    GenericParam,
    GenericParamRange,
    ImplAssocType,
    // Impl types
    ImplDef,
    ImplMethod,
    ImportPath,
    MapEntry,
    MapEntryRange,
    MatchArm,
    MatchPattern,
    Module,
    // function_exp types
    NamedExpr,
    NamedExprRange,
    Param,
    ParamRange,
    // function_seq types
    SeqBinding,
    SeqBindingRange,
    Stmt,
    StmtKind,
    StructField,
    TestDef,
    TraitAssocType,
    TraitBound,
    // Trait types
    TraitDef,
    TraitDefaultMethod,
    TraitItem,
    TraitMethodSig,
    // Type declaration types
    TypeDecl,
    TypeDeclKind,
    UnaryOp,
    // Import types
    UseDef,
    UseItem,
    Variant,
    VariantField,
    // Visibility
    Visibility,
    WhereClause,
};
pub use builtin_type::BuiltinType;
pub use comment::{Comment, CommentKind, CommentList};
pub use derives::{DerivedMethodInfo, DerivedTrait};
pub use expr_id::{
    ExprId, ExprRange, MatchPatternId, MatchPatternRange, ParsedTypeId, ParsedTypeRange, StmtId,
    StmtRange,
};
pub use inline_list::{ExprList, ExprListIter, INLINE_CAPACITY};
pub use interner::{InternError, SharedInterner, StringInterner, StringLookup};
pub use name::Name;
pub use parsed_type::ParsedType;
pub use span::{Span, SpanError};
pub use token::{DurationUnit, SizeUnit, Token, TokenKind, TokenList};
pub use traits::{Named, Spanned, Typed};
pub use type_id::TypeId;
