//! Builder pattern for `TypeChecker` construction.
//!
//! Eliminates constructor duplication by centralizing field initialization
//! in a single `build()` method.

use ori_diagnostic::queue::{DiagnosticConfig, DiagnosticQueue};
use ori_ir::{ExprArena, StringInterner};
use ori_patterns::PatternRegistry;
use ori_types::SharedTypeInterner;

use super::components::{
    CheckContext, DiagnosticState, InferenceState, Registries, ScopeContext,
};
use super::TypeChecker;
use crate::shared::SharedRegistry;

/// Builder for creating `TypeChecker` instances with various configurations.
pub struct TypeCheckerBuilder<'a> {
    arena: &'a ExprArena,
    interner: &'a StringInterner,
    source: Option<String>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    diagnostic_config: Option<DiagnosticConfig>,
    type_interner: Option<SharedTypeInterner>,
}

impl<'a> TypeCheckerBuilder<'a> {
    /// Create a new builder with required references.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        Self {
            arena,
            interner,
            source: None,
            registry: None,
            diagnostic_config: None,
            type_interner: None,
        }
    }

    /// Set source code for diagnostic queue features.
    ///
    /// When source is provided, error deduplication and limits are enabled.
    #[must_use]
    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    /// Set a custom pattern registry.
    #[must_use]
    pub fn with_pattern_registry(mut self, registry: SharedRegistry<PatternRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Set custom diagnostic configuration.
    #[must_use]
    pub fn with_diagnostic_config(mut self, config: DiagnosticConfig) -> Self {
        self.diagnostic_config = Some(config);
        self
    }

    /// Set a shared type interner for `TypeId` interning.
    ///
    /// Use this when you need to share the type interner with other code
    /// (e.g., for tests that need to verify `TypeId` values).
    #[must_use]
    pub fn with_type_interner(mut self, interner: SharedTypeInterner) -> Self {
        self.type_interner = Some(interner);
        self
    }

    /// Build the `TypeChecker` with the configured options.
    pub fn build(self) -> TypeChecker<'a> {
        // Build context component
        let context = CheckContext::new(self.arena, self.interner);

        // Build inference component
        let inference = if let Some(type_interner) = self.type_interner {
            InferenceState::with_type_interner(type_interner)
        } else {
            InferenceState::new()
        };

        // Build registries component
        let registries = if let Some(registry) = self.registry {
            Registries::with_pattern_registry(registry)
        } else {
            Registries::new()
        };

        // Build diagnostics component
        let diagnostics = if let Some(source) = self.source {
            let queue = if let Some(config) = self.diagnostic_config {
                DiagnosticQueue::with_config(config)
            } else {
                DiagnosticQueue::new()
            };
            DiagnosticState::with_source(source, queue)
        } else {
            DiagnosticState::new()
        };

        // Build scope component
        let scope = ScopeContext::new();

        TypeChecker {
            context,
            inference,
            registries,
            diagnostics,
            scope,
        }
    }
}
