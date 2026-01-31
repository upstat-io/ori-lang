//! Trait Definition Formatting
//!
//! Formatting for trait definitions including super traits and items.

use crate::comments::CommentIndex;
use crate::formatter::Formatter;
use ori_ir::ast::items::{TraitDef, TraitItem};
use ori_ir::{CommentList, StringLookup, Visibility};

use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a trait definition including super traits and items.
    pub fn format_trait(&mut self, trait_def: &TraitDef) {
        if trait_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("trait ");
        self.ctx.emit(self.interner.lookup(trait_def.name));

        // Generic parameters
        self.format_generic_params(trait_def.generics);

        // Super traits
        if !trait_def.super_traits.is_empty() {
            self.ctx.emit(": ");
            self.format_trait_bounds(&trait_def.super_traits);
        }

        // Body
        if trait_def.items.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, item) in trait_def.items.iter().enumerate() {
                if i > 0 && trait_def.items.len() > 1 {
                    self.ctx.emit_newline();
                }
                self.ctx.emit_indent();
                self.format_trait_item(item);
                self.ctx.emit_newline();
            }
            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }

    fn format_trait_item(&mut self, item: &TraitItem) {
        match item {
            TraitItem::MethodSig(sig) => {
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(sig.name));
                self.ctx.emit(" ");
                self.format_params(sig.params);
                self.ctx.emit(" -> ");
                format_parsed_type(&sig.return_ty, self.arena, self.interner, &mut self.ctx);
            }
            TraitItem::DefaultMethod(method) => {
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
                    Formatter::with_config(self.arena, self.interner, self.config)
                        .with_indent_level(current_indent)
                        .with_starting_column(current_column);
                expr_formatter.format(method.body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                self.ctx.emit(body_output);
            }
            TraitItem::AssocType(assoc) => {
                self.ctx.emit("type ");
                self.ctx.emit(self.interner.lookup(assoc.name));
            }
        }
    }

    /// Format a trait definition with comment preservation.
    pub fn format_trait_with_comments(
        &mut self,
        trait_def: &TraitDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        if trait_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("trait ");
        self.ctx.emit(self.interner.lookup(trait_def.name));

        // Generic parameters
        self.format_generic_params(trait_def.generics);

        // Super traits
        if !trait_def.super_traits.is_empty() {
            self.ctx.emit(": ");
            self.format_trait_bounds(&trait_def.super_traits);
        }

        // Body
        if trait_def.items.is_empty() {
            self.ctx.emit(" {}");
        } else {
            self.ctx.emit(" {");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, item) in trait_def.items.iter().enumerate() {
                if i > 0 && trait_def.items.len() > 1 {
                    self.ctx.emit_newline();
                }
                // Emit comments before this trait item
                self.emit_comments_before_indented(item.span().start, comments, comment_index);
                self.ctx.emit_indent();
                self.format_trait_item(item);
                self.ctx.emit_newline();
            }
            self.ctx.dedent();
            self.ctx.emit_indent();
            self.ctx.emit("}");
        }
    }
}
