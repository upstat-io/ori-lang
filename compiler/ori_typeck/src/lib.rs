//! Ori Typeck - Type checker for the Ori compiler.
//!
//! This crate provides type checking and inference based on Hindley-Milner
//! with extensions for Ori's pattern system.
//!
//! # Main Entry Points
//!
//! - [`TypeChecker`]: The main type checker struct
//! - [`TypeCheckerBuilder`]: Builder for creating `TypeChecker` instances
//!
//! # Module Organization
//!
//! - `checker`: Type checker implementation and components
//! - `infer`: Expression type inference
//! - `registry`: Type and trait registries
//! - `operators`: Binary operator type checking

pub mod checker;
pub mod derives;
pub mod infer;
pub mod operators;
pub mod registry;
mod shared;
mod stack;
pub mod suggest;

pub use shared::SharedRegistry;
pub use stack::ensure_sufficient_stack;

// Re-export registry types
pub use registry::{
    CoherenceError, ImplAssocTypeDef, ImplEntry, ImplMethodDef, MethodLookup, TraitAssocTypeDef,
    TraitEntry, TraitMethodDef, TraitRegistry, TypeEntry, TypeKind, TypeRegistry,
    VariantConstructorInfo, VariantDef,
};

// Re-export operator types
pub use operators::{check_binary_operation, TypeOpError, TypeOpResult};

// Re-export checker types
pub use checker::{
    add_pattern_bindings,
    // Convenience functions
    type_check,
    type_check_with_config,
    type_check_with_source,
    CheckContext,
    DiagnosticState,
    FunctionType,
    GenericBound,
    InferenceState,
    Registries,
    ScopeContext,
    TypeCheckError,
    TypeChecker,
    TypeCheckerBuilder,
    TypedModule,
    WhereConstraint,
};

// Re-export import support types
pub use checker::imports::{ImportedFunction, ImportedGeneric, ImportedModuleAlias};

// Re-export bound checking function
pub use checker::bound_checking::primitive_implements_trait;
