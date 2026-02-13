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
mod lifetime;
mod output;
mod pool;
mod registry;
mod tag;
mod type_error;
mod unify;
mod value_category;

pub use check::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
    ModuleChecker,
};
pub use flags::{TypeCategory, TypeFlags};
pub use idx::Idx;
pub use infer::{check_expr, infer_expr, resolve_parsed_type, ExprIndex, InferEngine, TypeEnv};
pub use item::Item;
pub use lifetime::LifetimeId;
pub use ori_ir::{PatternKey, PatternResolution};
pub use output::{EffectClass, FnWhereClause, FunctionSig, TypeCheckResult, TypedModule};
pub use pool::{EnumVariant, Pool, VarState, DEFAULT_RANK};
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
    ContextKind, ErrorContext, Expected, ExpectedOrigin, ImportErrorKind, SequenceKind, Severity,
    TypeCheckError, TypeErrorKind, TypeProblem,
};
pub use unify::{ArityKind, Rank, UnifyContext, UnifyEngine, UnifyError};
pub use value_category::ValueCategory;

// =============================================================================
// Compile-time Salsa compatibility assertions
// =============================================================================
//
// Salsa query results must implement Clone + Eq + PartialEq + Hash + Debug.
// These static assertions catch missing derives at compile time rather than
// runtime. If a type stops deriving a required trait, the build fails here
// with a clear error.

/// Compile-time assertion that `T` implements all Salsa-required traits.
///
/// Evaluates to 0 if the bounds are satisfied. Produces a compile error otherwise.
/// The type parameter is intentionally unused in the body — only the bounds matter.
#[allow(
    clippy::extra_unused_type_parameters,
    reason = "type param exists only for trait bound checking"
)]
const fn assert_salsa_compatible<T: Clone + Eq + std::hash::Hash + std::fmt::Debug>() -> usize {
    0
}

// Core type handles
const _: usize = assert_salsa_compatible::<Idx>();
const _: usize = assert_salsa_compatible::<Tag>();
const _: usize = assert_salsa_compatible::<TypeFlags>();
const _: usize = assert_salsa_compatible::<Rank>();
const _: usize = assert_salsa_compatible::<LifetimeId>();
const _: usize = assert_salsa_compatible::<ValueCategory>();

// Output types (Salsa query results)
const _: usize = assert_salsa_compatible::<TypeCheckResult>();
const _: usize = assert_salsa_compatible::<TypedModule>();
const _: usize = assert_salsa_compatible::<FunctionSig>();
const _: usize = assert_salsa_compatible::<FnWhereClause>();
const _: usize = assert_salsa_compatible::<TypeEntry>();

// Error types (embedded in query results)
const _: usize = assert_salsa_compatible::<TypeCheckError>();
const _: usize = assert_salsa_compatible::<TypeErrorKind>();
const _: usize = assert_salsa_compatible::<ErrorContext>();
const _: usize = assert_salsa_compatible::<ArityMismatchKind>();
const _: usize = assert_salsa_compatible::<TypeProblem>();
const _: usize = assert_salsa_compatible::<ContextKind>();
const _: usize = assert_salsa_compatible::<Expected>();
const _: usize = assert_salsa_compatible::<ExpectedOrigin>();
const _: usize = assert_salsa_compatible::<SequenceKind>();

// Unification types (used in error reporting)
const _: usize = assert_salsa_compatible::<UnifyError>();
const _: usize = assert_salsa_compatible::<UnifyContext>();
const _: usize = assert_salsa_compatible::<ArityKind>();
