//! Compiler context for dependency injection.
//!
//! `CompilerContext` holds shared registries and configuration that can be
//! passed to various compiler components (`TypeChecker`, Evaluator, etc.).
//!
//! This enables:
//! - Dependency injection for testing with mock registries
//! - Sharing registries across compiler phases
//! - Future extensibility without changing component signatures

// Arc is the implementation of SharedMutableRegistry - all usage goes through the newtype
#![expect(clippy::disallowed_types, reason = "Arc is the implementation of SharedMutableRegistry")]

use std::sync::Arc;
use std::fmt;
use ori_patterns::PatternRegistry;
use ori_types::SharedTypeInterner;

// Re-export SharedRegistry from ori_typeck so we have a single source of truth
pub use ori_typeck::SharedRegistry;

// SharedMutableRegistry Newtype

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

// Compiler Context

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
    /// Type interner for efficient type storage and O(1) equality comparison.
    pub type_interner: SharedTypeInterner,
}

impl CompilerContext {
    /// Create a new compiler context with default registries.
    pub fn new() -> Self {
        CompilerContext {
            pattern_registry: SharedRegistry::new(PatternRegistry::new()),
            type_interner: SharedTypeInterner::new(),
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

    /// Create a context with a custom type interner.
    ///
    /// Useful for sharing a type interner across compilation phases.
    #[must_use]
    pub fn with_type_interner(mut self, interner: SharedTypeInterner) -> Self {
        self.type_interner = interner;
        self
    }
}

impl Default for CompilerContext {
    fn default() -> Self {
        Self::new()
    }
}

// Shared Context

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
        let _ = &ctx.pattern_registry;
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

        let _ = &ctx1.pattern_registry;
        let _ = &ctx2.pattern_registry;
    }

    #[test]
    fn test_context_builder() {
        let custom_pattern_registry = PatternRegistry::new();
        let ctx = CompilerContext::new()
            .with_pattern_registry(custom_pattern_registry);

        let _ = &ctx.pattern_registry;
    }

    #[test]
    fn test_shared_context() {
        let ctx = CompilerContext::new();
        let shared = shared_context(ctx);

        let shared2 = shared.clone();
        let _ = &shared.pattern_registry;
        let _ = &shared2.pattern_registry;
    }
}
