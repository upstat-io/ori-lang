use super::*;

#[test]
fn test_text_edit_insert() {
    let edit = TextEdit::insert(10, "hello");

    assert_eq!(edit.span, Span::new(10, 10));
    assert_eq!(edit.new_text, "hello");
    assert!(edit.is_insert());
    assert!(!edit.is_delete());
    assert!(!edit.is_replace());
    assert_eq!(edit.length_delta(), 5);
}

#[test]
fn test_text_edit_delete() {
    let edit = TextEdit::delete(Span::new(10, 20));

    assert_eq!(edit.span, Span::new(10, 20));
    assert!(edit.new_text.is_empty());
    assert!(!edit.is_insert());
    assert!(edit.is_delete());
    assert!(!edit.is_replace());
    assert_eq!(edit.length_delta(), -10);
}

#[test]
fn test_text_edit_replace() {
    let edit = TextEdit::replace(Span::new(10, 15), "longer text");

    assert_eq!(edit.span, Span::new(10, 15));
    assert_eq!(edit.new_text, "longer text");
    assert!(!edit.is_insert());
    assert!(!edit.is_delete());
    assert!(edit.is_replace());
    assert_eq!(edit.length_delta(), 6); // 11 - 5 = 6
}

#[test]
fn test_tracker_simple_replace() {
    let mut tracker = ChangeTracker::new();
    tracker.replace(Span::new(6, 11), "Ori");

    let result = tracker.apply("Hello World!");
    assert_eq!(result, "Hello Ori!");
}

#[test]
fn test_tracker_insert() {
    let mut tracker = ChangeTracker::new();
    tracker.insert_before(6, "beautiful ");

    let result = tracker.apply("Hello World!");
    assert_eq!(result, "Hello beautiful World!");
}

#[test]
fn test_tracker_delete() {
    let mut tracker = ChangeTracker::new();
    tracker.delete(Span::new(5, 11));

    let result = tracker.apply("Hello World!");
    assert_eq!(result, "Hello!");
}

#[test]
fn test_tracker_multiple_edits() {
    let mut tracker = ChangeTracker::new();
    tracker.insert_before(0, "// comment\n");
    tracker.replace(Span::new(4, 5), "mut ");

    let result = tracker.apply("let x = 42;");
    assert_eq!(result, "// comment\nlet mut  = 42;");
}

#[test]
fn test_tracker_non_overlapping() {
    let mut tracker = ChangeTracker::new();
    // "let x = 42;"
    //  0123456789A   (positions in hex for clarity)
    // Replace "let x" (0-5) with "const"
    tracker.replace(Span::new(0, 5), "const");
    // Replace "42" (8-10) with "100"
    tracker.replace(Span::new(8, 10), "100");

    // "let x = 42;" -> "const = 100;"
    let result = tracker.apply("let x = 42;");
    assert_eq!(result, "const = 100;");
}

#[test]
fn test_tracker_insert_at_same_position() {
    let mut tracker = ChangeTracker::new();
    tracker.insert_before(0, "// first\n");
    tracker.insert_before(0, "// second\n");

    let result = tracker.apply("code");
    // Both inserts at position 0, order depends on implementation
    assert!(result.contains("// first\n"));
    assert!(result.contains("// second\n"));
    assert!(result.ends_with("code"));
}

#[test]
fn test_tracker_conflict_detection() {
    let mut tracker = ChangeTracker::new();
    tracker.replace(Span::new(5, 15), "aaa");
    tracker.replace(Span::new(10, 20), "bbb");

    let conflict = tracker.check_conflicts();
    assert!(conflict.is_some());
}

#[test]
fn test_tracker_no_conflict_adjacent() {
    let mut tracker = ChangeTracker::new();
    tracker.replace(Span::new(0, 5), "aaa");
    tracker.replace(Span::new(5, 10), "bbb");

    let conflict = tracker.check_conflicts();
    assert!(conflict.is_none());
}

#[test]
fn test_tracker_apply_checked_ok() {
    let mut tracker = ChangeTracker::new();
    tracker.replace(Span::new(0, 5), "const");

    let result = tracker.apply_checked("let x = 42;");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "const = 42;");
}

#[test]
fn test_tracker_apply_checked_conflict() {
    let mut tracker = ChangeTracker::new();
    tracker.replace(Span::new(0, 10), "aaa");
    tracker.replace(Span::new(5, 15), "bbb");

    let result = tracker.apply_checked("hello world testing");
    assert!(result.is_err());
}

#[test]
fn test_tracker_empty() {
    let tracker = ChangeTracker::new();

    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);

    let result = tracker.apply("unchanged");
    assert_eq!(result, "unchanged");
}

#[test]
fn test_tracker_total_delta() {
    let mut tracker = ChangeTracker::new();
    tracker.insert_before(0, "abc"); // +3
    tracker.delete(Span::new(10, 20)); // -10
    tracker.replace(Span::new(5, 7), "hello"); // +3 (5 - 2)

    assert_eq!(tracker.total_delta(), -4);
}

#[test]
fn test_tracker_clear() {
    let mut tracker = ChangeTracker::new();
    tracker.insert_before(0, "test");

    assert!(!tracker.is_empty());
    tracker.clear();
    assert!(tracker.is_empty());
}

#[test]
fn test_tracker_out_of_bounds() {
    let mut tracker = ChangeTracker::new();
    tracker.replace(Span::new(100, 200), "test");

    // Should handle gracefully, appending at end
    let result = tracker.apply("short");
    assert!(result.contains("test"));
}

#[test]
fn test_tracker_unicode() {
    let mut tracker = ChangeTracker::new();
    // "héllo" is 6 bytes (é is 2 bytes). Spans are byte-based.
    tracker.replace(Span::new(0, 6), "hello");

    let result = tracker.apply("héllo world");
    assert!(result.contains("world"));
}

#[test]
fn test_insert_after() {
    let mut tracker = ChangeTracker::new();
    tracker.insert_after(Span::new(0, 5), "!");

    let result = tracker.apply("Hello World");
    assert_eq!(result, "Hello! World");
}
