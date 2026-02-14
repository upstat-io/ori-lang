//! Import Statement Formatting
//!
//! Formatting for use/import declarations.

use crate::comments::{format_comment, CommentIndex};
use ori_ir::ast::items::{ExtensionImport, UseDef, UseItem};
use ori_ir::{CommentList, StringLookup, Visibility};

use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format import declarations, grouping stdlib imports before relative imports.
    pub(super) fn format_imports(&mut self, imports: &[UseDef]) {
        // Group imports: stdlib first, then relative
        let (stdlib, relative): (Vec<_>, Vec<_>) = imports
            .iter()
            .partition(|u| matches!(u.path, ori_ir::ast::items::ImportPath::Module(_)));

        // Format stdlib imports
        for import in &stdlib {
            self.format_use(import);
            self.ctx.emit_newline();
        }

        // Blank line between stdlib and relative if both exist
        if !stdlib.is_empty() && !relative.is_empty() {
            self.ctx.emit_newline();
        }

        // Format relative imports
        for import in &relative {
            self.format_use(import);
            self.ctx.emit_newline();
        }
    }

    /// Format import declarations with comments.
    pub(super) fn format_imports_with_comments(
        &mut self,
        imports: &[UseDef],
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        // Group imports: stdlib first, then relative
        let (stdlib, relative): (Vec<_>, Vec<_>) = imports
            .iter()
            .partition(|u| matches!(u.path, ori_ir::ast::items::ImportPath::Module(_)));

        // Format stdlib imports
        for import in &stdlib {
            self.emit_comments_before_import(import.span.start, comments, comment_index);
            self.format_use(import);
            self.ctx.emit_newline();
        }

        // Blank line between stdlib and relative if both exist
        if !stdlib.is_empty() && !relative.is_empty() {
            self.ctx.emit_newline();
        }

        // Format relative imports
        for import in &relative {
            self.emit_comments_before_import(import.span.start, comments, comment_index);
            self.format_use(import);
            self.ctx.emit_newline();
        }
    }

    fn emit_comments_before_import(
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

    fn format_use(&mut self, use_def: &UseDef) {
        // Visibility
        if use_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("use ");

        // Path
        match &use_def.path {
            ori_ir::ast::items::ImportPath::Relative(name) => {
                self.ctx.emit("\"");
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit("\"");
            }
            ori_ir::ast::items::ImportPath::Module(segments) => {
                for (i, seg) in segments.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(".");
                    }
                    self.ctx.emit(self.interner.lookup(*seg));
                }
            }
        }

        // Module alias or items
        if let Some(alias) = use_def.module_alias {
            self.ctx.emit(" as ");
            self.ctx.emit(self.interner.lookup(alias));
        } else if !use_def.items.is_empty() {
            self.ctx.emit(" { ");
            self.format_use_items(&use_def.items);
            self.ctx.emit(" }");
        }
    }

    fn format_use_items(&mut self, items: &[UseItem]) {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            if item.is_constant {
                self.ctx.emit("$");
            } else if item.is_private {
                self.ctx.emit("::");
            }
            self.ctx.emit(self.interner.lookup(item.name));
            if item.without_def {
                self.ctx.emit(" without def");
            }
            if let Some(alias) = item.alias {
                self.ctx.emit(" as ");
                self.ctx.emit(self.interner.lookup(alias));
            }
        }
    }

    /// Format extension import declarations.
    pub(super) fn format_extension_imports(&mut self, ext_imports: &[ExtensionImport]) {
        for ext_import in ext_imports {
            self.format_extension_import(ext_import);
            self.ctx.emit_newline();
        }
    }

    fn format_extension_import(&mut self, ext: &ExtensionImport) {
        if ext.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("extension ");

        // Path
        match &ext.path {
            ori_ir::ast::items::ImportPath::Relative(name) => {
                self.ctx.emit("\"");
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit("\"");
            }
            ori_ir::ast::items::ImportPath::Module(segments) => {
                for (i, seg) in segments.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(".");
                    }
                    self.ctx.emit(self.interner.lookup(*seg));
                }
            }
        }

        // Extension items: { Type.method, Type.method }
        if !ext.items.is_empty() {
            self.ctx.emit(" { ");
            for (i, item) in ext.items.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(item.type_name));
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(item.method_name));
            }
            self.ctx.emit(" }");
        }
    }
}
