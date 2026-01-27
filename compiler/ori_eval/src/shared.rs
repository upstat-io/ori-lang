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
