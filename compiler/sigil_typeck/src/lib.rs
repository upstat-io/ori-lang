//! Sigil Typeck - Type checker for the Sigil compiler.
//!
//! This crate provides type checking and inference based on Hindley-Milner
//! with extensions for Sigil's pattern system.
//!
//! # Main Entry Points
//!
//! - [`TypeChecker`]: The main type checker struct
//! - [`TypeCheckerBuilder`]: Builder for creating TypeChecker instances
//!
//! # Module Organization
//!
//! - `checker`: Type checker implementation and components
//! - `infer`: Expression type inference
//! - `registry`: Type and trait registries
//! - `operators`: Binary operator type checking

mod stack;
mod shared;
pub mod checker;
pub mod derives;
pub mod infer;
pub mod operators;
pub mod registry;

pub use stack::ensure_sufficient_stack;
pub use shared::SharedRegistry;

// Re-export registry types
pub use registry::{
    TypeRegistry, TypeEntry, TypeKind, VariantDef,
    TraitRegistry, TraitEntry, TraitMethodDef, TraitAssocTypeDef,
    ImplEntry, ImplMethodDef, ImplAssocTypeDef, MethodLookup, CoherenceError,
};

// Re-export operator types
pub use operators::{
    TypeOperator, TypeOpResult, TypeOpError, TypeOperatorRegistry,
    ArithmeticTypeOp, ComparisonTypeOp, LogicalTypeOp, BitwiseTypeOp,
    RangeTypeOp, CoalesceTypeOp,
};

// Re-export checker types
pub use checker::{
    TypeChecker, TypeCheckerBuilder,
    CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext,
    SavedCapabilityContext, SavedImplContext,
    TypedModule, FunctionType, GenericBound, WhereConstraint, TypeCheckError,
    add_pattern_bindings,
    // Convenience functions
    type_check, type_check_with_source, type_check_with_config,
};

// Re-export bound checking function
pub use checker::bound_checking::primitive_implements_trait;
