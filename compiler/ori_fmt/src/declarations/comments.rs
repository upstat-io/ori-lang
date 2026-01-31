//! Comment Emission
//!
//! Methods for emitting comments before declarations.

use crate::comments::{format_comment, CommentIndex};
use ori_ir::ast::items::{Function, TypeDecl, TypeDeclKind};
use ori_ir::{CommentList, StringLookup};

use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Emit comments that should appear before a given position.
    pub fn emit_comments_before(
        &mut self,
        pos: u32,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let indices = comment_index.take_comments_before(pos);
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Emit comments that should appear before a function, with @param reordering.
    pub fn emit_comments_before_function(
        &mut self,
        func: &Function,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        // Get param names from the function
        let params_list = self.arena.get_params(func.params);
        let param_names: Vec<&str> = params_list
            .iter()
            .map(|p| self.interner.lookup(p.name))
            .collect();

        let indices = comment_index.take_comments_before_function(
            func.span.start,
            &param_names,
            comments,
            self.interner,
        );
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Emit comments that should appear before a type, with @field reordering.
    pub fn emit_comments_before_type(
        &mut self,
        type_decl: &TypeDecl,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        // Get field names from struct type, if applicable
        let field_names: Vec<&str> = match &type_decl.kind {
            TypeDeclKind::Struct(fields) => fields
                .iter()
                .map(|f| self.interner.lookup(f.name))
                .collect(),
            _ => Vec::new(),
        };

        let indices = comment_index.take_comments_before_type(
            type_decl.span.start,
            &field_names,
            comments,
            self.interner,
        );
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    /// Emit any remaining comments at the end of the file.
    pub(super) fn emit_trailing_comments(
        &mut self,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let indices = comment_index.remaining_indices();
        if !indices.is_empty() {
            // Add a blank line before trailing comments
            self.ctx.emit_newline();
            for idx in indices {
                let comment = &comments[idx];
                self.ctx.emit(&format_comment(comment, self.interner));
                self.ctx.emit_newline();
            }
        }
    }

    /// Emit comments that should appear before a given position, with indentation.
    pub(super) fn emit_comments_before_indented(
        &mut self,
        pos: u32,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let indices = comment_index.take_comments_before(pos);
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit_indent();
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }
}
