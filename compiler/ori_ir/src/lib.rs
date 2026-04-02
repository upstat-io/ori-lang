#![deny(unsafe_code)]
//! Ori IR - Intermediate Representation Types
//!
//! This crate contains the core data structures for the Ori compiler:
//! - Spans for source locations
//! - Names for interned identifiers
//! - Tokens and `TokenList` for lexer output
//! - AST nodes (Expr, Stmt, Function, etc.)
//! - Arena allocation for expressions
//! - `BuiltinType` for IR-level type identity
//! - `builtin_constants` for constant definitions
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

/// Separator used in monomorphized function names.
///
/// When a generic function is monomorphized, its name is suffixed with this
/// separator followed by the encoded type arguments (e.g., `foo$m$int_str`).
///
/// **Sync point**: Used by `ori_llvm` (monomorphize) and `ori_arc` (`arg_ownership`).
/// Both crates MUST use this constant — never inline `"$m$"`.
pub const MONO_SEPARATOR: &str = "$m$";

mod arena;
pub mod ast;
pub mod builtin_constants;
mod builtin_type;
pub mod canon;
mod comment;
mod derives;
mod expr_id;
pub mod format_spec;
pub mod hash_constants;
pub mod incremental;
mod interner;
mod metadata;
mod name;
mod parsed_type;
mod pattern_resolution;
mod span;
pub mod tag_constants;
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
    // Conditional compilation attribute types
    CfgAttr,
    // Constant definition
    ConstDef,
    // Default implementation types
    DefImplDef,
    ExpectedError,
    Expr,
    ExprKind,
    // Extension types
    ExtendDef,
    ExtensionImport,
    ExtensionImportItem,
    // Extern (FFI) types
    ExternBlock,
    ExternItem,
    ExternParam,
    FieldBinding,
    FieldInit,
    FieldInitRange,
    // File-level attribute
    FileAttr,
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
    ImportErrorKind,
    ImportPath,
    // List with spread types
    ListElement,
    ListElementRange,
    // Map types
    MapElement,
    MapElementRange,
    MapEntry,
    MapEntryRange,
    MatchArm,
    MatchPattern,
    Module,
    Mutability,
    // function_exp types
    NamedExpr,
    NamedExprRange,
    Param,
    ParamRange,
    PostContract,
    // Function contract types
    PreContract,
    // Repr attribute (for #repr("c"), etc.)
    ReprAttrKind,
    Stmt,
    StmtKind,
    StructField,
    StructLitField,
    StructLitFieldRange,
    // Target conditional compilation
    TargetAttr,
    TemplatePart,
    TemplatePartRange,
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
pub use derives::strategy::{CombineOp, DeriveStrategy, FieldOp, FormatOpen, StructBody, SumBody};
pub use derives::{DerivedMethodInfo, DerivedMethodShape, DerivedTrait};
pub use expr_id::{
    BindingPatternId, ExprId, ExprRange, FunctionExpId, FunctionSeqId, MatchPatternId,
    MatchPatternRange, ParsedTypeId, ParsedTypeRange, StmtId, StmtRange,
};
pub use hash_constants::{FNV_OFFSET_BASIS, FNV_PRIME};
pub use interner::{InternError, SharedInterner, StringInterner, StringLookup};
pub use metadata::ModuleExtra;
pub use name::Name;
pub use parsed_type::ParsedType;
pub use pattern_resolution::{PatternKey, PatternResolution};
pub use span::{Span, SpanError};
pub use tag_constants::{
    min_tag_bytes, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN, FIELD_CAP, FIELD_DATA, FIELD_LEN,
    OPTION_TAG_NONE, OPTION_TAG_SOME, OPTION_VARIANT_NONE, OPTION_VARIANT_SOME, RANGE_FIELD_END,
    RANGE_FIELD_START, RESULT_TAG_ERR, RESULT_TAG_OK, RESULT_VARIANT_ERR, RESULT_VARIANT_OK,
};
pub use token::{
    DurationUnit, SizeUnit, Token, TokenCapture, TokenFlags, TokenIdx, TokenKind, TokenList,
    TokenTag,
};
pub use traits::{Named, Spanned, Typed};
pub use type_id::TypeId;
