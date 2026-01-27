//! Type checking re-exports from ori_typeck.
//!
//! This module provides the type checking infrastructure for oric.
//! The core implementation lives in the `ori_typeck` crate.

// Re-export all public types from ori_typeck
pub use ori_typeck::{
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

// Registry re-exports (also available as ori_typeck::registry::*)
pub mod type_registry {
    pub use ori_typeck::registry::*;
}

pub use ori_typeck::registry::{
    TypeRegistry, TypeEntry, TypeKind, VariantDef,
    TraitRegistry, TraitEntry, TraitMethodDef, TraitAssocTypeDef,
    ImplEntry, ImplMethodDef, ImplAssocTypeDef, MethodLookup, CoherenceError,
};

// Operator re-exports
pub mod operators {
    pub use ori_typeck::operators::*;
}

// Derives re-exports
pub mod derives {
    pub use ori_typeck::derives::*;
}

// Inference re-exports
pub mod infer {
    pub use ori_typeck::infer::*;
}

// Re-export DiagnosticConfig from ori_diagnostic (for type_check_with_config)
pub use ori_diagnostic::queue::DiagnosticConfig;

use crate::context::CompilerContext;
use crate::ir::StringInterner;
use crate::parser::ParseResult;

/// Type check a parsed module with a custom compiler context.
///
/// This allows dependency injection of custom registries for testing.
/// This function is specific to oric since it uses CompilerContext.
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
