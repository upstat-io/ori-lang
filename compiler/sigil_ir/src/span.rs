//! Source location spans.
//!
//! Provides compact 8-byte span representation with all Salsa-required traits.

use std::fmt;

/// Source location span.
///
/// Layout: 8 bytes total
/// - start: u32 - byte offset from file start
/// - end: u32 - byte offset (exclusive)
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug, Default
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Dummy span for generated code.
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    /// Create a new span.
    #[inline]
    pub const fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }

    /// Create from a byte range.
    ///
    /// # Panics
    /// Panics if the range exceeds `u32::MAX` bytes.
    #[inline]
    pub fn from_range(range: std::ops::Range<usize>) -> Self {
        Span {
            start: u32::try_from(range.start).unwrap_or_else(|_| {
                panic!("span start {} exceeds u32::MAX", range.start)
            }),
            end: u32::try_from(range.end).unwrap_or_else(|_| {
                panic!("span end {} exceeds u32::MAX", range.end)
            }),
        }
    }

    /// Length of the span in bytes.
    #[inline]
    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Check if span is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Check if an offset is within this span.
    #[inline]
    pub fn contains(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Merge two spans to create one covering both.
    #[inline]
    #[must_use]
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Extend span to include another position.
    #[inline]
    #[must_use]
    pub fn extend_to(self, end: u32) -> Span {
        Span {
            start: self.start,
            end: self.end.max(end),
        }
    }

    /// Create a point span (zero-length).
    #[inline]
    pub const fn point(offset: u32) -> Span {
        Span { start: offset, end: offset }
    }

    /// Convert to a `std::ops::Range`.
    #[inline]
    pub fn to_range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

// Size assertions to prevent accidental regressions
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::Span;
    crate::static_assert_size!(Span, 8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_basic() {
        let span = Span::new(10, 20);
        assert_eq!(span.len(), 10);
        assert!(!span.is_empty());
        assert!(span.contains(15));
        assert!(!span.contains(20));
    }

    #[test]
    fn test_span_merge() {
        let a = Span::new(10, 20);
        let b = Span::new(15, 30);
        let merged = a.merge(b);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_span_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Span::new(0, 10));
        set.insert(Span::new(0, 10)); // duplicate
        set.insert(Span::new(5, 15));
        assert_eq!(set.len(), 2);
    }
}
