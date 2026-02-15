//! Parse warnings (non-fatal diagnostics).

use ori_diagnostic::{Diagnostic, ErrorCode};
use ori_ir::Span;

/// Reason why a doc comment is detached from any declaration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DetachmentReason {
    /// A blank line separates the comment from the next declaration.
    BlankLine,
    /// A regular (non-doc) comment interrupts between this doc comment
    /// and the declaration.
    RegularCommentInterrupting,
    /// The doc comment appears at end of file with no following declaration.
    NoFollowingDeclaration,
    /// Multiple blank lines or other content separates from declaration.
    TooFarFromDeclaration,
}

impl DetachmentReason {
    /// Get a user-friendly hint explaining why the comment is detached.
    pub fn hint(&self) -> &'static str {
        match self {
            DetachmentReason::BlankLine => {
                "There's a blank line between this doc comment and the next \
                 declaration. Remove the blank line to attach the comment."
            }
            DetachmentReason::RegularCommentInterrupting => {
                "A regular comment (`//`) appears between this doc comment and \
                 the declaration. Doc comments must be immediately before the \
                 declaration they document."
            }
            DetachmentReason::NoFollowingDeclaration => {
                "This doc comment isn't followed by any declaration. Doc comments \
                 should appear immediately before functions, types, or other \
                 declarations."
            }
            DetachmentReason::TooFarFromDeclaration => {
                "This doc comment is too far from the next declaration. Move it \
                 directly above the item you want to document."
            }
        }
    }
}

/// A parse warning (non-fatal diagnostic).
///
/// Warnings don't prevent compilation but indicate potential issues
/// like detached doc comments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ParseWarning {
    /// A doc comment that isn't attached to any declaration.
    DetachedDocComment {
        /// Location of the doc comment.
        span: Span,
        /// Why the comment is considered detached.
        reason: DetachmentReason,
    },
    /// An unknown calling convention string in an `extern` block.
    UnknownCallingConvention {
        /// Location of the convention string literal.
        span: Span,
        /// The convention string that was used.
        convention: String,
    },
}

impl ParseWarning {
    /// Create a warning for a detached doc comment.
    pub fn detached_doc_comment(span: Span, reason: DetachmentReason) -> Self {
        ParseWarning::DetachedDocComment { span, reason }
    }

    /// Get the span of the warning.
    pub fn span(&self) -> Span {
        match self {
            ParseWarning::DetachedDocComment { span, .. }
            | ParseWarning::UnknownCallingConvention { span, .. } => *span,
        }
    }

    /// Get a title for the warning.
    pub fn title(&self) -> &'static str {
        match self {
            ParseWarning::DetachedDocComment { .. } => "DETACHED DOC COMMENT",
            ParseWarning::UnknownCallingConvention { .. } => "UNKNOWN CALLING CONVENTION",
        }
    }

    /// Get the warning message.
    pub fn message(&self) -> String {
        match self {
            ParseWarning::DetachedDocComment { reason, .. } => {
                format!(
                    "This doc comment isn't attached to any declaration. {}",
                    reason.hint()
                )
            }
            ParseWarning::UnknownCallingConvention { convention, .. } => {
                format!("unknown calling convention \"{convention}\"; expected \"c\" or \"js\"")
            }
        }
    }

    /// Convert to a diagnostic for display.
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            ParseWarning::DetachedDocComment { .. } => Diagnostic::warning(ErrorCode::W1001)
                .with_message(self.message())
                .with_label(self.span(), "detached doc comment"),
            ParseWarning::UnknownCallingConvention { convention, .. } => {
                Diagnostic::warning(ErrorCode::W1002)
                    .with_message(self.message())
                    .with_label(self.span(), format!("unknown convention \"{convention}\""))
            }
        }
    }
}
