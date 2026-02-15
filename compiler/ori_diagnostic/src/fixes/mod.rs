//! Code Fix System
//!
//! Provides extensible code fix (quick fix) functionality for diagnostics.
//! Fixes can automatically correct common errors.
//!
//! # Design
//!
//! - Each fix is registered for specific error codes
//! - Fixes generate `CodeAction`s with text edits
//! - Multiple fixes can apply to the same error
//!
//! # Example
//!
//! ```text
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

use ori_ir::Span;

use crate::{Diagnostic, ErrorCode};

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
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
    #[must_use]
    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }

    /// Add an edit to this action.
    #[must_use]
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
mod tests;
