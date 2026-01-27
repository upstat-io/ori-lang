//! Heap wrapper for enforced Arc usage.
//!
//! The `Heap<T>` type wraps `Arc<T>` and provides the ONLY way to allocate
//! heap values in the Value system. External code cannot call `Heap::new()`
//! directly since the constructor is `pub(super)` (visible only within the
//! value module).
//!
//! This ensures that all heap allocations go through Value's factory methods,
//! providing a single point of control for memory allocation.

// Arc is the intentional implementation detail of Heap<T>
#![expect(clippy::disallowed_types, reason = "Arc is the whole point of Heap<T>")]

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// A heap-allocated value wrapper.
///
/// This type enforces that all heap allocations in the Value system go through
/// factory methods on Value. The `new` constructor is private to the value module,
/// so external code must use `Value::string()`, `Value::list()`, etc.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Zero-Cost Abstraction
/// The `#[repr(transparent)]` attribute ensures this has the same memory layout
/// as `Arc<T>`, so there's no overhead from the wrapper.
#[repr(transparent)]
pub struct Heap<T: ?Sized>(Arc<T>);

impl<T> Heap<T> {
    /// Create a new heap-allocated value.
    ///
    /// This is `pub(super)` - only visible within the value module.
    /// External code must use Value's factory methods.
    #[inline]
    pub(super) fn new(value: T) -> Self {
        Heap(Arc::new(value))
    }

    /// Get the inner Arc reference.
    ///
    /// This is useful for cases where you need to pass the Arc to code
    /// that expects an Arc (e.g., for cloning into other data structures).
    #[inline]
    pub fn inner(&self) -> &Arc<T> {
        &self.0
    }
}

impl<T: ?Sized> Deref for Heap<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> Clone for Heap<T> {
    #[inline]
    fn clone(&self) -> Self {
        Heap(Arc::clone(&self.0))
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Heap<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl<T: ?Sized + Eq> Eq for Heap<T> {}

impl<T: ?Sized + Hash> Hash for Heap<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self.0).hash(state);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Heap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Heap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: ?Sized> AsRef<T> for Heap<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized + PartialOrd> PartialOrd for Heap<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<T: ?Sized + Ord> Ord for Heap<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        (**self).cmp(&**other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heap_deref() {
        let h = Heap::new(42i64);
        assert_eq!(*h, 42);
    }

    #[test]
    fn test_heap_clone() {
        let h1 = Heap::new(vec![1, 2, 3]);
        let h2 = h1.clone();
        assert_eq!(*h1, *h2);
        // They share the same allocation
        assert!(Arc::ptr_eq(&h1.0, &h2.0));
    }

    #[test]
    fn test_heap_eq() {
        let h1 = Heap::new("hello".to_string());
        let h2 = Heap::new("hello".to_string());
        let h3 = Heap::new("world".to_string());
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }
}
