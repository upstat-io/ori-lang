//! Module metadata for formatting and IDE support.
//!
//! Provides `ModuleExtra` for preserving non-semantic information:
//! - Comments (via `CommentList`)
//! - Blank line positions (for formatting preservation)
//! - Newline positions (for line counting)
//! - Trailing comma positions (for style preservation)
//!
//! This metadata enables lossless roundtrip formatting and rich IDE features.
//!
//! # Salsa Compatibility
//! All types derive required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`

use std::fmt;
use std::hash::{Hash, Hasher};

use super::{Comment, CommentKind, CommentList, Span};

/// Non-semantic metadata collected during parsing.
///
/// This struct holds formatting-relevant information that the parser would
/// otherwise discard. It enables:
/// - Lossless roundtrip formatting
/// - Doc comment display in IDE hover
/// - Intentional blank line preservation
/// - Style-consistent trailing comma handling
///
/// # Usage
///
/// Created during parsing and returned alongside the AST:
/// ```ignore
/// let output = parse(source);
/// let comments_above = output.metadata.doc_comments_for(fn_start);
/// ```
#[derive(Clone, Eq, PartialEq, Default)]
pub struct ModuleExtra {
    /// All comments in the module, in source order.
    pub comments: CommentList,

    /// Byte positions where blank lines occur.
    ///
    /// A "blank line" is detected when two newlines appear with only
    /// whitespace between them. This tracks the position of the second newline.
    pub blank_lines: Vec<u32>,

    /// Byte positions of all newlines.
    ///
    /// Used for line counting and determining if content spans multiple lines.
    pub newlines: Vec<u32>,

    /// Byte positions of trailing commas.
    ///
    /// Tracks commas that appear before closing delimiters (], ), }).
    /// Used by the formatter to preserve user's trailing comma style.
    pub trailing_commas: Vec<u32>,
}

impl ModuleExtra {
    /// Create a new empty metadata container.
    #[inline]
    pub fn new() -> Self {
        ModuleExtra::default()
    }

    /// Create with pre-allocated capacity based on source length.
    ///
    /// Heuristics:
    /// - ~1 comment per 200 bytes
    /// - ~1 newline per 40 bytes
    /// - ~1 blank line per 400 bytes
    pub fn with_capacity(source_len: usize) -> Self {
        ModuleExtra {
            comments: CommentList::new(),
            blank_lines: Vec::with_capacity(source_len / 400),
            newlines: Vec::with_capacity(source_len / 40),
            trailing_commas: Vec::with_capacity(source_len / 200),
        }
    }

    /// Check if there's a blank line between two positions.
    ///
    /// Returns `true` if any blank line position falls strictly between
    /// `start` and `end`.
    #[inline]
    pub fn has_blank_line_between(&self, start: u32, end: u32) -> bool {
        self.blank_lines.iter().any(|&pos| pos > start && pos < end)
    }

    /// Check if there's a regular (non-doc) comment between two positions.
    pub fn has_comment_between(&self, start: u32, end: u32) -> bool {
        self.comments.iter().any(|c| {
            c.span.start > start && c.span.end < end && matches!(c.kind, CommentKind::Regular)
        })
    }

    /// Get doc comments that should attach to a declaration at `decl_start`.
    ///
    /// Returns doc comments that:
    /// 1. End before `decl_start`
    /// 2. Have no blank line between them and the declaration
    /// 3. Are doc comments (not regular comments)
    ///
    /// Comments are returned in source order.
    pub fn doc_comments_for(&self, decl_start: u32) -> Vec<&Comment> {
        // Find the last blank line before decl_start
        let last_blank = self
            .blank_lines
            .iter()
            .filter(|&&pos| pos < decl_start)
            .max()
            .copied();

        // Find the last regular comment before decl_start
        let last_regular = self
            .comments
            .iter()
            .filter(|c| c.span.end <= decl_start && matches!(c.kind, CommentKind::Regular))
            .map(|c| c.span.end)
            .max();

        // The barrier is the latest of blank line or regular comment (if any)
        let barrier = match (last_blank, last_regular) {
            (Some(b), Some(r)) => Some(b.max(r)),
            (Some(b), None) => Some(b),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        };

        // Return doc comments that end before decl_start and start after the barrier
        self.comments
            .iter()
            .filter(|c| {
                if !c.kind.is_doc() {
                    return false;
                }
                if c.span.end > decl_start {
                    return false;
                }
                // If there's a barrier, comment must start after it
                match barrier {
                    Some(b) => c.span.start > b,
                    None => true, // No barrier, all doc comments attach
                }
            })
            .collect()
    }

