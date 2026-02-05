//! Type system for Ori.
//!
//! Provides the unified type system based on:
//! - [`Idx`]: 32-bit type index (THE canonical type handle)
//! - [`Tag`]: Type kind discriminant
//! - [`Item`]: Compact type storage (tag + data)
//! - [`TypeFlags`]: Pre-computed metadata for O(1) queries
//! - [`Pool`]: Unified type pool with interning
//!
//! Per design spec 02-design-principles.md:
//! - All types have Clone, Eq, Hash for Salsa compatibility
//! - Interned type representations for efficiency
//! - Flat structures for cache locality

mod check;
mod flags;
mod idx;
mod infer;
mod item;
mod output;
mod pool;
mod registry;
mod tag;
mod type_error;
mod unify;

pub use check::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
    ModuleChecker,
};
pub use flags::{TypeCategory, TypeFlags};
pub use idx::Idx;
pub use infer::{check_expr, infer_expr, resolve_parsed_type, ExprIndex, InferEngine, TypeEnv};
pub use item::Item;
pub use output::{FnWhereClause, FunctionSig, TypeCheckResult, TypedModule};
pub use pool::{Pool, VarState, DEFAULT_RANK};
pub use registry::{
    // Method registry
    BuiltinMethod,
    BuiltinMethodKind,
    // Type registry
    FieldDef,
    HigherOrderMethod,
    // Trait registry
    ImplEntry,
    ImplMethodDef,
    MethodLookup,
    MethodRegistry,
    MethodResolution,
    MethodTransform,
    StructDef,
    TraitAssocTypeDef,
    TraitEntry,
    TraitMethodDef,
    TraitRegistry,
    TypeEntry,
    TypeKind,
    TypeRegistry,
    VariantDef,
    VariantFields,
    Visibility,
    WhereConstraint,
};
pub use tag::Tag;
pub use type_error::{
    diff_types, edit_distance, find_closest_field, suggest_field_typo, ArityMismatchKind,
    ContextKind, ErrorContext, Expected, ExpectedOrigin, Replacement, SequenceKind, Severity,
    Suggestion, TypeCheckError, TypeErrorKind, TypeProblem,
};
pub use unify::{ArityKind, Rank, UnifyContext, UnifyEngine, UnifyError};
