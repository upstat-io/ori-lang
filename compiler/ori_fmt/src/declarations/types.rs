//! Type Declaration Formatting
//!
//! Formatting for type declarations: structs, sum types, and newtypes.

use ori_ir::ast::items::{StructField, TypeDecl, TypeDeclKind, Variant};
use ori_ir::{Name, StringLookup, Visibility};

use super::parsed_types::{calculate_type_width, const_expr_render_width, format_parsed_type};
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a type declaration (struct, sum type, or newtype).
    pub fn format_type_decl(&mut self, type_decl: &TypeDecl) {
        // Item-level conditional attributes (Spec §25.4) — before #repr and #derive
        if let Some(ref target) = type_decl.target_attr {
            self.emit_item_target_attr(target);
        }
        if let Some(ref cfg) = type_decl.cfg_attr {
            self.emit_item_cfg_attr(cfg);
        }

        // Repr attributes (Spec §26 — between #target/#cfg and #derive in canonical order)
        for repr in &type_decl.repr_attrs {
            self.emit_repr_attr(repr);
        }

        self.emit_derive_attrs(&type_decl.derives);

        // Visibility
        if type_decl.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        self.ctx.emit("type ");
        self.ctx.emit(self.interner.lookup(type_decl.name));

        // Generic parameters
        self.format_generic_params(type_decl.generics);

        // Where clauses
        self.format_where_clauses(&type_decl.where_clauses);

        self.ctx.emit(" = ");

        // Type body
        match &type_decl.kind {
            TypeDeclKind::Struct(fields) => {
                self.format_struct_fields(fields);
                // Struct bodies end with `}` — no trailing semicolon
            }
            TypeDeclKind::Sum(variants) => {
                self.format_sum_variants(variants);
                // Sum types don't end with `}` — need trailing semicolon
                self.ctx.emit(";");
            }
            TypeDeclKind::Newtype(ty) => {
                format_parsed_type(ty, self.arena, self.interner, &mut self.ctx);
                // Newtypes don't end with `}` — need trailing semicolon
                self.ctx.emit(";");
            }
        }
    }

    fn emit_derive_attrs(&mut self, derives: &[Name]) {
        if derives.is_empty() {
            return;
        }

        let derive_names: Vec<String> = derives
            .iter()
            .map(|derive| self.interner.lookup(*derive).to_string())
            .collect();

        if !self
            .ctx
            .would_exceed_limit(derive_attr_width(&derive_names))
        {
            emit_derive_attr(&mut self.ctx, &derive_names);
            self.ctx.emit_newline_indent();
            return;
        }

        let mut start = 0;
        while start < derive_names.len() {
            let mut end = start + 1;
            while end < derive_names.len() {
                let candidate_width = derive_attr_width(&derive_names[start..=end]);
                if self.ctx.column() + candidate_width > self.ctx.max_width() {
                    break;
                }
                end += 1;
            }

            emit_derive_attr(&mut self.ctx, &derive_names[start..end]);
            self.ctx.emit_newline_indent();
            start = end;
        }
    }

    fn format_struct_fields(&mut self, fields: &[StructField]) {
        if fields.is_empty() {
            self.ctx.emit("{}");
            return;
        }

        // Calculate inline width
        let inline_width = self.calculate_struct_fields_width(fields);
        let fits_inline = self.ctx.fits(inline_width);

        if fits_inline {
            self.ctx.emit("{ ");
            for (i, field) in fields.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(field.name));
                self.ctx.emit(": ");
                format_parsed_type(&field.ty, self.arena, self.interner, &mut self.ctx);
            }
            self.ctx.emit(" }");
        } else {
            self.ctx.emit("{");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, field) in fields.iter().enumerate() {
                self.ctx.emit_indent();
                self.ctx.emit(self.interner.lookup(field.name));
                self.ctx.emit(": ");
                format_parsed_type(&field.ty, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit(",");
                if i < fields.len() - 1 {
                    self.ctx.emit_newline();
                }
            }
            self.ctx.dedent();
            self.ctx.emit_newline_indent();
            self.ctx.emit("}");
        }
    }

    fn calculate_struct_fields_width(&self, fields: &[StructField]) -> usize {
        let mut width = 4; // "{ " + " }"
        let mut width_of_expr = |e| const_expr_render_width(self.arena, self.interner, e);
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                width += 2; // ", "
            }
            width += self.interner.lookup(field.name).len();
            width += 2; // ": "
            width += calculate_type_width(&field.ty, self.arena, self.interner, &mut width_of_expr);
        }
        width
    }

    fn format_sum_variants(&mut self, variants: &[Variant]) {
        if variants.is_empty() {
            return;
        }

        // Calculate inline width
        let inline_width = self.calculate_sum_variants_width(variants);
        let fits_inline = self.ctx.fits(inline_width);

        if fits_inline && variants.len() <= 3 {
            for (i, variant) in variants.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(" | ");
                }
                self.format_variant(variant);
            }
        } else {
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, variant) in variants.iter().enumerate() {
                self.ctx.emit_indent();
                self.ctx.emit("| ");
                self.format_variant(variant);
                if i < variants.len() - 1 {
                    self.ctx.emit_newline();
                }
            }
            self.ctx.dedent();
        }
    }

    fn format_variant(&mut self, variant: &Variant) {
        self.ctx.emit(self.interner.lookup(variant.name));
        if !variant.fields.is_empty() {
            self.ctx.emit("(");
            for (i, field) in variant.fields.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(field.name));
                self.ctx.emit(": ");
                format_parsed_type(&field.ty, self.arena, self.interner, &mut self.ctx);
            }
            self.ctx.emit(")");
        }
    }

    fn calculate_sum_variants_width(&self, variants: &[Variant]) -> usize {
        let mut width = 0;
        let mut width_of_expr = |e| const_expr_render_width(self.arena, self.interner, e);
        for (i, variant) in variants.iter().enumerate() {
            if i > 0 {
                width += 3; // " | "
            }
            width += self.interner.lookup(variant.name).len();
            if !variant.fields.is_empty() {
                width += 2; // "()"
                for (j, field) in variant.fields.iter().enumerate() {
                    if j > 0 {
                        width += 2; // ", "
                    }
                    width += self.interner.lookup(field.name).len();
                    width += 2; // ": "
                    width += calculate_type_width(
                        &field.ty,
                        self.arena,
                        self.interner,
                        &mut width_of_expr,
                    );
                }
            }
        }
        width
    }
}

fn derive_attr_width(derives: &[impl AsRef<str>]) -> usize {
    "#derive()".len()
        + derives
            .iter()
            .map(|derive| derive.as_ref().len())
            .sum::<usize>()
        + derives.len().saturating_sub(1) * ", ".len()
}

fn emit_derive_attr(ctx: &mut crate::FormatContext, derives: &[impl AsRef<str>]) {
    ctx.emit("#derive(");
    for (i, derive) in derives.iter().enumerate() {
        if i > 0 {
            ctx.emit(", ");
        }
        ctx.emit(derive.as_ref());
    }
    ctx.emit(")");
}
