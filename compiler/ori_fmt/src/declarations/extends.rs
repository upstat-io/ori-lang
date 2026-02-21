//! Extension block formatting.
//!
//! Formats `extend<T> Type { ... }` blocks.

use crate::comments::CommentIndex;
use crate::formatter::Formatter;
use ori_ir::ast::items::ExtendDef;
use ori_ir::{CommentList, StringLookup};

use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format an extension block.
    pub(crate) fn format_extend(&mut self, extend: &ExtendDef) {
        self.ctx.emit("extend");

        // Generic parameters
        self.format_generic_params(extend.generics);

        self.ctx.emit(" ");

        // Target type
        format_parsed_type(&extend.target_ty, self.arena, self.interner, &mut self.ctx);

        // Where clauses
        self.format_where_clauses(&extend.where_clauses);

        // Body
        if extend.methods.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            for (i, method) in extend.methods.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit_newline();
                }
                self.ctx.emit_indent();
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(method.name));
                self.ctx.emit(" ");
                self.format_params(method.params);
                self.ctx.emit(" -> ");
                format_parsed_type(&method.return_ty, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit(" = ");

                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
                if !body_output.ends_with('}') {
                    self.ctx.emit(";");
                }
                self.ctx.emit_newline();
            }

            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }

    /// Format an extension block with comment preservation.
    pub(super) fn format_extend_with_comments(
        &mut self,
        extend: &ExtendDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        self.ctx.emit("extend");

        // Generic parameters
        self.format_generic_params(extend.generics);

        self.ctx.emit(" ");

        // Target type
        format_parsed_type(&extend.target_ty, self.arena, self.interner, &mut self.ctx);

        // Where clauses
        self.format_where_clauses(&extend.where_clauses);

        // Body
        if extend.methods.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            for (i, method) in extend.methods.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit_newline();
                }
                self.emit_comments_before_indented(method.span.start, comments, comment_index);
                self.ctx.emit_indent();
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(method.name));
                self.ctx.emit(" ");
                self.format_params(method.params);
                self.ctx.emit(" -> ");
                format_parsed_type(&method.return_ty, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit(" = ");

                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
                if !body_output.ends_with('}') {
                    self.ctx.emit(";");
                }
                self.ctx.emit_newline();
            }

            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }
}
