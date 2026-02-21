//! Default implementation block formatting.
//!
//! Formats `def impl Trait { ... }` blocks.

use crate::comments::CommentIndex;
use crate::formatter::Formatter;
use ori_ir::ast::items::DefImplDef;
use ori_ir::{CommentList, StringLookup, Visibility};

use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a default implementation block.
    pub(crate) fn format_def_impl(&mut self, def_impl: &DefImplDef) {
        if def_impl.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("def impl ");
        self.ctx.emit(self.interner.lookup(def_impl.trait_name));

        // Body
        if def_impl.methods.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            for (i, method) in def_impl.methods.iter().enumerate() {
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

    /// Format a default implementation block with comment preservation.
    pub(super) fn format_def_impl_with_comments(
        &mut self,
        def_impl: &DefImplDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        if def_impl.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("def impl ");
        self.ctx.emit(self.interner.lookup(def_impl.trait_name));

        // Body
        if def_impl.methods.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            for (i, method) in def_impl.methods.iter().enumerate() {
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
