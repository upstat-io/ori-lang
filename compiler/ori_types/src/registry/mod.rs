//! Type, trait, and method registries.
//!
//! Registries provide lookup tables for user-defined types, traits, and methods.
//! Unlike the Pool (which stores all type representations), registries store
//! semantic information about type definitions.
//!
//! # Architecture
//!
//! ```text
//! Pool (types as Idx)
//!     └── TypeRegistry (user-defined types)
//!     └── TraitRegistry (traits and implementations)
//!     └── MethodRegistry (unified method lookup)
//! ```
//!
//! # Design Decisions
//!
//! - Registries use `Idx` (not legacy `TypeId`)
//! - Dual indexing: `BTreeMap<Name, _>` (sorted) + `FxHashMap<Idx, _>` (fast)
//! - Secondary indices for O(1) variant and field lookup
//! - All types derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility

mod methods;
mod traits;
mod types;

// Type registry exports
pub use types::{
    FieldDef, StructDef, TypeEntry, TypeKind, TypeRegistry, VariantDef, VariantFields, Visibility,
};

// Trait registry exports
pub use traits::{
    ImplEntry, ImplMethodDef, MethodLookup, TraitAssocTypeDef, TraitEntry, TraitMethodDef,
    TraitRegistry, WhereConstraint,
};

// Method registry exports
pub use methods::{
    BuiltinMethod, BuiltinMethodKind, HigherOrderMethod, MethodRegistry, MethodResolution,
    MethodTransform,
};
