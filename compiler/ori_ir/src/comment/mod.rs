//! Comment types for the Ori lexer and formatter.
//!
//! Provides comment representation with all Salsa-required traits (Clone, Eq, Hash, Debug).
//! Comments are captured separately from tokens to allow the parser to work without
//! comment awareness while preserving comments for formatting.

use std::fmt;
use std::hash::{Hash, Hasher};

use super::{Name, Span};

/// A source comment with its span and content.
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Comment {
    /// The content of the comment (excluding the `//` prefix).
    /// Interned for efficient equality checking.
    pub content: Name,
    /// The span in the source covering the entire comment including `//`.
    pub span: Span,
    /// The kind of comment (regular, doc description, doc param, etc.)
    pub kind: CommentKind,
}

impl Comment {
    /// Create a new comment.
    #[inline]
    pub fn new(content: Name, span: Span, kind: CommentKind) -> Self {
        Comment {
            content,
            span,
            kind,
        }
    }

    /// Create a regular comment.
    #[inline]
    pub fn regular(content: Name, span: Span) -> Self {
        Comment::new(content, span, CommentKind::Regular)
    }
}

impl fmt::Debug for Comment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {} ({:?})", self.content, self.span, self.kind)
    }
}

/// The kind of comment, distinguishing regular comments from doc comments.
///
/// Doc comments have special markers that affect formatting order:
/// - `#` Description (must come first)
/// - `* name:` Member (parameter/field) documentation
/// - `!` Warning/panic documentation
/// - `>` Example code
///
/// Legacy `@param` and `@field` markers are also recognized and classified
/// as `DocMember`.
///
/// The formatter uses this to reorder doc comments into canonical order.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CommentKind {
    /// Regular comment: `// any text`
    Regular,
    /// Description doc comment: `// #Description text`
    DocDescription,
    /// Member doc comment: `// * name: description`
    ///
    /// Also produced by legacy `@param` and `@field` markers.
    /// Works for both function parameters and struct fields.
    DocMember,
    /// Warning/panic doc comment: `// !Warning text`
    DocWarning,
    /// Example doc comment: `// >example() -> result`
    DocExample,
}

impl CommentKind {
    /// Get the sort order for doc comment kinds.
    /// Lower numbers should appear first.
    ///
    /// Order: Description(0) -> Member(1) -> Warning(2) -> Example(3)
    /// Regular comments have order 100 to sort after doc comments.
    #[inline]
    pub fn sort_order(self) -> u8 {
        match self {
            CommentKind::DocDescription => 0,
            CommentKind::DocMember => 1,
            CommentKind::DocWarning => 2,
            CommentKind::DocExample => 3,
            CommentKind::Regular => 100,
        }
    }

    /// Check if this is any kind of doc comment.
    #[inline]
    pub fn is_doc(self) -> bool {
        !matches!(self, CommentKind::Regular)
    }
}

/// A list of comments with Salsa-compatible traits.
///
/// Wraps `Vec<Comment>` with Clone, Eq, Hash support.
/// Comments are stored in source order (by span start position).
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`, `Default`
#[derive(Clone, Eq, PartialEq, Default)]
pub struct CommentList {
    comments: Vec<Comment>,
}

impl CommentList {
    /// Create a new empty comment list.
    #[inline]
    pub fn new() -> Self {
        CommentList {
            comments: Vec::new(),
        }
    }

    /// Create from a Vec of comments.
    #[inline]
    pub fn from_vec(comments: Vec<Comment>) -> Self {
        CommentList { comments }
    }

    /// Push a comment.
    #[inline]
    pub fn push(&mut self, comment: Comment) {
        self.comments.push(comment);
    }

    /// Get the number of comments.
    #[inline]
    pub fn len(&self) -> usize {
        self.comments.len()
    }

    /// Check if empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    /// Get comment at index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&Comment> {
        self.comments.get(index)
    }

    /// Get a slice of all comments.
    #[inline]
    pub fn as_slice(&self) -> &[Comment] {
        &self.comments
    }

    /// Iterate over comments.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Comment> {
        self.comments.iter()
    }

    /// Consume into Vec.
    #[inline]
    pub fn into_vec(self) -> Vec<Comment> {
        self.comments
    }

    /// Find comments that appear before a given position.
    ///
    /// Returns comments whose end position is less than or equal to `pos`.
    /// This is useful for finding leading comments before a declaration.
    pub fn comments_before(&self, pos: u32) -> impl Iterator<Item = &Comment> {
        self.comments.iter().filter(move |c| c.span.end <= pos)
    }

    /// Find comments within a span range.
    ///
    /// Returns comments that start within the given span.
    pub fn comments_in_span(&self, span: Span) -> impl Iterator<Item = &Comment> {
        self.comments
            .iter()
            .filter(move |c| c.span.start >= span.start && c.span.start < span.end)
    }
}

impl Hash for CommentList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.comments.hash(state);
    }
}

impl fmt::Debug for CommentList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommentList({} comments)", self.comments.len())
    }
}

impl std::ops::Index<usize> for CommentList {
    type Output = Comment;

    fn index(&self, index: usize) -> &Self::Output {
        &self.comments[index]
    }
}

impl IntoIterator for CommentList {
    type Item = Comment;
    type IntoIter = std::vec::IntoIter<Comment>;

    fn into_iter(self) -> Self::IntoIter {
        self.comments.into_iter()
    }
}

impl<'a> IntoIterator for &'a CommentList {
    type Item = &'a Comment;
    type IntoIter = std::slice::Iter<'a, Comment>;

    fn into_iter(self) -> Self::IntoIter {
        self.comments.iter()
    }
}

#[cfg(test)]
mod tests;
