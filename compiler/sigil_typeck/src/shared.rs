//! Thread-safe shared registry wrappers.
//!
//! `SharedRegistry<T>` provides thread-safe access to registries using `Arc`.
//! This enforces a pattern where registries are fully built before sharing.

// Arc is the implementation of SharedRegistry - all usage goes through the newtype
#![expect(clippy::disallowed_types, reason = "Arc is the implementation of SharedRegistry")]

use std::sync::Arc;
use std::fmt;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::TypeRegistry;

    #[test]
    fn test_shared_registry_creation() {
        let registry = SharedRegistry::new(TypeRegistry::new());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_shared_registry_clone() {
        let registry1 = SharedRegistry::new(TypeRegistry::new());
        let registry2 = registry1.clone();
        assert_eq!(registry1.len(), registry2.len());
    }

    #[test]
    fn test_shared_registry_deref() {
        let registry = SharedRegistry::new(TypeRegistry::new());
        // Access via deref
        let len = registry.len();
        assert_eq!(len, 0);
    }

    #[test]
    fn test_shared_registry_debug() {
        let registry = SharedRegistry::new(TypeRegistry::new());
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("SharedRegistry"));
    }
}
