//! Type checker core implementation.
//!
//! Contains the `TypeChecker` struct and main entry point for type checking.
//!
//! # Module Structure
//!
//! - `types`: Output types (`TypedModule`, `FunctionType`, etc.)
//! - `components`: Component structs for `TypeChecker` organization
//! - `scope_guards`: RAII scope guards for context management
//! - `signatures`: Function signature inference
//! - `pattern_binding`: Pattern to type binding
//! - `cycle_detection`: Closure self-capture detection
//! - `type_registration`: User-defined type registration
//! - `trait_registration`: Trait and impl registration
//! - `bound_checking`: Trait bound verification
//! - `builder`: `TypeChecker` builder pattern
//! - `type_resolution`: Type ID and parsed type to internal type conversion
//! - `function_checking`: Function, test, and impl method body type checking
//! - `orchestration`: Module type checking orchestration
//! - `utilities`: Utility methods (validation, error reporting)
//! - `api`: Public API functions

mod api;
pub mod bound_checking;
mod builder;
pub mod components;
mod cycle_detection;
mod function_checking;
pub mod imports;
mod orchestration;
mod pattern_binding;
mod scope_guards;
mod signatures;
mod trait_registration;
mod type_registration;
mod type_resolution;
pub mod types;
mod utilities;

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;

pub use api::{type_check, type_check_with_config, type_check_with_source};
pub use builder::TypeCheckerBuilder;
pub use components::{CheckContext, DiagnosticState, InferenceState, Registries, ScopeContext};
pub use cycle_detection::add_pattern_bindings;
pub use types::{FunctionType, GenericBound, TypeCheckError, TypedModule, WhereConstraint};

use ori_diagnostic::queue::DiagnosticConfig;
use ori_ir::{ExprArena, StringInterner};

/// Type checker state.
///
/// Organized into logical components for better testability and maintainability:
/// - `context`: Immutable references to arena and interner
/// - `inference`: Mutable inference state (context, environments, expression types)
/// - `registries`: Pattern, type operator, type, and trait registries
/// - `diagnostics`: Error collection and diagnostic queue
/// - `scope`: Function signatures, impl Self type, config types, capabilities
pub struct TypeChecker<'a> {
    /// Immutable references for expression lookup.
    pub(crate) context: CheckContext<'a>,
    /// Mutable inference state.
    pub(crate) inference: InferenceState,
    /// Registry bundle for patterns, types, and traits.
    pub(crate) registries: Registries,
    /// Diagnostic collection state.
    pub(crate) diagnostics: DiagnosticState,
    /// Function and scope context state.
    pub(crate) scope: ScopeContext,
}

impl<'a> TypeChecker<'a> {
    /// Create a new type checker with default registries.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        TypeCheckerBuilder::new(arena, interner).build()
    }

    /// Create a type checker with source code for diagnostic queue features.
    ///
    /// When source is provided, error deduplication and limits are enabled.
    pub fn with_source(arena: &'a ExprArena, interner: &'a StringInterner, source: String) -> Self {
        TypeCheckerBuilder::new(arena, interner)
            .with_source(source)
            .build()
    }

    /// Create a type checker with source and custom diagnostic configuration.
    pub fn with_source_and_config(
        arena: &'a ExprArena,
        interner: &'a StringInterner,
        source: String,
        config: DiagnosticConfig,
    ) -> Self {
        TypeCheckerBuilder::new(arena, interner)
            .with_source(source)
            .with_diagnostic_config(config)
            .build()
    }
}
