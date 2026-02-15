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
