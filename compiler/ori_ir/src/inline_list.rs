//! Two-tier inline/overflow storage for expression lists.
//!
//! This module provides `ExprList`, a compact representation that stores
//! 0-2 items inline and overflows to arena storage for larger lists.
//!
//! # Motivation
//!
//! Profiling shows that ~77% of function call arguments have 0-2 items.
//! By storing small lists inline, we:
//! - Eliminate indirection through ``expr_lists`` for common cases
//! - Improve cache locality (data is adjacent to the expression node)
//! - Reduce arena allocations
//!
//! # Memory Layout
//!
//! Both variants fit in 12 bytes:
//! - `Inline`: discriminant(1) + count(1) + padding(2) + items(8) = 12 bytes
//! - `Overflow`: discriminant(1) + padding(3) + start(4) + len(2) + padding(2) = 12 bytes
//!
//! # Salsa Compatibility
//!
//! All types have Copy, Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::ExprId;

/// Maximum number of items that can be stored inline.
pub const INLINE_CAPACITY: usize = 2;

/// Two-tier expression list: inline for small counts, overflow to arena for large.
///
/// # Usage
///
/// ```ignore
/// // Construction (in parser)
/// let list = ExprList::new(&[expr1, expr2], &mut arena);
///
/// // Access (anywhere with arena reference)
/// for expr_id in list.iter(&arena) {
///     let expr = arena.get_expr(expr_id);
/// }
/// ```
#[derive(Copy, Clone)]
pub enum ExprList {
    /// 0-2 items stored directly in the enum.
    ///
    /// The `count` field indicates how many of the `items` are valid.
    /// Items beyond `count` are set to `ExprId::INVALID`.
    Inline {
        /// Number of valid items (0, 1, or 2).
        count: u8,
        /// Storage for up to 2 items. Only first `count` items are valid.
        items: [ExprId; INLINE_CAPACITY],
    },

    /// 3+ items stored in arena's ``expr_lists``.
    ///
    /// This is equivalent to the old `ExprRange` representation.
    Overflow {
        /// Start index in arena's ``expr_lists``.
        start: u32,
        /// Number of items.
        len: u16,
    },
}

impl ExprList {
    /// Empty list constant.
    pub const EMPTY: ExprList = ExprList::Inline {
        count: 0,
        items: [ExprId::INVALID; INLINE_CAPACITY],
    };

    /// Create a new expression list from items.
    ///
    /// If `items.len() <= INLINE_CAPACITY`, stores inline.
    /// Otherwise, allocates to arena's ``expr_lists``.
    ///
    /// # Arguments
    ///
    /// * `items` - The expression IDs to store
    /// * `alloc` - Closure that allocates to arena when overflow is needed.
    ///   Takes an iterator and returns (start, len).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let list = ExprList::from_items(&[id1, id2], |iter| {
    ///     let start = arena.`expr_lists`.len() as u32;
    ///     arena.`expr_lists`.extend(iter);
    ///     let len = (arena.`expr_lists`.len() - start as usize) as u16;
    ///     (start, len)
    /// });
    /// ```
    pub fn from_items<F>(items: &[ExprId], mut alloc: F) -> Self
    where
        F: FnMut(&[ExprId]) -> (u32, u16),
    {
        if items.len() <= INLINE_CAPACITY {
            let mut storage = [ExprId::INVALID; INLINE_CAPACITY];
            for (i, &id) in items.iter().enumerate() {
                storage[i] = id;
            }
            ExprList::Inline {
                // Safe: INLINE_CAPACITY is 2, which fits in u8
                #[expect(clippy::cast_possible_truncation)]
                count: items.len() as u8,
                items: storage,
            }
        } else {
            let (start, len) = alloc(items);
            ExprList::Overflow { start, len }
        }
    }

    /// Create an inline list from a single item.
    #[inline]
    pub const fn single(item: ExprId) -> Self {
        ExprList::Inline {
            count: 1,
            items: [item, ExprId::INVALID],
        }
    }

    /// Create an inline list from two items.
    #[inline]
    pub const fn pair(first: ExprId, second: ExprId) -> Self {
        ExprList::Inline {
            count: 2,
            items: [first, second],
        }
    }

    /// Create an overflow list from a range.
    ///
    /// This is the migration path from `ExprRange`.
    #[inline]
    pub const fn from_range(start: u32, len: u16) -> Self {
        ExprList::Overflow { start, len }
    }

