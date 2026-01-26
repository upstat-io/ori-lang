//! Type checking re-exports from sigil_typeck.
//!
//! This module provides the type checking infrastructure for sigilc.
//! The core implementation lives in the `sigil_typeck` crate.

// Re-export all public types from sigil_typeck
pub use sigil_typeck::{
    // Main type checker
    TypeChecker, TypeCheckerBuilder,
    // Components
    CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext,
    SavedCapabilityContext, SavedImplContext,
    // Output types
    TypedModule, FunctionType, GenericBound, WhereConstraint, TypeCheckError,
    // Convenience functions
    type_check, type_check_with_source, type_check_with_config,
    // Utility
    add_pattern_bindings, ensure_sufficient_stack,
    primitive_implements_trait,
    SharedRegistry,
};

// Registry re-exports (also available as sigil_typeck::registry::*)
pub mod type_registry {
    pub use sigil_typeck::registry::*;
}

pub use sigil_typeck::registry::{
    TypeRegistry, TypeEntry, TypeKind, VariantDef,
    TraitRegistry, TraitEntry, TraitMethodDef, TraitAssocTypeDef,
    ImplEntry, ImplMethodDef, ImplAssocTypeDef, MethodLookup, CoherenceError,
};

// Operator re-exports
pub mod operators {
    pub use sigil_typeck::operators::*;
}

// Derives re-exports
pub mod derives {
    pub use sigil_typeck::derives::*;
}

// Inference re-exports
pub mod infer {
    pub use sigil_typeck::infer::*;
}

// Re-export DiagnosticConfig from sigil_diagnostic (for type_check_with_config)
pub use sigil_diagnostic::queue::DiagnosticConfig;

use crate::context::CompilerContext;
use crate::ir::StringInterner;
use crate::parser::ParseResult;

/// Type check a parsed module with a custom compiler context.
///
/// This allows dependency injection of custom registries for testing.
/// This function is specific to sigilc since it uses CompilerContext.
pub fn type_check_with_context(
    parse_result: &ParseResult,
    interner: &StringInterner,
    context: &CompilerContext,
) -> TypedModule {
    TypeCheckerBuilder::new(&parse_result.arena, interner)
        .with_pattern_registry(context.pattern_registry.clone())
        .build()
        .check_module(&parse_result.module)
}
