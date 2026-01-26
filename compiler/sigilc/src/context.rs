//! Compiler context for dependency injection.
//!
//! `CompilerContext` holds shared registries and configuration that can be
//! passed to various compiler components (`TypeChecker`, Evaluator, etc.).
//!
//! This enables:
//! - Dependency injection for testing with mock registries
//! - Sharing registries across compiler phases
//! - Future extensibility without changing component signatures

// Arc is the implementation of SharedRegistry - all usage goes through the newtype
#![expect(clippy::disallowed_types, reason = "Arc is the implementation of SharedRegistry")]

use std::sync::Arc;
use std::fmt;
use sigil_patterns::PatternRegistry;
use crate::eval::{OperatorRegistry, MethodRegistry, UnaryOperatorRegistry};

// =============================================================================
// SharedRegistry Newtype
// =============================================================================

/// Thread-safe shared registry wrapper.
///
/// This newtype enforces that all registry sharing goes through this type,
/// preventing accidental direct `Arc<Registry>` usage.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Usage
/// ```ignore
/// let registry = SharedRegistry::new(PatternRegistry::new());
/// // Access via Deref
/// let pattern = registry.get("map");
/// ```
pub struct SharedRegistry<T>(Arc<T>);

impl<T> SharedRegistry<T> {
    /// Create a new shared registry from an owned registry.
    pub fn new(registry: T) -> Self {
        SharedRegistry(Arc::new(registry))
    }
}

impl<T> Clone for SharedRegistry<T> {
    fn clone(&self) -> Self {
        SharedRegistry(Arc::clone(&self.0))
    }
}

impl<T> std::ops::Deref for SharedRegistry<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: fmt::Debug> fmt::Debug for SharedRegistry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedRegistry({:?})", &*self.0)
    }
}

// =============================================================================
// SharedMutableRegistry Newtype
// =============================================================================

/// Thread-safe mutable shared registry wrapper.
///
/// Unlike `SharedRegistry`, this type uses `RwLock` for interior mutability,
/// allowing modifications through shared references. This is needed when:
/// - The registry is cached (e.g., in a `MethodDispatcher`)
/// - Methods need to be added after the cache is created
///
/// # Thread Safety
/// Uses `Arc<RwLock<T>>` internally for thread-safe mutable access.
///
/// # Usage
/// ```ignore
/// let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
/// // Read access
/// registry.read().lookup("Point", "distance");
/// // Write access
/// registry.write().register("Point".into(), "distance".into(), method);
/// ```
pub struct SharedMutableRegistry<T>(Arc<parking_lot::RwLock<T>>);

impl<T> SharedMutableRegistry<T> {
    /// Create a new shared mutable registry from an owned registry.
    pub fn new(registry: T) -> Self {
        SharedMutableRegistry(Arc::new(parking_lot::RwLock::new(registry)))
    }

    /// Get read access to the registry.
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.0.read()
    }

    /// Get write access to the registry.
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.0.write()
    }
}

impl<T> Clone for SharedMutableRegistry<T> {
    fn clone(&self) -> Self {
        SharedMutableRegistry(Arc::clone(&self.0))
    }
}

impl<T: fmt::Debug> fmt::Debug for SharedMutableRegistry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedMutableRegistry({:?})", &*self.0.read())
    }
}

// =============================================================================
// Compiler Context
// =============================================================================

/// Shared compiler context containing registries and configuration.
///
/// This struct is designed for dependency injection, allowing components
/// like `TypeChecker` and Evaluator to receive pre-configured registries
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
    /// Pattern registry for `function_exp` patterns (map, filter, fold, etc.).
    pub pattern_registry: SharedRegistry<PatternRegistry>,
    /// Binary operator registry for arithmetic, comparison, etc.
    pub operator_registry: SharedRegistry<OperatorRegistry>,
    /// Method registry for method dispatch.
    pub method_registry: SharedRegistry<MethodRegistry>,
    /// Unary operator registry for negation, not, etc.
    pub unary_operator_registry: SharedRegistry<UnaryOperatorRegistry>,
}

impl CompilerContext {
    /// Create a new compiler context with default registries.
    pub fn new() -> Self {
        CompilerContext {
            pattern_registry: SharedRegistry::new(PatternRegistry::new()),
            operator_registry: SharedRegistry::new(OperatorRegistry::new()),
            method_registry: SharedRegistry::new(MethodRegistry::new()),
            unary_operator_registry: SharedRegistry::new(UnaryOperatorRegistry::new()),
        }
    }

    /// Create a context with a custom pattern registry.
    ///
    /// Useful for testing with mock patterns.
    #[must_use]
    pub fn with_pattern_registry(mut self, registry: PatternRegistry) -> Self {
        self.pattern_registry = SharedRegistry::new(registry);
        self
    }

    /// Create a context with a custom operator registry.
    ///
    /// Useful for testing with mock operators.
    #[must_use]
    pub fn with_operator_registry(mut self, registry: OperatorRegistry) -> Self {
        self.operator_registry = SharedRegistry::new(registry);
        self
    }

    /// Create a context with a custom method registry.
    ///
    /// Useful for testing with mock methods.
    #[must_use]
    pub fn with_method_registry(mut self, registry: MethodRegistry) -> Self {
        self.method_registry = SharedRegistry::new(registry);
        self
    }

    /// Create a context with a custom unary operator registry.
    ///
    /// Useful for testing with mock unary operators.
    #[must_use]
    pub fn with_unary_operator_registry(mut self, registry: UnaryOperatorRegistry) -> Self {
        self.unary_operator_registry = SharedRegistry::new(registry);
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
        // Verify all registries are present - just check they exist
        let _ = &ctx.pattern_registry;
        let _ = &ctx.operator_registry;
        let _ = &ctx.method_registry;
        let _ = &ctx.unary_operator_registry;
    }

    #[test]
    fn test_context_default() {
        let ctx = CompilerContext::default();
        let _ = &ctx.pattern_registry;
    }

    #[test]
    fn test_context_clone() {
        let ctx1 = CompilerContext::new();
        let ctx2 = ctx1.clone();

        // Both contexts should have pattern registries
        let _ = &ctx1.pattern_registry;
        let _ = &ctx2.pattern_registry;
    }

    #[test]
    fn test_context_builder() {
        // Test the builder pattern for custom registries
        let custom_pattern_registry = PatternRegistry::new();
        let ctx = CompilerContext::new()
            .with_pattern_registry(custom_pattern_registry);

        // Should have a registry
        let _ = &ctx.pattern_registry;
    }

    #[test]
    fn test_shared_context() {
        let ctx = CompilerContext::new();
        let shared = shared_context(ctx);

        // Clone the shared context
        let shared2 = shared.clone();
        // Both should have pattern registries
        let _ = &shared.pattern_registry;
        let _ = &shared2.pattern_registry;
    }
}
