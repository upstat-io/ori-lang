//! Thread-safe shared registry wrappers.
//!
//! Provides thread-safe access to registries using `Arc` and `Arc<RwLock>`.

// Arc is the implementation - all usage goes through the newtype
#![expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedRegistry"
)]

use std::fmt;
use std::sync::Arc;

/// Thread-safe shared registry wrapper (immutable).
///
/// Uses `Arc` internally for thread-safe reference counting.
/// The wrapped registry is immutable after creation.
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

/// Thread-safe mutable shared registry wrapper.
///
/// Uses `Arc<RwLock<T>>` internally for interior mutability.
/// Needed when methods must be added after the registry is cached.
///
/// # Salsa Compliance Note
///
/// This type uses `Arc<RwLock<T>>` which technically violates Salsa's
/// preference for immutable query results. We use this pattern for
/// registries that:
///
/// 1. Are built incrementally during evaluation (e.g., user method registry)
/// 2. Must be accessible from multiple interpreter contexts
/// 3. Cannot be fully built before caching (methods discovered during eval)
///
/// The trade-off: we sacrifice Salsa's automatic invalidation tracking
/// for the ability to add entries after initial construction. This is
/// acceptable because:
///
/// - User methods don't change during a single evaluation run
/// - The registry is rebuilt from scratch when the source changes
/// - We manually ensure no stale data persists across queries
///
/// A future improvement could split this into an immutable base registry
/// (populated during type checking) and a runtime method cache.
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
