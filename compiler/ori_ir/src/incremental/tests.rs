use super::*;

// === TextChange tests ===

#[test]
fn test_text_change_insert() {
    let change = TextChange::insert(10, 5);
    assert_eq!(change.start, 10);
    assert_eq!(change.old_end, 10);
    assert_eq!(change.new_len, 5);
    assert_eq!(change.delta(), 5);
    assert_eq!(change.old_len(), 0);
    assert_eq!(change.new_end(), 15);
}

#[test]
fn test_text_change_delete() {
    let change = TextChange::delete(10, 5);
    assert_eq!(change.start, 10);
    assert_eq!(change.old_end, 15);
    assert_eq!(change.new_len, 0);
    assert_eq!(change.delta(), -5);
    assert_eq!(change.old_len(), 5);
    assert_eq!(change.new_end(), 10);
}

#[test]
fn test_text_change_replace() {
    // Replace 3 chars with 5 chars
    let change = TextChange::replace(10, 3, 5);
    assert_eq!(change.start, 10);
    assert_eq!(change.old_end, 13);
    assert_eq!(change.new_len, 5);
    assert_eq!(change.delta(), 2);
    assert_eq!(change.old_len(), 3);
    assert_eq!(change.new_end(), 15);
}

#[test]
fn test_text_change_intersects() {
    let change = TextChange::new(10, 20, 15);

    // Before - no intersection
    assert!(!change.intersects(Span::new(0, 5)));
    assert!(!change.intersects(Span::new(0, 10))); // Adjacent, exclusive end

    // Overlapping
    assert!(change.intersects(Span::new(5, 15)));
    assert!(change.intersects(Span::new(15, 25)));
    assert!(change.intersects(Span::new(5, 25))); // Contains change

    // Contained
    assert!(change.intersects(Span::new(12, 18)));

    // After - no intersection
    assert!(!change.intersects(Span::new(20, 30))); // Adjacent
    assert!(!change.intersects(Span::new(25, 35)));
}

#[test]
fn test_text_change_contains() {
    let change = TextChange::new(10, 20, 15);

    assert!(change.contains(Span::new(10, 20))); // Exact match
    assert!(change.contains(Span::new(12, 18))); // Strictly inside
    assert!(change.contains(Span::new(10, 15))); // At start
    assert!(change.contains(Span::new(15, 20))); // At end

    assert!(!change.contains(Span::new(5, 15))); // Extends before
    assert!(!change.contains(Span::new(15, 25))); // Extends after
    assert!(!change.contains(Span::new(0, 5))); // Entirely before
}

#[test]
fn test_text_change_is_before_after() {
    let change = TextChange::new(10, 20, 15);

    // Before
    assert!(change.is_before(Span::new(0, 5)));
    assert!(change.is_before(Span::new(0, 10))); // Adjacent
    assert!(!change.is_before(Span::new(5, 15))); // Overlaps

    // After
    assert!(change.is_after(Span::new(25, 30)));
    assert!(change.is_after(Span::new(20, 30))); // Adjacent
    assert!(!change.is_after(Span::new(15, 25))); // Overlaps
}

// === ChangeMarker tests ===

#[test]
fn test_change_marker_from_change() {
    let change = TextChange::new(100, 110, 15);

    // Previous token ended at 95
    let marker = ChangeMarker::from_change(&change, 95);
    assert_eq!(marker.affected_start, 95);
    assert_eq!(marker.affected_end, 110);
    assert_eq!(marker.delta, 5);

    // Previous token ended after change start (use change.start)
    let marker2 = ChangeMarker::from_change(&change, 105);
    assert_eq!(marker2.affected_start, 100);
}

#[test]
fn test_change_marker_adjust_position() {
    let marker = ChangeMarker::new(100, 110, 5);

    // Strictly before affected region - unchanged
    assert_eq!(marker.adjust_position(50), 50);
    assert_eq!(marker.adjust_position(99), 99);

    // Inside affected region (start <= pos < end) - unchanged (caller should reparse)
    assert_eq!(marker.adjust_position(100), 100);
    assert_eq!(marker.adjust_position(105), 105);
    assert_eq!(marker.adjust_position(109), 109);

    // At or after affected end - shifted
    assert_eq!(marker.adjust_position(110), 115);
    assert_eq!(marker.adjust_position(200), 205);
}

