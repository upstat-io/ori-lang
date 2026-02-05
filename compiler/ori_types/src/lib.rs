//! Type system for Ori.
//!
//! Per design spec 02-design-principlesmd:
//! - All types have Clone, Eq, Hash for Salsa compatibility
//! - Interned type representations for efficiency
//! - Flat structures for cache locality
//!
//! # Types V2 (NEW)
//!
//! The new unified type system provides:
//! - [`Idx`]: 32-bit type index (THE canonical type handle)
//! - [`Tag`]: Type kind discriminant
//! - [`Item`]: Compact type storage (tag + data)
//! - [`TypeFlags`]: Pre-computed metadata for O(1) queries
//! - [`Pool`]: Unified type pool with interning
//!
//! # Legacy Type System
//!
//! The old type system is still available during migration:
//! - `Type`: The traditional boxed representation
//! - `TypeData`/`TypeId`: The interned representation
//!
//! Use `TypeInterner` to intern types and get `TypeId` handles.

// === Types V2 (New Unified System) ===
mod flags;
mod idx;
mod infer;
mod item;
mod pool;
mod registry;
mod tag;
mod type_error;
mod unify;

// Re-export new types
pub use flags::{TypeCategory, TypeFlags};
pub use idx::Idx;
pub use infer::{infer_expr, ExprIndex, InferEngine, TypeEnvV2};
pub use item::Item;
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

// === Legacy Type System ===
mod context;
mod core;
mod data;
mod env;
mod error;
mod traverse;
mod type_interner;

// Re-export all public types
pub use context::{InferenceContext, TypeContext};
pub use core::{Type, TypeScheme, TypeSchemeId};
pub use env::TypeEnv;
pub use error::TypeError;
pub use traverse::{TypeFolder, TypeIdFolder, TypeIdVisitor, TypeVisitor};

// Type interning exports
pub use data::{TypeData, TypeVar};
pub use type_interner::{SharedTypeInterner, TypeInternError, TypeInterner};

// Size assertions to prevent accidental regressions.
// Type is used throughout type checking and stored in query results.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{Type, TypeVar};
    // Type enum: largest variant is Applied with Name (8) + Vec<Type> (24) = 32 bytes + discriminant = 40 bytes
    // Applied variant has: name: Name (u64 = 8) + args: Vec<Type> (24) = 32 bytes
    ori_ir::static_assert_size!(Type, 40);
    // TypeVar is just a u32 wrapper
    ori_ir::static_assert_size!(TypeVar, 4);
}

// Tests extracted to: compiler/oric/tests/phases/typeck/types.rs
