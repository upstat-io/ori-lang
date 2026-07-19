//! Impl Block Formatting
//!
//! Formatting for impl blocks (trait impls and inherent impls).

use crate::comments::CommentIndex;
use ori_ir::ast::items::ImplDef;
use ori_ir::{CommentList, StringLookup};

use super::parsed_types::format_parsed_type;
use super::{BodyBreakPolicy, ModuleFormatter};

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format an impl block (trait impl or inherent impl).
    pub fn format_impl(&mut self, impl_def: &ImplDef) {
        self.format_impl_inner(impl_def, None, None);
    }

    /// Format an impl block with comment preservation.
    pub fn format_impl_with_comments(
        &mut self,
        impl_def: &ImplDef,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        self.format_impl_inner(impl_def, Some(comments), Some(comment_index));
    }

    /// Emit the impl header: item attributes, `impl`, generics, and the
    /// subject-first colon form `impl Type: Trait[<args>]` (grammar.ebnf `trait_impl`)
    /// followed by any where-clauses. Shared by both formatting paths.
    fn format_impl_header(&mut self, impl_def: &ImplDef) {
        // Spec: Clause 25.4.
        if let Some(ref target) = impl_def.target_attr {
            self.emit_item_target_attr(target);
        }
        if let Some(ref cfg) = impl_def.cfg_attr {
            self.emit_item_cfg_attr(cfg);
        }
        self.ctx.emit("impl");

        // Generic parameters
        self.format_generic_params(impl_def.generics);

        self.ctx.emit(" ");

        // Self type (subject) first.
        format_parsed_type(&impl_def.self_ty, self.arena, self.interner, &mut self.ctx);

        // Trait (if trait impl): `: Trait[<args>]`.
        if let Some(ref trait_path) = impl_def.trait_path {
            self.ctx.emit(": ");
            for (i, seg) in trait_path.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(".");
                }
                self.ctx.emit(self.interner.lookup(*seg));
            }
            let trait_args = self.arena.get_parsed_type_list(impl_def.trait_type_args);
            if !trait_args.is_empty() {
                self.ctx.emit("<");
                for (i, arg_id) in trait_args.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let arg = self.arena.get_parsed_type(*arg_id);
                    format_parsed_type(arg, self.arena, self.interner, &mut self.ctx);
                }
                self.ctx.emit(">");
            }
        }

        // Where clauses
        self.format_where_clauses(&impl_def.where_clauses);
    }

    /// Shared impl-block formatting. When `comments` + `comment_index` are
    /// `Some`, emits preserved comments before each associated type and method;
    /// when `None`, formats without comment preservation.
    fn format_impl_inner(
        &mut self,
        impl_def: &ImplDef,
        comments: Option<&CommentList>,
        mut comment_index: Option<&mut CommentIndex>,
    ) {
        self.format_impl_header(impl_def);

        // Body
        if impl_def.methods.is_empty() && impl_def.assoc_types.is_empty() {
            self.ctx.emit(" {}");
            return;
        }
        self.ctx.emit(" {");
        self.ctx.emit_newline();
        self.ctx.indent();

        // Associated types
        for assoc in &impl_def.assoc_types {
            if let (Some(comments), Some(comment_index)) = (comments, comment_index.as_deref_mut())
            {
                self.emit_comments_before_indented(assoc.span.start, comments, comment_index);
            }
            self.ctx.emit_indent();
            self.ctx.emit("type ");
            self.ctx.emit(self.interner.lookup(assoc.name));
            self.ctx.emit(" = ");
            format_parsed_type(&assoc.ty, self.arena, self.interner, &mut self.ctx);
            self.ctx.emit_newline();
            self.ctx.emit_newline();
        }

        // Methods
        for method in &impl_def.methods {
            if let (Some(comments), Some(comment_index)) = (comments, comment_index.as_deref_mut())
            {
                self.emit_comments_before_indented(method.span.start, comments, comment_index);
            }
            self.ctx.emit_indent();
            self.ctx.emit("@");
            self.ctx.emit(self.interner.lookup(method.name));
            self.ctx.emit(" ");
            self.format_params(method.params);
            self.ctx.emit(" -> ");
            format_parsed_type(&method.return_ty, self.arena, self.interner, &mut self.ctx);
            self.emit_expr_body(method.body, BodyBreakPolicy::OverflowOnly);
            self.ctx.emit_newline();
        }

        self.ctx.dedent();
        self.ctx.emit_indent();
        self.ctx.emit("}");
    }
}
