//! Semantic registries for types, traits, implementations, and methods.
//!
//! Registries index pooled [`Idx`](crate::Idx) values by names and definitions,
//! while the pool owns structural type representations.

pub mod burden;
pub mod burden_compose;
pub mod burden_dedup;
mod methods;
mod traits;
mod types;

pub use types::{
    FieldDef, StructDef, TypeEntry, TypeKind, TypeRegistry, VariantDef, VariantFields, Visibility,
};

pub use traits::{
    BoundChainLookup, GenericParamMeta, ImplEntry, ImplMethodDef, ImplSpecificity, MethodLookup,
    MethodLookupResult, ObjectSafetyViolation, TraitAssocTypeDef, TraitEntry, TraitMethodDef,
    TraitRegistry, WhereConstraint,
};
pub(crate) use traits::{ImportedMethodOrigin, RegisteredImplOrigin};

pub use methods::MethodRegistry;
