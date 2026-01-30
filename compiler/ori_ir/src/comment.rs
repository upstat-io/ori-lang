//! Comment types for the Ori lexer and formatter.
//!
//! Provides comment representation with all Salsa-required traits (Clone, Eq, Hash, Debug).
//! Comments are captured separately from tokens to allow the parser to work without
//! comment awareness while preserving comments for formatting.

use super::{Name, Span};
use std::fmt;
use std::hash::{Hash, Hasher};

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
/// - `@param` Parameter documentation
/// - `@field` Field documentation
/// - `!` Warning/panic documentation
/// - `>` Example code
///
/// The formatter uses this to reorder doc comments into canonical order.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CommentKind {
    /// Regular comment: `// any text`
    Regular,
    /// Description doc comment: `// #Description text`
    DocDescription,
    /// Parameter doc comment: `// @param name description`
    DocParam,
    /// Field doc comment: `// @field name description`
    DocField,
    /// Warning/panic doc comment: `// !Warning text`
    DocWarning,
    /// Example doc comment: `// >example() -> result`
    DocExample,
}

impl CommentKind {
    /// Get the sort order for doc comment kinds.
    /// Lower numbers should appear first.
    ///
    /// Order: Description(0) -> Param/Field(1) -> Warning(2) -> Example(3)
    /// Regular comments have order 100 to sort after doc comments.
    #[inline]
    pub fn sort_order(self) -> u8 {
        match self {
            CommentKind::DocDescription => 0,
            CommentKind::DocParam | CommentKind::DocField => 1,
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
mod tests {
    use super::*;
    use crate::StringInterner;

    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn test_comment_creation() {
        let interner = test_interner();
        let content = interner.intern("This is a comment");
        let span = Span::new(0, 20);

        let comment = Comment::regular(content, span);

        assert_eq!(comment.kind, CommentKind::Regular);
        assert_eq!(comment.span, span);
    }

    #[test]
    fn test_comment_kind_sort_order() {
        // Description comes first
        assert!(CommentKind::DocDescription.sort_order() < CommentKind::DocParam.sort_order());
        // Param/Field come before Warning
        assert!(CommentKind::DocParam.sort_order() < CommentKind::DocWarning.sort_order());
        // Warning comes before Example
        assert!(CommentKind::DocWarning.sort_order() < CommentKind::DocExample.sort_order());
        // Regular comments come last
        assert!(CommentKind::DocExample.sort_order() < CommentKind::Regular.sort_order());
    }

    #[test]
    fn test_comment_kind_is_doc() {
        assert!(!CommentKind::Regular.is_doc());
        assert!(CommentKind::DocDescription.is_doc());
        assert!(CommentKind::DocParam.is_doc());
        assert!(CommentKind::DocField.is_doc());
        assert!(CommentKind::DocWarning.is_doc());
        assert!(CommentKind::DocExample.is_doc());
    }

    #[test]
    fn test_comment_list_operations() {
        let interner = test_interner();
        let mut list = CommentList::new();
        assert!(list.is_empty());

        let c1 = Comment::regular(interner.intern("comment 1"), Span::new(0, 10));
        let c2 = Comment::regular(interner.intern("comment 2"), Span::new(20, 30));

        list.push(c1);
        list.push(c2);

        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
    }

    #[test]
    fn test_comment_list_comments_before() {
        let interner = test_interner();
        let comments = CommentList::from_vec(vec![
            Comment::regular(interner.intern("c1"), Span::new(0, 10)),
            Comment::regular(interner.intern("c2"), Span::new(15, 25)),
            Comment::regular(interner.intern("c3"), Span::new(30, 40)),
        ]);

        let before_20: Vec<_> = comments.comments_before(20).collect();
        assert_eq!(before_20.len(), 1);

        let before_30: Vec<_> = comments.comments_before(30).collect();
        assert_eq!(before_30.len(), 2);
    }

    #[test]
    fn test_comment_list_hash() {
        use std::collections::HashSet;
        let interner = test_interner();
        let mut set = HashSet::new();

        let list1 = CommentList::from_vec(vec![Comment::regular(
            interner.intern("test"),
            Span::new(0, 10),
        )]);
        let list2 = CommentList::from_vec(vec![Comment::regular(
            interner.intern("test"),
            Span::new(0, 10),
        )]);
        let list3 = CommentList::from_vec(vec![Comment::regular(
            interner.intern("other"),
            Span::new(0, 10),
        )]);

        set.insert(list1);
        set.insert(list2); // same as list1
        set.insert(list3);

        assert_eq!(set.len(), 2);
    }
}