    /// Check if the list is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        match self {
            ExprList::Inline { count, .. } => *count == 0,
            ExprList::Overflow { len, .. } => *len == 0,
        }
    }

    /// Get the number of items.
    #[inline]
    pub const fn len(&self) -> usize {
        match self {
            ExprList::Inline { count, .. } => *count as usize,
            ExprList::Overflow { len, .. } => *len as usize,
        }
    }

    /// Check if this list uses inline storage.
    #[inline]
    pub const fn is_inline(&self) -> bool {
        matches!(self, ExprList::Inline { .. })
    }

    /// Get the first item, if any.
    #[inline]
    pub fn first(&self) -> Option<ExprId> {
        match self {
            ExprList::Inline { count, items } if *count > 0 => Some(items[0]),
            ExprList::Overflow { start, len } if *len > 0 => {
                // Note: caller must resolve through arena
                // We return the "virtual" first item indicator
                Some(ExprId::new(*start))
            }
            _ => None,
        }
    }

    /// Iterate over items.
    ///
    /// For inline lists, yields items directly.
    /// For overflow lists, caller must provide the arena's `expr_lists` slice.
    ///
    /// # Arguments
    ///
    /// * `expr_lists` - The arena's `expr_lists` storage (only used for Overflow)
    #[inline]
    pub fn iter<'a>(&self, expr_lists: &'a [ExprId]) -> ExprListIter<'a> {
        match *self {
            ExprList::Inline { count, items } => ExprListIter::Inline {
                items,
                index: 0,
                count,
            },
            ExprList::Overflow { start, len } => {
                let start = start as usize;
                let end = start + len as usize;
                ExprListIter::Overflow {
                    slice: &expr_lists[start..end],
                    index: 0,
                }
            }
        }
    }

    /// Get items as a slice (convenience for overflow case or when you need a slice).
    ///
    /// For inline lists, returns None (use `iter()` instead).
    /// For overflow lists, returns the slice from `expr_lists`.
    #[inline]
    pub fn as_overflow_slice<'a>(&self, expr_lists: &'a [ExprId]) -> Option<&'a [ExprId]> {
        match *self {
            ExprList::Inline { .. } => None,
            ExprList::Overflow { start, len } => {
                let start = start as usize;
                let end = start + len as usize;
                Some(&expr_lists[start..end])
            }
        }
    }

    /// Collect items into a Vec.
    ///
    /// This is useful when you need owned data or a slice-like interface.
    #[inline]
    pub fn to_vec(&self, expr_lists: &[ExprId]) -> Vec<ExprId> {
        self.iter(expr_lists).collect()
    }
}

impl Default for ExprList {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl PartialEq for ExprList {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                ExprList::Inline {
                    count: c1,
                    items: i1,
                },
                ExprList::Inline {
                    count: c2,
                    items: i2,
                },
            ) => {
                // Compare only valid items
                c1 == c2 && i1[..*c1 as usize] == i2[..*c2 as usize]
            }
            (
                ExprList::Overflow { start: s1, len: l1 },
                ExprList::Overflow { start: s2, len: l2 },
            ) => s1 == s2 && l1 == l2,
            _ => false,
        }
    }
}

impl Eq for ExprList {}

impl Hash for ExprList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ExprList::Inline { count, items } => {
                0u8.hash(state); // discriminant
                count.hash(state);
                // Only hash valid items
                for item in items.iter().take(*count as usize) {
                    item.hash(state);
                }
            }
            ExprList::Overflow { start, len } => {
                1u8.hash(state); // discriminant
                start.hash(state);
                len.hash(state);
            }
        }
    }
}

impl fmt::Debug for ExprList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprList::Inline { count, items } => {
                write!(f, "ExprList::Inline[")?;
                for (i, item) in items.iter().enumerate().take(*count as usize) {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item:?}")?;
                }
                write!(f, "]")
            }
            ExprList::Overflow { start, len } => {
                write!(
                    f,
                    "ExprList::Overflow({start}..{})",
                    start + u32::from(*len)
                )
            }
        }
    }
}

/// Iterator over expression list items.
#[derive(Clone)]
pub enum ExprListIter<'a> {
    /// Iterating over inline items.
    Inline {
        items: [ExprId; INLINE_CAPACITY],
        index: u8,
        count: u8,
    },
    /// Iterating over overflow slice.
    Overflow { slice: &'a [ExprId], index: usize },
}

