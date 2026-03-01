//! Zero-copy list data with offset/length slicing.
//!
//! `ListData` wraps a shared `Heap<Vec<Value>>` backing buffer with an
//! `offset` and `len` that define a logical window. Slice operations
//! (`slice`, `take`, `skip`) share the same Arc-backed buffer via O(1)
//! clone, avoiding the O(n) copies that `.to_vec()` would require.
//!
//! Mutation methods (`push`, `insert`, `remove`, `set`, `reverse`,
//! `sort_by`, `extend_from_slice`) materialize the slice window into
//! an owned Vec first (if needed), then apply COW via `Heap::make_mut`.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use super::heap::Heap;
use super::Value;

/// List data with zero-copy slicing support.
///
/// Stores a shared backing buffer with offset/length to define the
/// visible window. Multiple `ListData` values can share the same
/// underlying `Vec<Value>` through Arc reference counting.
pub struct ListData {
    /// Shared storage (Arc-wrapped).
    backing: Heap<Vec<Value>>,
    /// Start index into backing.
    offset: usize,
    /// Logical length of the visible window.
    len: usize,
}

impl ListData {
    /// Create an owned `ListData` from a freshly-allocated Vec.
    ///
    /// The offset is 0 and len covers the entire Vec.
    #[inline]
    pub(super) fn owned(items: Vec<Value>) -> Self {
        let len = items.len();
        ListData {
            backing: Heap::new(items),
            offset: 0,
            len,
        }
    }

    /// Returns `true` if this is a slice (offset > 0 or len < backing len).
    #[inline]
    pub fn is_slice(&self) -> bool {
        self.offset != 0 || self.len != self.backing.len()
    }

    /// Get a reference to the shared backing buffer.
    ///
    /// Used by `IteratorValue` to create list iterators that share
    /// the same Arc without copying.
    #[inline]
    pub fn backing(&self) -> &Heap<Vec<Value>> {
        &self.backing
    }

    /// Get the start offset into the backing buffer.
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    // --- Zero-copy slice operations ---

    /// Create a zero-copy sub-slice sharing the same backing buffer.
    ///
    /// `start` and `end` are relative to the current window (not the
    /// backing buffer). Out-of-range indices are clamped.
    #[must_use]
    pub fn slice(&self, start: usize, end: usize) -> Self {
        let end = end.min(self.len);
        let start = start.min(end);
        ListData {
            backing: self.backing.clone(),
            offset: self.offset.saturating_add(start),
            len: end.saturating_sub(start),
        }
    }

    /// Take the first `n` elements as a zero-copy slice.
    #[must_use]
    pub fn take(&self, n: usize) -> Self {
        self.slice(0, n)
    }

    /// Skip the first `n` elements as a zero-copy slice.
    #[must_use]
    pub fn skip(&self, n: usize) -> Self {
        let n = n.min(self.len);
        self.slice(n, self.len)
    }

    // --- Mutation methods (materialize + COW) ---

    /// Materialize the slice into an owned Vec if this is a slice view.
    ///
    /// After materialization, offset is 0 and len equals backing len.
    /// This is a prerequisite for all mutation operations.
    fn materialize_if_slice(&mut self) {
        if self.is_slice() {
            let materialized = self[..].to_vec();
            self.backing = Heap::new(materialized);
            self.offset = 0;
            // len stays the same — it matches the materialized Vec's length
        }
    }

    /// Push a value onto the end of the list.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "len increment after successful push cannot overflow (bounded by memory)"
    )]
    pub fn push(&mut self, value: Value) {
        self.materialize_if_slice();
        self.backing.make_mut().push(value);
        self.len += 1;
    }

    /// Insert a value at the given index.
    ///
    /// # Panics
    /// Panics if `index > self.len`.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "len increment after successful insert cannot overflow (bounded by memory)"
    )]
    pub fn insert(&mut self, index: usize, value: Value) {
        self.materialize_if_slice();
        self.backing.make_mut().insert(index, value);
        self.len += 1;
    }

    /// Remove the element at the given index.
    ///
    /// # Panics
    /// Panics if `index >= self.len`.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "len decrement after successful remove; len > 0 since index < len"
    )]
    pub fn remove(&mut self, index: usize) {
        self.materialize_if_slice();
        self.backing.make_mut().remove(index);
        self.len -= 1;
    }

    /// Set the element at the given index.
    ///
    /// # Panics
    /// Panics if `index >= self.len`.
    pub fn set(&mut self, index: usize, value: Value) {
        self.materialize_if_slice();
        self.backing.make_mut()[index] = value;
    }

    /// Reverse the list in-place.
    pub fn reverse(&mut self) {
        self.materialize_if_slice();
        self.backing.make_mut().reverse();
    }

    /// Sort the list using a custom comparator.
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&Value, &Value) -> Ordering,
    {
        self.materialize_if_slice();
        self.backing.make_mut().sort_by(compare);
    }

    /// Extend the list with elements from a slice.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "len addition after extend; bounded by memory"
    )]
    pub fn extend_from_slice(&mut self, other: &[Value]) {
        self.materialize_if_slice();
        self.backing.make_mut().extend_from_slice(other);
        self.len += other.len();
    }

    /// Consume this `ListData` and return the elements as a Vec.
    ///
    /// If this is a slice, materializes the visible window. If this is
    /// an owned list with a unique reference, tries to unwrap without cloning.
    pub fn into_vec(self) -> Vec<Value> {
        if self.is_slice() {
            return self[..].to_vec();
        }
        match self.backing.try_into_inner() {
            Ok(vec) => vec,
            Err(heap) => (*heap).clone(),
        }
    }
}

impl Deref for ListData {
    type Target = [Value];

    #[inline]
    fn deref(&self) -> &[Value] {
        &self.backing[self.offset..self.offset.saturating_add(self.len)]
    }
}

impl Clone for ListData {
    /// Clone is O(1) — just bumps the Arc refcount and copies offset/len.
    #[inline]
    fn clone(&self) -> Self {
        ListData {
            backing: self.backing.clone(),
            offset: self.offset,
            len: self.len,
        }
    }
}

impl PartialEq for ListData {
    /// Equality compares the visible window, not the backing buffer.
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for ListData {}

impl Hash for ListData {
    /// Hashes the visible window elements, not the backing buffer.
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl fmt::Debug for ListData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slice: &[Value] = self;
        write!(f, "{slice:?}")
    }
}

#[cfg(test)]
mod tests;
