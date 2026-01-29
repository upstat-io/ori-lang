//! Source location spans.
//!
//! Provides compact 8-byte span representation with all Salsa-required traits.

use std::fmt;

/// Error when creating a span from a range that exceeds u32::MAX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanError {
    /// Span start position exceeds u32::MAX
    StartTooLarge(usize),
    /// Span end position exceeds u32::MAX
    EndTooLarge(usize),
}

impl std::fmt::Display for SpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanError::StartTooLarge(v) => write!(f, "span start {} exceeds u32::MAX", v),
            SpanError::EndTooLarge(v) => write!(f, "span end {} exceeds u32::MAX", v),
        }
    }
}

impl std::error::Error for SpanError {}

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

    /// Try to create a span from a byte range.
    ///
    /// Returns an error if the range exceeds `u32::MAX` bytes.
    /// Use this for fallible conversion when handling user input.
    #[inline]
    pub fn try_from_range(range: std::ops::Range<usize>) -> Result<Self, SpanError> {
        let start = u32::try_from(range.start).map_err(|_| SpanError::StartTooLarge(range.start))?;
        let end = u32::try_from(range.end).map_err(|_| SpanError::EndTooLarge(range.end))?;
        Ok(Span { start, end })
    }

    /// Create from a byte range.
    ///
    /// # Panics
    /// Panics if the range exceeds `u32::MAX` bytes.
    /// Use `try_from_range` for fallible conversion when handling user input.
    #[inline]
    pub fn from_range(range: std::ops::Range<usize>) -> Self {
        Self::try_from_range(range).unwrap_or_else(|e| panic!("{}", e))
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
        Span {
            start: offset,
            end: offset,
        }
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