impl Iterator for ExprListIter<'_> {
    type Item = ExprId;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ExprListIter::Inline {
                items,
                index,
                count,
            } => {
                if *index < *count {
                    let item = items[*index as usize];
                    *index += 1;
                    Some(item)
                } else {
                    None
                }
            }
            ExprListIter::Overflow { slice, index } => {
                if *index < slice.len() {
                    let item = slice[*index];
                    *index += 1;
                    Some(item)
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match self {
            ExprListIter::Inline { index, count, .. } => (*count - *index) as usize,
            ExprListIter::Overflow { slice, index } => slice.len() - *index,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ExprListIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_list() {
        let list = ExprList::EMPTY;
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.is_inline());
    }

    #[test]
    fn test_single_item() {
        let id = ExprId::new(42);
        let list = ExprList::single(id);

        assert!(!list.is_empty());
        assert_eq!(list.len(), 1);
        assert!(list.is_inline());

        let items: Vec<_> = list.iter(&[]).collect();
        assert_eq!(items, vec![id]);
    }

    #[test]
    fn test_pair_items() {
        let id1 = ExprId::new(1);
        let id2 = ExprId::new(2);
        let list = ExprList::pair(id1, id2);

        assert!(!list.is_empty());
        assert_eq!(list.len(), 2);
        assert!(list.is_inline());

        let items: Vec<_> = list.iter(&[]).collect();
        assert_eq!(items, vec![id1, id2]);
    }

    #[test]
    fn test_from_items_inline() {
        let ids = [ExprId::new(1), ExprId::new(2)];
        let list = ExprList::from_items(&ids, |_| panic!("should not allocate"));

        assert!(list.is_inline());
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_from_items_empty() {
        let list = ExprList::from_items(&[], |_| panic!("should not allocate"));

        assert!(list.is_inline());
        assert!(list.is_empty());
    }

    #[test]
    fn test_from_items_overflow() {
        let ids = [ExprId::new(1), ExprId::new(2), ExprId::new(3)];
        let list = ExprList::from_items(&ids, |items| {
            assert_eq!(items.len(), 3);
            (10, 3) // Simulated allocation at position 10
        });

        assert!(!list.is_inline());
        assert_eq!(list.len(), 3);

        // Verify overflow details
        if let ExprList::Overflow { start, len } = list {
            assert_eq!(start, 10);
            assert_eq!(len, 3);
        } else {
            panic!("expected Overflow variant");
        }
    }

    #[test]
    fn test_overflow_iteration() {
        let list = ExprList::from_range(5, 3);
        let expr_lists = [
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
            ExprId::new(4),
            ExprId::new(100), // index 5
            ExprId::new(101), // index 6
            ExprId::new(102), // index 7
        ];

        let items: Vec<_> = list.iter(&expr_lists).collect();
        assert_eq!(
            items,
            vec![ExprId::new(100), ExprId::new(101), ExprId::new(102)]
        );
    }

    #[test]
    fn test_to_vec() {
        let id1 = ExprId::new(1);
        let id2 = ExprId::new(2);
        let list = ExprList::pair(id1, id2);

        let vec = list.to_vec(&[]);
        assert_eq!(vec, vec![id1, id2]);
    }

    #[test]
    fn test_equality() {
        let list1 = ExprList::single(ExprId::new(42));
        let list2 = ExprList::single(ExprId::new(42));
        let list3 = ExprList::single(ExprId::new(43));

        assert_eq!(list1, list2);
        assert_ne!(list1, list3);

        // Different variants are never equal
        let overflow = ExprList::from_range(0, 1);
        assert_ne!(list1, overflow);
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;

        let mut set = HashSet::new();

        let list1 = ExprList::single(ExprId::new(42));
        let list2 = ExprList::single(ExprId::new(42));

        set.insert(list1);
        set.insert(list2); // duplicate

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_iterator_exact_size() {
        let list = ExprList::pair(ExprId::new(1), ExprId::new(2));
        let iter = list.iter(&[]);

        assert_eq!(iter.len(), 2);
    }

    #[test]
    fn test_debug_format() {
        let inline = ExprList::pair(ExprId::new(1), ExprId::new(2));
        let debug = format!("{inline:?}");
        assert!(debug.contains("Inline"));

        let overflow = ExprList::from_range(10, 5);
        let debug = format!("{overflow:?}");
        assert!(debug.contains("Overflow"));
        assert!(debug.contains("10..15"));
    }

    #[test]
    fn test_default() {
        let list: ExprList = ExprList::default();
        assert!(list.is_empty());
        assert_eq!(list, ExprList::EMPTY);
    }

    #[test]
    fn test_memory_size() {
        use crate::ExprRange;

        // Verify our size assumptions
        let size = std::mem::size_of::<ExprList>();
        // Should be 12 bytes based on our layout analysis
        assert!(size <= 16, "ExprList should be compact, got {size} bytes");
        println!("ExprList size: {size} bytes");

        // Compare to ExprRange (8 bytes)
        let range_size = std::mem::size_of::<ExprRange>();
        println!("ExprRange size: {range_size} bytes");
    }
}