    /// Get all comments immediately before a position.
    ///
    /// Returns comments whose end position is at most `gap` bytes before `pos`.
    /// Default gap of 2 accounts for typical whitespace between comment and code.
    pub fn comments_immediately_before(&self, pos: u32, gap: u32) -> Vec<&Comment> {
        self.comments
            .iter()
            .filter(|c| c.span.end <= pos && c.span.end + gap >= pos)
            .collect()
    }

    /// Get the line number for a byte position.
    ///
    /// Returns 1-indexed line number. Position 0 is line 1.
    pub fn line_number(&self, pos: u32) -> usize {
        // Count newlines before this position
        self.newlines.iter().filter(|&&nl| nl < pos).count() + 1
    }

    /// Check if a span covers multiple lines.
    pub fn is_multiline(&self, span: Span) -> bool {
        self.newlines
            .iter()
            .any(|&nl| nl > span.start && nl < span.end)
    }

    /// Add a blank line position.
    #[inline]
    pub fn add_blank_line(&mut self, pos: u32) {
        self.blank_lines.push(pos);
    }

    /// Add a newline position.
    #[inline]
    pub fn add_newline(&mut self, pos: u32) {
        self.newlines.push(pos);
    }

    /// Add a trailing comma position.
    #[inline]
    pub fn add_trailing_comma(&mut self, pos: u32) {
        self.trailing_commas.push(pos);
    }

    /// Check if there's a trailing comma at the given position.
    #[inline]
    pub fn has_trailing_comma(&self, pos: u32) -> bool {
        self.trailing_commas.contains(&pos)
    }

    /// Get unattached doc comments.
    ///
    /// Returns doc comments that weren't consumed by any declaration.
    /// Useful for generating "detached doc comment" warnings.
    ///
    /// `declaration_starts` should be a sorted list of byte positions where
    /// declarations begin.
    pub fn unattached_doc_comments(&self, declaration_starts: &[u32]) -> Vec<&Comment> {
        self.comments
            .iter()
            .filter(|c| {
                if !c.kind.is_doc() {
                    return false;
                }

                // Check if any declaration is close enough to claim this comment
                let comment_end = c.span.end;

                // Find the next declaration after this comment
                let next_decl = declaration_starts
                    .iter()
                    .find(|&&start| start > comment_end);

                match next_decl {
                    Some(&decl_start) => {
                        // Check if there's a barrier between comment and declaration
                        self.has_blank_line_between(comment_end, decl_start)
                            || self.has_comment_between(comment_end, decl_start)
                    }
                    None => {
                        // No declaration after this comment - it's definitely unattached
                        true
                    }
                }
            })
            .collect()
    }

    /// Merge another `ModuleExtra` into this one.
    ///
    /// Used when combining metadata from multiple parse passes or files.
    pub fn merge(&mut self, other: ModuleExtra) {
        for comment in other.comments {
            self.comments.push(comment);
        }
        self.blank_lines.extend(other.blank_lines);
        self.newlines.extend(other.newlines);
        self.trailing_commas.extend(other.trailing_commas);
    }
}

impl Hash for ModuleExtra {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.comments.hash(state);
        self.blank_lines.hash(state);
        self.newlines.hash(state);
        self.trailing_commas.hash(state);
    }
}

impl fmt::Debug for ModuleExtra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleExtra")
            .field("comments", &self.comments.len())
            .field("blank_lines", &self.blank_lines.len())
            .field("newlines", &self.newlines.len())
            .field("trailing_commas", &self.trailing_commas.len())
            .finish()
    }
}

#[cfg(test)]
mod tests;
