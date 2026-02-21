//! Impl Block Formatting
//!
//! Formatting for impl blocks (trait impls and inherent impls).

use crate::comments::CommentIndex;
use crate::formatter::Formatter;
use ori_ir::ast::items::ImplDef;
use ori_ir::{CommentList, StringLookup};

use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format an impl block (trait impl or inherent impl).
    pub fn format_impl(&mut self, impl_def: &ImplDef) {
        self.ctx.emit("impl");

        // Generic parameters
        self.format_generic_params(impl_def.generics);

        self.ctx.emit(" ");

        // Trait path (if trait impl)
        if let Some(ref trait_path) = impl_def.trait_path {
            for (i, seg) in trait_path.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(".");
                }
                self.ctx.emit(self.interner.lookup(*seg));
            }
            self.ctx.emit(" for ");
        }

        // Self type
        format_parsed_type(&impl_def.self_ty, self.arena, self.interner, &mut self.ctx);

        // Where clauses
        self.format_where_clauses(&impl_def.where_clauses);

        // Body
        if impl_def.methods.is_empty() && impl_def.assoc_types.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            // Associated types
            for assoc in &impl_def.assoc_types {
                self.ctx.emit_indent();
                self.ctx.emit("type ");
                self.ctx.emit(self.interner.lookup(assoc.name));
                self.ctx.emit(" = ");
                format_parsed_type(&assoc.ty, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit_newline();
                self.ctx.emit_newline();
            }

            // Methods
            for (i, method) in impl_def.methods.iter().enumerate() {
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

                // Pass current column and indent level so width decisions and
                // line breaks account for full context
                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
                // Trailing semicolon for non-block expression bodies
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

    /// Format an impl block with comment preservation.
    pub fn format_impl_with_comments(
        &mut self,
        impl_def: &ImplDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        self.ctx.emit("impl");

        // Generic parameters
        self.format_generic_params(impl_def.generics);

        self.ctx.emit(" ");

        // Trait path (if trait impl)
        if let Some(ref trait_path) = impl_def.trait_path {
            for (i, seg) in trait_path.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(".");
                }
                self.ctx.emit(self.interner.lookup(*seg));
            }
            self.ctx.emit(" for ");
        }

        // Self type
        format_parsed_type(&impl_def.self_ty, self.arena, self.interner, &mut self.ctx);

        // Where clauses
        self.format_where_clauses(&impl_def.where_clauses);

        // Body
        if impl_def.methods.is_empty() && impl_def.assoc_types.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();

            // Associated types
            for assoc in &impl_def.assoc_types {
                // Emit comments before this associated type
                self.emit_comments_before_indented(assoc.span.start, comments, comment_index);
                self.ctx.emit_indent();
                self.ctx.emit("type ");
                self.ctx.emit(self.interner.lookup(assoc.name));
                self.ctx.emit(" = ");
                format_parsed_type(&assoc.ty, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit_newline();
                self.ctx.emit_newline();
            }

            // Methods
            for (i, method) in impl_def.methods.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit_newline();
                }
                // Emit comments before this method
                self.emit_comments_before_indented(method.span.start, comments, comment_index);
                self.ctx.emit_indent();
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(method.name));
                self.ctx.emit(" ");
                self.format_params(method.params);
                self.ctx.emit(" -> ");
                format_parsed_type(&method.return_ty, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit(" = ");

                // Pass current column and indent level so width decisions and
                // line breaks account for full context
                let current_column = self.ctx.column();
                let current_indent = self.ctx.indent_level();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
                // Trailing semicolon for non-block expression bodies
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
