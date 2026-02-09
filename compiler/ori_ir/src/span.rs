//! Source location spans.
//!
//! Provides compact 8-byte span representation with all Salsa-required traits.

use std::fmt;

/// Error when creating a span from a range that exceeds `u32::MAX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanError {
    /// Span start position exceeds `u32::MAX`.
    StartTooLarge(usize),
    /// Span end position exceeds `u32::MAX`.
    EndTooLarge(usize),
}

impl std::fmt::Display for SpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanError::StartTooLarge(v) => write!(
                f,
                "span start {} (0x{:X}) exceeds u32::MAX (0x{:X})",
                v,
                v,
                u32::MAX
            ),
            SpanError::EndTooLarge(v) => write!(
                f,
                "span end {} (0x{:X}) exceeds u32::MAX (0x{:X})",
                v,
                v,
                u32::MAX
            ),
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
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
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
        let start =
            u32::try_from(range.start).map_err(|_| SpanError::StartTooLarge(range.start))?;
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

    /// Check if another span is fully contained within this span.
    #[inline]
    pub fn contains_span(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
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

    // Boundary condition tests

    #[test]
    fn test_span_u32_max_boundaries() {
        // Test with u32::MAX values
        let span = Span::new(u32::MAX - 10, u32::MAX);
        assert_eq!(span.len(), 10);
        assert!(!span.is_empty());
        assert!(span.contains(u32::MAX - 5));
        assert!(!span.contains(u32::MAX)); // end is exclusive
    }

    #[test]
    fn test_span_from_range_success() {
        let span = Span::from_range(100..200);
        assert_eq!(span.start, 100);
        assert_eq!(span.end, 200);
    }

    #[test]
    fn test_span_try_from_range_success() {
        let result = Span::try_from_range(50..100);
        let Ok(span) = result else {
            panic!("expected Ok for valid range");
        };
        assert_eq!(span.start, 50);
        assert_eq!(span.end, 100);
    }

    #[test]
    fn test_span_try_from_range_start_too_large() {
        let large_start = u32::MAX as usize + 1;
        let result = Span::try_from_range(large_start..large_start + 10);
        assert!(result.is_err());
        assert!(matches!(result, Err(SpanError::StartTooLarge(_))));
    }

    #[test]
    fn test_span_try_from_range_end_too_large() {
        let large_end = u32::MAX as usize + 1;
        let result = Span::try_from_range(0..large_end);
        assert!(result.is_err());
        assert!(matches!(result, Err(SpanError::EndTooLarge(_))));
    }

    #[test]
    fn test_span_error_display() {
        let err = SpanError::StartTooLarge(0x1_0000_0000);
        let msg = format!("{err}");
        assert!(msg.contains("start"));
        assert!(msg.contains("0x100000000"));

        let err = SpanError::EndTooLarge(0x2_0000_0000);
        let msg = format!("{err}");
        assert!(msg.contains("end"));
        assert!(msg.contains("0x200000000"));
    }

    #[test]
    fn test_span_merge_disjoint() {
        // Merge non-overlapping spans
        let a = Span::new(0, 10);
        let b = Span::new(20, 30);
        let merged = a.merge(b);
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_span_merge_reversed_order() {
        // Merge where second span starts before first
        let a = Span::new(20, 30);
        let b = Span::new(10, 25);
        let merged = a.merge(b);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_span_point() {
        let point = Span::point(42);
        assert_eq!(point.start, 42);
        assert_eq!(point.end, 42);
        assert!(point.is_empty());
        assert_eq!(point.len(), 0);
    }

    #[test]
    fn test_span_contains_boundary() {
        let span = Span::new(10, 20);

        // Boundary at start (inclusive)
        assert!(span.contains(10));

        // Boundary at end (exclusive)
        assert!(!span.contains(20));

        // One before start
        assert!(!span.contains(9));

        // One before end
        assert!(span.contains(19));
    }

    #[test]
    fn test_span_extend_to() {
        let span = Span::new(10, 20);

        // Extend beyond current end
        let extended = span.extend_to(30);
        assert_eq!(extended.start, 10);
        assert_eq!(extended.end, 30);

        // Extend to less than current end (no change)
        let not_extended = span.extend_to(15);
        assert_eq!(not_extended.start, 10);
        assert_eq!(not_extended.end, 20);
    }

    #[test]
    fn test_span_to_range() {
        let span = Span::new(10, 20);
        let range = span.to_range();
        assert_eq!(range.start, 10);
        assert_eq!(range.end, 20);
    }

    #[test]
    fn test_span_dummy() {
        assert_eq!(Span::DUMMY.start, 0);
        assert_eq!(Span::DUMMY.end, 0);
        assert!(Span::DUMMY.is_empty());
    }

    #[test]
    fn test_span_debug_display() {
        let span = Span::new(100, 200);
        assert_eq!(format!("{span:?}"), "100..200");
        assert_eq!(format!("{span}"), "100..200");
    }

    #[test]
    fn test_span_default() {
        let default: Span = Span::default();
        assert_eq!(default, Span::DUMMY);
    }
}
