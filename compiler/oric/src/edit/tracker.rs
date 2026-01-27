//! Change Tracker
//!
//! Tracks and applies text edits to source code for building and applying code fixes.
//!
//! # Design
//!
//! Edits are accumulated and then applied in reverse order (from end to start)
//! to avoid invalidating earlier spans when text is inserted or deleted.
//!
//! # Example
//!
//! ```ignore
//! let mut tracker = ChangeTracker::new();
//! tracker.insert_before(Span::new(10, 10), "// comment\n");
//! tracker.replace(Span::new(20, 25), "newValue");
//! tracker.delete(Span::new(30, 35));
//!
//! let result = tracker.apply("original source code...");
//! ```

use crate::ir::Span;

/// A text edit that modifies source code.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TextEdit {
    /// The span to replace (empty span for insert).
    pub span: Span,
    /// The new text to insert.
    pub new_text: String,
}

impl TextEdit {
    /// Create a replacement edit.
    pub fn replace(span: Span, new_text: impl Into<String>) -> Self {
        TextEdit {
            span,
            new_text: new_text.into(),
        }
    }

    /// Create an insertion edit at a specific position.
    pub fn insert(at: u32, text: impl Into<String>) -> Self {
        TextEdit {
            span: Span::new(at, at),
            new_text: text.into(),
        }
    }

    /// Create a deletion edit.
    pub fn delete(span: Span) -> Self {
        TextEdit {
            span,
            new_text: String::new(),
        }
    }

    /// Check if this edit is an insertion.
    pub fn is_insert(&self) -> bool {
        self.span.start == self.span.end && !self.new_text.is_empty()
    }

    /// Check if this edit is a deletion.
    pub fn is_delete(&self) -> bool {
        self.new_text.is_empty() && self.span.start != self.span.end
    }

    /// Check if this edit is a replacement.
    pub fn is_replace(&self) -> bool {
        !self.is_insert() && !self.is_delete()
    }

    /// Get the length change this edit would cause.
    ///
    /// Positive = text grows, negative = text shrinks.
    pub fn length_delta(&self) -> i64 {
        let removed = i64::from(self.span.end - self.span.start);
        let added = i64::try_from(self.new_text.len()).unwrap_or(i64::MAX);
        added - removed
    }
}

/// Error when edits conflict (overlap).
#[derive(Clone, Debug)]
pub struct EditConflict {
    pub edit1: TextEdit,
    pub edit2: TextEdit,
}

impl std::fmt::Display for EditConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "edits overlap: {:?} and {:?}",
            self.edit1.span, self.edit2.span
        )
    }
}

impl std::error::Error for EditConflict {}

/// Tracks and applies text edits.
///
/// Edits are collected and then applied in a single pass, sorted from
/// end to start to avoid invalidating earlier positions.
#[derive(Clone, Debug, Default)]
pub struct ChangeTracker {
    edits: Vec<TextEdit>,
}

impl ChangeTracker {
    /// Create a new empty change tracker.
    pub fn new() -> Self {
        ChangeTracker { edits: Vec::new() }
    }

    /// Replace text at the given span.
    pub fn replace(&mut self, span: Span, text: impl Into<String>) {
        self.edits.push(TextEdit::replace(span, text));
    }

    /// Insert text before the given position.
    pub fn insert_before(&mut self, at: u32, text: impl Into<String>) {
        self.edits.push(TextEdit::insert(at, text));
    }

    /// Insert text after the given span.
    pub fn insert_after(&mut self, span: Span, text: impl Into<String>) {
        self.edits.push(TextEdit::insert(span.end, text));
    }

    /// Delete text at the given span.
    pub fn delete(&mut self, span: Span) {
        self.edits.push(TextEdit::delete(span));
    }

    /// Add a raw text edit.
    pub fn push(&mut self, edit: TextEdit) {
        self.edits.push(edit);
    }

    /// Get the number of pending edits.
    pub fn len(&self) -> usize {
        self.edits.len()
    }

    /// Check if there are no pending edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Clear all pending edits.
    pub fn clear(&mut self) {
        self.edits.clear();
    }

    /// Get the pending edits.
    pub fn edits(&self) -> &[TextEdit] {
        &self.edits
    }

    /// Check for overlapping edits.
    ///
    /// Returns the first conflict found, if any.
    pub fn check_conflicts(&self) -> Option<EditConflict> {
        let mut sorted = self.edits.clone();
        sorted.sort_by_key(|e| (e.span.start, e.span.end));

        for window in sorted.windows(2) {
            let e1 = &window[0];
            let e2 = &window[1];

            // Two edits conflict if they overlap (not just touch)
            // Insert at same position is OK (they're independent)
            if e1.span.end > e2.span.start && !(e1.is_insert() && e2.is_insert()) {
                return Some(EditConflict {
                    edit1: e1.clone(),
                    edit2: e2.clone(),
                });
            }
        }

        None
    }

    /// Apply all edits to the source and return the modified text.
    ///
    /// Edits are applied from end to start to preserve positions.
    pub fn apply(&self, source: &str) -> String {
        if self.edits.is_empty() {
            return source.to_string();
        }

        // Sort edits by position (end to start for reverse application)
        let mut sorted = self.edits.clone();
        sorted.sort_by(|a, b| {
            // Sort by start position descending, then end position descending
            b.span
                .start
                .cmp(&a.span.start)
                .then(b.span.end.cmp(&a.span.end))
        });

        let mut result = source.to_string();

        for edit in sorted {
            let start = edit.span.start as usize;
            let end = edit.span.end as usize;

            // Clamp to valid range
            let start = start.min(result.len());
            let end = end.min(result.len()).max(start);

            result.replace_range(start..end, &edit.new_text);
        }

        result
    }

    /// Apply all edits, returning an error if there are conflicts.
    pub fn apply_checked(&self, source: &str) -> Result<String, EditConflict> {
        if let Some(conflict) = self.check_conflicts() {
            return Err(conflict);
        }
        Ok(self.apply(source))
    }

    /// Calculate the total length change from all edits.
    pub fn total_delta(&self) -> i64 {
        self.edits.iter().map(TextEdit::length_delta).sum()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
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
}