#[test]
fn test_change_marker_adjust_position_negative_delta() {
    let marker = ChangeMarker::new(100, 120, -10);

    // Before - unchanged
    assert_eq!(marker.adjust_position(50), 50);

    // After - shifted backward
    assert_eq!(marker.adjust_position(120), 110);
    assert_eq!(marker.adjust_position(200), 190);
}

#[test]
fn test_change_marker_intersects() {
    let marker = ChangeMarker::new(100, 110, 5);

    // Before - no intersection
    assert!(!marker.intersects(Span::new(0, 50)));
    assert!(!marker.intersects(Span::new(0, 100))); // Adjacent

    // Overlapping
    assert!(marker.intersects(Span::new(50, 105)));
    assert!(marker.intersects(Span::new(105, 150)));

    // After - no intersection
    assert!(!marker.intersects(Span::new(110, 150))); // Adjacent
    assert!(!marker.intersects(Span::new(150, 200)));
}

#[test]
fn test_change_marker_is_before_after() {
    let marker = ChangeMarker::new(100, 110, 5);

    // Before
    assert!(marker.is_before(Span::new(0, 50)));
    assert!(marker.is_before(Span::new(0, 100))); // Adjacent
    assert!(!marker.is_before(Span::new(50, 105)));

    // After
    assert!(marker.is_after(Span::new(150, 200)));
    assert!(marker.is_after(Span::new(110, 150))); // Adjacent
    assert!(!marker.is_after(Span::new(105, 150)));
}

#[test]
fn test_change_marker_adjust_span() {
    let marker = ChangeMarker::new(100, 110, 5);

    // Before affected region - unchanged
    let before = Span::new(10, 50);
    assert_eq!(marker.adjust_span(before), Some(Span::new(10, 50)));

    // After affected region - shifted
    let after = Span::new(150, 200);
    assert_eq!(marker.adjust_span(after), Some(Span::new(155, 205)));

    // Intersecting - None
    let intersecting = Span::new(50, 105);
    assert_eq!(marker.adjust_span(intersecting), None);
}

#[test]
fn test_change_marker_edge_cases() {
    // Empty affected region (just an insertion point)
    // When affected_start == affected_end == 100:
    // - pos < 100: strictly before, unchanged
    // - pos >= 100: at or after the end, shifted
    //
    // This is correct for insertions: text inserted at position 100
    // pushes everything at position 100 and beyond forward.
    let marker = ChangeMarker::new(100, 100, 10);
    assert_eq!(marker.adjust_position(99), 99); // Before - unchanged
    assert_eq!(marker.adjust_position(100), 110); // At insertion point - shifted
    assert_eq!(marker.adjust_position(101), 111); // After - shifted
}

#[test]
fn test_text_change_zero_delta() {
    // Replace with same length
    let change = TextChange::replace(10, 5, 5);
    assert_eq!(change.delta(), 0);

    let marker = ChangeMarker::from_change(&change, 5);
    assert_eq!(marker.delta, 0);
    assert_eq!(marker.adjust_position(100), 100);
}

#[test]
fn test_salsa_traits() {
    use std::collections::HashSet;

    // TextChange is hashable
    let mut changes = HashSet::new();
    changes.insert(TextChange::insert(10, 5));
    changes.insert(TextChange::insert(10, 5)); // Duplicate
    changes.insert(TextChange::insert(20, 5));
    assert_eq!(changes.len(), 2);

    // ChangeMarker is hashable
    let mut markers = HashSet::new();
    markers.insert(ChangeMarker::new(10, 20, 5));
    markers.insert(ChangeMarker::new(10, 20, 5)); // Duplicate
    markers.insert(ChangeMarker::new(30, 40, -5));
    assert_eq!(markers.len(), 2);
}
