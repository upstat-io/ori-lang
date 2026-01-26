//! Type checker for Sigil.
//!
//! Implements Hindley-Milner type inference with extensions for
//! Sigil's pattern system.

pub mod components;
pub mod scope_guards;
pub mod types;

pub use components::{
    CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext,
};
pub use scope_guards::{SavedCapabilityContext, SavedImplContext};
pub use types::{
    TypedModule, FunctionType, GenericBound, WhereConstraint, TypeCheckError,
};
