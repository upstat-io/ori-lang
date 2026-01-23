//! Compiler context for dependency injection.
//!
//! CompilerContext holds shared registries and configuration that can be
//! passed to various compiler components (TypeChecker, Evaluator, etc.).
//!
//! This enables:
//! - Dependency injection for testing with mock registries
//! - Sharing registries across compiler phases
//! - Future extensibility without changing component signatures

use std::sync::Arc;
use crate::patterns::PatternRegistry;
use crate::eval::{OperatorRegistry, MethodRegistry, UnaryOperatorRegistry};

// =============================================================================
// Compiler Context
// =============================================================================

/// Shared compiler context containing registries and configuration.
///
/// This struct is designed for dependency injection, allowing components
/// like TypeChecker and Evaluator to receive pre-configured registries
/// rather than creating their own.
///
/// # Thread Safety
///
/// Uses `Arc` for registries that need to be shared across threads.
/// Individual registries use internal synchronization if needed.
///
/// # Testing
///
/// Create a custom context with mock registries:
/// ```ignore
/// let ctx = CompilerContext::new()
///     .with_pattern_registry(mock_pattern_registry);
/// let checker = TypeChecker::with_context(&arena, &interner, &ctx);
/// ```
#[derive(Clone)]
pub struct CompilerContext {
    /// Pattern registry for function_exp patterns (map, filter, fold, etc.).
    pub pattern_registry: Arc<PatternRegistry>,
    /// Binary operator registry for arithmetic, comparison, etc.
    pub operator_registry: Arc<OperatorRegistry>,
    /// Method registry for method dispatch.
    pub method_registry: Arc<MethodRegistry>,
    /// Unary operator registry for negation, not, etc.
    pub unary_operator_registry: Arc<UnaryOperatorRegistry>,
}

impl CompilerContext {
    /// Create a new compiler context with default registries.
    pub fn new() -> Self {
        CompilerContext {
            pattern_registry: Arc::new(PatternRegistry::new()),
            operator_registry: Arc::new(OperatorRegistry::new()),
            method_registry: Arc::new(MethodRegistry::new()),
            unary_operator_registry: Arc::new(UnaryOperatorRegistry::new()),
        }
    }

    /// Create a context with a custom pattern registry.
    ///
    /// Useful for testing with mock patterns.
    pub fn with_pattern_registry(mut self, registry: PatternRegistry) -> Self {
        self.pattern_registry = Arc::new(registry);
        self
    }

    /// Create a context with a custom operator registry.
    ///
    /// Useful for testing with mock operators.
    pub fn with_operator_registry(mut self, registry: OperatorRegistry) -> Self {
        self.operator_registry = Arc::new(registry);
        self
    }

    /// Create a context with a custom method registry.
    ///
    /// Useful for testing with mock methods.
    pub fn with_method_registry(mut self, registry: MethodRegistry) -> Self {
        self.method_registry = Arc::new(registry);
        self
    }

    /// Create a context with a custom unary operator registry.
    ///
    /// Useful for testing with mock unary operators.
    pub fn with_unary_operator_registry(mut self, registry: UnaryOperatorRegistry) -> Self {
        self.unary_operator_registry = Arc::new(registry);
        self
    }
}

impl Default for CompilerContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Shared Context
// =============================================================================

/// Thread-safe shared context reference.
///
/// This type alias makes it clear when a context is being shared.
pub type SharedContext = Arc<CompilerContext>;

/// Create a shared context from an owned context.
pub fn shared_context(ctx: CompilerContext) -> SharedContext {
    Arc::new(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = CompilerContext::new();
        // Verify all registries are present
        assert!(Arc::strong_count(&ctx.pattern_registry) == 1);
        assert!(Arc::strong_count(&ctx.operator_registry) == 1);
        assert!(Arc::strong_count(&ctx.method_registry) == 1);
        assert!(Arc::strong_count(&ctx.unary_operator_registry) == 1);
    }

    #[test]
    fn test_context_default() {
        let ctx = CompilerContext::default();
        assert!(Arc::strong_count(&ctx.pattern_registry) == 1);
    }

    #[test]
    fn test_context_clone() {
        let ctx1 = CompilerContext::new();
        let ctx2 = ctx1.clone();

        // Cloning shares the Arc references
        assert!(Arc::strong_count(&ctx1.pattern_registry) == 2);
        assert!(Arc::ptr_eq(&ctx1.pattern_registry, &ctx2.pattern_registry));
    }

    #[test]
    fn test_context_builder() {
        // Test the builder pattern for custom registries
        let custom_pattern_registry = PatternRegistry::new();
        let ctx = CompilerContext::new()
            .with_pattern_registry(custom_pattern_registry);

        // Should have a new registry (not the original)
        assert!(Arc::strong_count(&ctx.pattern_registry) == 1);
    }

    #[test]
    fn test_shared_context() {
        let ctx = CompilerContext::new();
        let shared = shared_context(ctx);

        assert!(Arc::strong_count(&shared) == 1);

        // Clone the shared context
        let shared2 = shared.clone();
        assert!(Arc::strong_count(&shared) == 2);
        assert!(Arc::ptr_eq(&shared.pattern_registry, &shared2.pattern_registry));
    }
}
