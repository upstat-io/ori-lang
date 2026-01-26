//! Sigil Typeck - Type checker for the Sigil compiler.
//!
//! This crate provides type checking and inference.

mod stack;
mod shared;
pub mod checker;
pub mod operators;
pub mod registry;

pub use stack::ensure_sufficient_stack;
pub use shared::SharedRegistry;
pub use registry::{
    TypeRegistry, TypeEntry, TypeKind, VariantDef,
    TraitRegistry, TraitEntry, TraitMethodDef, TraitAssocTypeDef,
    ImplEntry, ImplMethodDef, ImplAssocTypeDef, MethodLookup, CoherenceError,
};
pub use operators::{
    TypeOperator, TypeOpResult, TypeOpError, TypeOperatorRegistry,
    ArithmeticTypeOp, ComparisonTypeOp, LogicalTypeOp, BitwiseTypeOp,
    RangeTypeOp, CoalesceTypeOp,
};
pub use checker::{
    CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext,
    SavedCapabilityContext, SavedImplContext,
    TypedModule, FunctionType, GenericBound, WhereConstraint, TypeCheckError,
};
