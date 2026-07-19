//! Parsed module output and post-parse metadata analysis.

use ori_ir::{Module, ModuleExtra, SharedArena};

use crate::{DetachmentReason, ParseError, ParseWarning};

/// Parsed module, arena, diagnostics, and formatting metadata.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseOutput {
    pub module: Module,
    pub arena: SharedArena,
    pub errors: Vec<ParseError>,
    pub warnings: Vec<ParseWarning>,
    pub metadata: ModuleExtra,
}

impl ParseOutput {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Append diagnostics for doc comments detached from declarations.
    pub fn check_detached_doc_comments(&mut self) {
        let mut declaration_starts = Vec::new();
        declaration_starts.extend(self.module.functions.iter().map(|item| item.span.start));
        declaration_starts.extend(self.module.tests.iter().map(|item| item.span.start));
        declaration_starts.extend(self.module.types.iter().map(|item| item.span.start));
        declaration_starts.extend(self.module.traits.iter().map(|item| item.span.start));
        declaration_starts.extend(self.module.impls.iter().map(|item| item.span.start));
        declaration_starts.extend(
            self.module
                .extension_imports
                .iter()
                .map(|item| item.span.start),
        );
        declaration_starts.sort_unstable();

        for comment in self.metadata.unattached_doc_comments(&declaration_starts) {
            let reason = declaration_starts
                .iter()
                .find(|&&start| start > comment.span.end)
                .map_or(DetachmentReason::NoFollowingDeclaration, |&start| {
                    if self
                        .metadata
                        .has_blank_line_between(comment.span.end, start)
                    {
                        DetachmentReason::BlankLine
                    } else if self.metadata.has_comment_between(comment.span.end, start) {
                        DetachmentReason::RegularCommentInterrupting
                    } else {
                        DetachmentReason::TooFarFromDeclaration
                    }
                });
            self.warnings
                .push(ParseWarning::detached_doc_comment(comment.span, reason));
        }
    }
}
