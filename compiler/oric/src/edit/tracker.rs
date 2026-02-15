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
//! ```text
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
        if self.edits.is_empty() {
            return None;
        }

        // Use index-based sorting to avoid cloning the entire Vec
        let mut indices: Vec<usize> = (0..self.edits.len()).collect();
        indices.sort_by_key(|&i| {
            let e = &self.edits[i];
            (e.span.start, e.span.end)
        });

        for window in indices.windows(2) {
            let e1 = &self.edits[window[0]];
            let e2 = &self.edits[window[1]];

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

        // Use index-based sorting to avoid cloning the entire Vec
        let mut indices: Vec<usize> = (0..self.edits.len()).collect();
        indices.sort_by(|&a, &b| {
            let ea = &self.edits[a];
            let eb = &self.edits[b];
            // Sort by start position descending, then end position descending
            eb.span
                .start
                .cmp(&ea.span.start)
                .then(eb.span.end.cmp(&ea.span.end))
        });

        let mut result = source.to_string();

        for &idx in &indices {
            let edit = &self.edits[idx];
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
mod tests;
