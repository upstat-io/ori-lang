//! Scratch buffer for temporary allocations during parsing.
//!
//! Provides a reusable buffer to reduce per-list allocations.
//! Inspired by Zig's parser which uses a scratch buffer for temporary collections.
//!
//! # Status
//! This module provides infrastructure for the scratch buffer optimization
//! described in `plans/ori_parse_improvements/section-01-quick-wins.md`.
//! The types are ready for use but not yet integrated into the parser.
//! Migration of list-parsing functions will be done incrementally as needed.
//!
//! # Usage Pattern
//! ```ignore
//! fn parse_items(&mut self) -> Result<ItemRange, ParseError> {
//!     // Borrow scratch buffer with RAII scope
//!     let mut scope = self.scratch.scope();
//!
//!     while has_more {
//!         let item = self.parse_item()?;
//!         scope.push(item);
//!     }
//!
//!     // Copy to arena and get range
//!     let range = self.arena.alloc_items(scope.drain());
//!     Ok(range)
//!     // scope drops here, buffer is truncated automatically
//! }
//! ```

// Infrastructure for future optimization - not yet integrated into parser.
// See plans/ori_parse_improvements/section-01-quick-wins.md for details.
#![allow(dead_code)]

use std::marker::PhantomData;

/// A reusable buffer for temporary storage during parsing.
///
/// The buffer maintains a single backing Vec that grows as needed but is never
/// shrunk. Multiple "scopes" can be nested, each tracking their start position.
/// When a scope is dropped, the buffer is truncated back to that position.
///
/// # Performance
/// - Avoids repeated allocations for small temporary collections
/// - The backing Vec grows to the high-water mark and stays there
/// - Stack-like scope semantics allow nested list parsing
#[derive(Debug, Default)]
pub struct ScratchBuffer<T> {
    /// Storage for all temporary items.
    storage: Vec<T>,
}

impl<T> ScratchBuffer<T> {
    /// Create a new empty scratch buffer.
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
        }
    }

    /// Create a scratch buffer with pre-allocated capacity.
    ///
    /// # Heuristics
    /// For parsing, a good starting capacity is `source_len / 50`, which
    /// roughly estimates the number of AST nodes in typical code.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: Vec::with_capacity(capacity),
        }
    }

    /// Begin a new scope for temporary storage.
    ///
    /// The returned `ScratchScope` provides push access and automatically
    /// truncates the buffer when dropped.
    pub fn scope(&mut self) -> ScratchScope<'_, T> {
        let start = self.storage.len();
        ScratchScope {
            buffer: self,
            start,
            _marker: PhantomData,
        }
    }

    /// Get the current capacity of the buffer.
    ///
    /// This is useful for monitoring memory usage.
    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    /// Get the current length of the buffer.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.storage.len()
    }
}

/// An RAII guard for a scratch buffer scope.
///
/// When dropped, truncates the buffer back to the start position,
/// allowing the memory to be reused by subsequent scopes.
pub struct ScratchScope<'a, T> {
    buffer: &'a mut ScratchBuffer<T>,
    start: usize,
    _marker: PhantomData<T>,
}

impl<T> ScratchScope<'_, T> {
    /// Push an item to the scratch buffer.
    #[inline]
    pub fn push(&mut self, item: T) {
        self.buffer.storage.push(item);
    }

    /// Get a slice of items in this scope.
    pub fn as_slice(&self) -> &[T] {
        &self.buffer.storage[self.start..]
    }

    /// Get the number of items in this scope.
    pub fn len(&self) -> usize {
        self.buffer.storage.len() - self.start
    }

    /// Check if this scope is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drain all items from this scope, consuming them.
    ///
    /// This is typically called right before the scope is dropped to
    /// transfer ownership of the items to another container.
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.buffer.storage.drain(self.start..)
    }
}

impl<T> Drop for ScratchScope<'_, T> {
    fn drop(&mut self) {
        // Truncate back to start position, allowing memory reuse
        self.buffer.storage.truncate(self.start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scratch_buffer_basic() {
        let mut buf: ScratchBuffer<i32> = ScratchBuffer::new();

        {
            let mut scope = buf.scope();
            scope.push(1);
            scope.push(2);
            scope.push(3);
            assert_eq!(scope.len(), 3);
            assert_eq!(scope.as_slice(), &[1, 2, 3]);
        }

        // After scope drops, buffer should be empty but retain capacity
        assert_eq!(buf.len(), 0);
        assert!(buf.capacity() >= 3);
    }

    #[test]
    fn test_scratch_buffer_sequential_scopes() {
        let mut buf: ScratchBuffer<i32> = ScratchBuffer::new();

        // First scope
        {
            let mut scope1 = buf.scope();
            scope1.push(1);
            scope1.push(2);
            assert_eq!(scope1.as_slice(), &[1, 2]);
        }
        // scope1 dropped

        // Second scope reuses the same buffer
        {
            let mut scope2 = buf.scope();
            scope2.push(10);
            scope2.push(20);
            scope2.push(30);
            assert_eq!(scope2.as_slice(), &[10, 20, 30]);
        }
        // scope2 dropped

        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_scratch_buffer_drain() {
        let mut buf: ScratchBuffer<i32> = ScratchBuffer::new();
        let collected: Vec<i32>;

        {
            let mut scope = buf.scope();
            scope.push(1);
            scope.push(2);
            scope.push(3);

            collected = scope.drain().collect();
        }

        assert_eq!(collected, vec![1, 2, 3]);
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_scratch_buffer_reuse() {
        let mut buf: ScratchBuffer<i32> = ScratchBuffer::new();

        // First use
        {
            let mut scope = buf.scope();
            scope.push(1);
            scope.push(2);
        }

        let cap_after_first = buf.capacity();

        // Second use - should reuse existing capacity
        {
            let mut scope = buf.scope();
            scope.push(10);
            scope.push(20);
            scope.push(30);
        }

        // Capacity should have grown to accommodate 3 items
        assert!(buf.capacity() >= 3);
        // But if first use was smaller, we should have reused that allocation
        // (This test mainly verifies no crashes and basic reuse logic)

        // Third use with fewer items
        {
            let mut scope = buf.scope();
            scope.push(100);
            assert_eq!(scope.len(), 1);
        }

        // Capacity should still be at least what it was
        assert!(buf.capacity() >= cap_after_first);
    }

    #[test]
    fn test_scratch_buffer_with_capacity() {
        let buf: ScratchBuffer<i32> = ScratchBuffer::with_capacity(100);
        assert!(buf.capacity() >= 100);
    }

    #[test]
    fn test_scratch_scope_empty() {
        let mut buf: ScratchBuffer<i32> = ScratchBuffer::new();
        let scope = buf.scope();
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
    }
}
