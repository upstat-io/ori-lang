//! Builder pattern for TypeChecker construction.
//!
//! Eliminates constructor duplication by centralizing field initialization
//! in a single `build()` method.

use crate::context::{CompilerContext, SharedRegistry};
use crate::diagnostic::queue::{DiagnosticConfig, DiagnosticQueue};
use crate::ir::{ExprArena, StringInterner};
use sigil_patterns::PatternRegistry;

use super::components::{
    CheckContext, DiagnosticState, InferenceState, Registries, ScopeContext,
};
use super::TypeChecker;

/// Builder for creating TypeChecker instances with various configurations.
pub struct TypeCheckerBuilder<'a> {
    arena: &'a ExprArena,
    interner: &'a StringInterner,
    source: Option<String>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    diagnostic_config: Option<DiagnosticConfig>,
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

    /// Set a custom compiler context.
    ///
    /// This enables dependency injection for testing with mock registries.
    /// Clones the pattern registry from the context.
    #[must_use]
    pub fn with_context(mut self, context: &CompilerContext) -> Self {
        self.registry = Some(context.pattern_registry.clone());
        self
    }

    /// Set custom diagnostic configuration.
    #[must_use]
    pub fn with_diagnostic_config(mut self, config: DiagnosticConfig) -> Self {
        self.diagnostic_config = Some(config);
        self
    }

    /// Build the TypeChecker with the configured options.
    pub fn build(self) -> TypeChecker<'a> {
        // Build context component
        let context = CheckContext::new(self.arena, self.interner);

        // Build inference component
        let inference = InferenceState::new();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ExprArena, StringInterner};

    #[test]
    fn test_builder_default_construction() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();

        let checker = TypeCheckerBuilder::new(&arena, &interner).build();

        assert!(checker.diagnostics.errors.is_empty());
        assert!(checker.diagnostics.source.is_none());
        assert!(checker.diagnostics.queue.is_none());
    }

    #[test]
    fn test_builder_with_source() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();

        let checker = TypeCheckerBuilder::new(&arena, &interner)
            .with_source("let x = 1".to_string())
            .build();

        assert!(checker.diagnostics.source.is_some());
        assert!(checker.diagnostics.queue.is_some());
    }

    #[test]
    fn test_builder_with_diagnostic_config() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let config = DiagnosticConfig {
            error_limit: 5,
            filter_follow_on: true,
            deduplicate: false,
        };

        let checker = TypeCheckerBuilder::new(&arena, &interner)
            .with_source("let x = 1".to_string())
            .with_diagnostic_config(config)
            .build();

        assert!(checker.diagnostics.source.is_some());
        assert!(checker.diagnostics.queue.is_some());
    }

    #[test]
    fn test_builder_with_context() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let context = CompilerContext::new();

        // Just verify that building with context succeeds
        let checker = TypeCheckerBuilder::new(&arena, &interner)
            .with_context(&context)
            .build();

        assert!(checker.diagnostics.errors.is_empty());
    }

    #[test]
    fn test_builder_combined_options() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let context = CompilerContext::new();
        let config = DiagnosticConfig {
            error_limit: 10,
            filter_follow_on: true,
            deduplicate: true,
        };

        let checker = TypeCheckerBuilder::new(&arena, &interner)
            .with_source("test".to_string())
            .with_context(&context)
            .with_diagnostic_config(config)
            .build();

        assert!(checker.diagnostics.source.is_some());
        assert!(checker.diagnostics.queue.is_some());
        assert!(checker.diagnostics.errors.is_empty());
    }
}
