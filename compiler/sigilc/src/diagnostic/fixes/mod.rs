//! Code Fix System
//!
//! Provides extensible code fix (quick fix) functionality for diagnostics.
//! Fixes can automatically correct common errors.
//!
//! # Design
//!
//! Inspired by TypeScript's code fix system:
//! - Each fix is registered for specific error codes
//! - Fixes generate `CodeAction`s with text edits
//! - Multiple fixes can apply to the same error
//!
//! # Example
//!
//! ```ignore
//! struct AddTypeAnnotation;
//!
//! impl CodeFix for AddTypeAnnotation {
//!     fn error_codes(&self) -> &'static [ErrorCode] {
//!         &[ErrorCode::E2005] // Cannot infer type
//!     }
//!
//!     fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
//!         // Generate fix to add type annotation
//!         vec![CodeAction {
//!             title: "Add type annotation".into(),
//!             edits: vec![TextEdit::insert(span, ": Type")],
//!         }]
//!     }
//! }
//! ```

mod registry;

pub use registry::FixRegistry;

use crate::diagnostic::{Diagnostic, ErrorCode};
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

    /// Create an insertion edit (before the span start).
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
}

/// A code action that can be applied to fix an error.
#[derive(Clone, Debug)]
pub struct CodeAction {
    /// User-visible title describing the fix.
    pub title: String,
    /// Text edits to apply.
    pub edits: Vec<TextEdit>,
    /// Whether this fix is the preferred/main fix.
    pub is_preferred: bool,
}

impl CodeAction {
    /// Create a new code action.
    pub fn new(title: impl Into<String>, edits: Vec<TextEdit>) -> Self {
        CodeAction {
            title: title.into(),
            edits,
            is_preferred: false,
        }
    }

    /// Mark this as the preferred fix.
    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }

    /// Add an edit to this action.
    pub fn with_edit(mut self, edit: TextEdit) -> Self {
        self.edits.push(edit);
        self
    }
}

/// Context provided to code fixes.
#[derive(Debug)]
pub struct FixContext<'a> {
    /// The diagnostic being fixed.
    pub diagnostic: &'a Diagnostic,
    /// Source code being fixed.
    pub source: &'a str,
}

impl<'a> FixContext<'a> {
    /// Create a new fix context.
    pub fn new(diagnostic: &'a Diagnostic, source: &'a str) -> Self {
        FixContext { diagnostic, source }
    }

    /// Get the primary span from the diagnostic.
    pub fn primary_span(&self) -> Option<Span> {
        self.diagnostic.primary_span()
    }

    /// Get the text at the given span.
    pub fn text_at(&self, span: Span) -> &str {
        let start = span.start as usize;
        let end = span.end as usize;
        &self.source[start..end]
    }
}

/// Trait for implementing code fixes.
///
/// Each fix handles specific error codes and generates
/// code actions to correct the error.
pub trait CodeFix: Send + Sync {
    /// The error codes this fix applies to.
    fn error_codes(&self) -> &'static [ErrorCode];

    /// Generate fix actions for the given diagnostic.
    ///
    /// Returns an empty vec if the fix doesn't apply to this specific
    /// diagnostic (even if the error code matches).
    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction>;

    /// A unique identifier for this fix type (for debugging/testing).
    fn id(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[cfg(test)]
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
    }

    #[test]
    fn test_text_edit_delete() {
        let edit = TextEdit::delete(Span::new(10, 20));

        assert_eq!(edit.span, Span::new(10, 20));
        assert!(edit.new_text.is_empty());
        assert!(!edit.is_insert());
        assert!(edit.is_delete());
        assert!(!edit.is_replace());
    }

    #[test]
    fn test_text_edit_replace() {
        let edit = TextEdit::replace(Span::new(10, 20), "new content");

        assert_eq!(edit.span, Span::new(10, 20));
        assert_eq!(edit.new_text, "new content");
        assert!(!edit.is_insert());
        assert!(!edit.is_delete());
        assert!(edit.is_replace());
    }

    #[test]
    fn test_code_action_builder() {
        let action = CodeAction::new("Fix it", vec![])
            .with_edit(TextEdit::insert(0, "// fixed\n"))
            .preferred();

        assert_eq!(action.title, "Fix it");
        assert!(action.is_preferred);
        assert_eq!(action.edits.len(), 1);
    }

    struct MockFix;

    impl CodeFix for MockFix {
        fn error_codes(&self) -> &'static [ErrorCode] {
            &[ErrorCode::E2003]
        }

        fn get_fixes(&self, _ctx: &FixContext) -> Vec<CodeAction> {
            vec![CodeAction::new("Mock fix", vec![TextEdit::insert(0, "fix")])]
        }
    }

    #[test]
    fn test_mock_fix() {
        let fix = MockFix;

        assert_eq!(fix.error_codes(), &[ErrorCode::E2003]);

        let diag = Diagnostic::error(ErrorCode::E2003).with_message("test");
        let ctx = FixContext::new(&diag, "source code");
        let actions = fix.get_fixes(&ctx);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Mock fix");
    }

    #[test]
    fn test_fix_context() {
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("test")
            .with_label(Span::new(5, 10), "here");
        let source = "hello world";
        let ctx = FixContext::new(&diag, source);

        assert_eq!(ctx.primary_span(), Some(Span::new(5, 10)));
        assert_eq!(ctx.text_at(Span::new(0, 5)), "hello");
        assert_eq!(ctx.text_at(Span::new(6, 11)), "world");
    }
}
