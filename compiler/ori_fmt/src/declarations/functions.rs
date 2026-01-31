//! Function Declaration Formatting
//!
//! Formatting for function declarations including signatures and bodies.

use crate::formatter::Formatter;
use crate::width::ALWAYS_STACKED;
use ori_ir::ast::items::{Function, Param, TraitBound, WhereClause};
use ori_ir::{ExprId, StringLookup, Visibility};

use super::parsed_types::{calculate_type_width, format_parsed_type};
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a function declaration including signature and body.
    pub fn format_function(&mut self, func: &Function) {
        // Visibility
        if func.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        // Function name
        self.ctx.emit("@");
        self.ctx.emit(self.interner.lookup(func.name));

        // Generic parameters
        self.format_generic_params(func.generics);

        // Calculate trailing width (return type + capabilities + where + " = ")
        // so params can decide whether to break based on full signature
        let trailing_width = self.calculate_function_trailing_width(func);

        // Parameters
        self.ctx.emit(" ");
        self.format_params_with_trailing(func.params, trailing_width);

        // Return type
        if let Some(ref ret_ty) = func.return_ty {
            self.ctx.emit(" -> ");
            format_parsed_type(ret_ty, self.arena, self.interner, &mut self.ctx);
        }

        // Capabilities
        if !func.capabilities.is_empty() {
            self.ctx.emit(" uses ");
            for (i, cap) in func.capabilities.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.ctx.emit(self.interner.lookup(cap.name));
            }
        }

        // Where clauses
        self.format_where_clauses(&func.where_clauses);

        // Body
        self.format_function_body(func.body);
    }

    /// Format a function body, breaking to new line if it doesn't fit after `= `.
    pub(super) fn format_function_body(&mut self, body: ExprId) {
        // Calculate body width to determine if it fits inline
        let body_width = self.width_calc.width(body);

        // Check if body fits after " = " on current line
        let space_after_eq = 3; // " = "
        let fits_inline =
            body_width != ALWAYS_STACKED && self.ctx.fits(space_after_eq + body_width);

        if fits_inline {
            // Inline: " = body"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        } else if self.is_conditional(body) {
            // Conditionals break to new line: " =\n    if cond then ... else ..."
            self.ctx.emit(" =");
            self.ctx.emit_newline();
            self.ctx.indent();
            self.ctx.emit_indent();

            // Create formatter with indent level 1 for proper nested breaks
            // Use format_broken to prevent re-evaluation of fit at new position
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_indent_level(1)
                .with_starting_column(self.ctx.column());
            expr_formatter.format_broken(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
            self.ctx.dedent();
        } else {
            // Other constructs stay on same line, break internally: " = [...\n]"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        }
    }

    /// Check if an expression is a conditional (if-then-else).
    fn is_conditional(&self, body: ExprId) -> bool {
        matches!(self.arena.get_expr(body).kind, ori_ir::ExprKind::If { .. })
    }

    /// Format params without considering trailing content (for method params, etc.).
    pub(super) fn format_params(&mut self, params: ori_ir::ParamRange) {
        self.format_params_with_trailing(params, 0);
    }

    /// Format params considering trailing content width (return type, capabilities, etc.).
    /// This ensures we break params if the full signature would exceed line width.
    fn format_params_with_trailing(&mut self, params: ori_ir::ParamRange, trailing_width: usize) {
        let params_list = self.arena.get_params(params);

        if params_list.is_empty() {
            self.ctx.emit("()");
            return;
        }

        // Calculate if params + trailing content fit on one line
        let inline_width = self.calculate_params_width(params_list);
        let total_width = inline_width + trailing_width;
        let fits_inline = self.ctx.fits(total_width);

        if fits_inline {
            self.ctx.emit("(");
            for (i, param) in params_list.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                self.format_param(param);
            }
            self.ctx.emit(")");
        } else {
            self.ctx.emit("(");
            self.ctx.emit_newline();
            self.ctx.indent();
            for (i, param) in params_list.iter().enumerate() {
                self.ctx.emit_indent();
                self.format_param(param);
                self.ctx.emit(",");
                if i < params_list.len() - 1 {
                    self.ctx.emit_newline();
                }
            }
            self.ctx.dedent();
            self.ctx.emit_newline_indent();
            self.ctx.emit(")");
        }
    }

    fn format_param(&mut self, param: &Param) {
        self.ctx.emit(self.interner.lookup(param.name));
        if let Some(ref ty) = param.ty {
            self.ctx.emit(": ");
            format_parsed_type(ty, self.arena, self.interner, &mut self.ctx);
        }
    }

    fn calculate_params_width(&self, params: &[Param]) -> usize {
        let mut width = 2; // ()
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                width += 2; // ", "
            }
            width += self.interner.lookup(param.name).len();
            if let Some(ref ty) = param.ty {
                width += 2; // ": "
                width += calculate_type_width(ty, self.arena, self.interner);
            }
        }
        width
    }

    /// Calculate width of function trailing content (return type + caps + where + " = " + body).
    /// This is used to help params decide whether to break based on full signature width.
    ///
    /// Only includes body width if the body is short enough that breaking it would look ugly.
    /// Long bodies will break naturally at good points (else, operators, etc.), so we let them.
    fn calculate_function_trailing_width(&mut self, func: &Function) -> usize {
        const SHORT_BODY_THRESHOLD: usize = 20;
        let mut width = 0;

        // Return type: " -> Type"
        if let Some(ref ret_ty) = func.return_ty {
            width += 4; // " -> "
            width += calculate_type_width(ret_ty, self.arena, self.interner);
        }

        // Capabilities: " uses Cap1, Cap2"
        if !func.capabilities.is_empty() {
            width += 6; // " uses "
            for (i, cap) in func.capabilities.iter().enumerate() {
                if i > 0 {
                    width += 2; // ", "
                }
                width += self.interner.lookup(cap.name).len();
            }
        }

        // Where clauses: " where T: Trait"
        // For simplicity, estimate 20 chars if where clauses exist
        // (full calculation would be complex and rarely needed)
        if !func.where_clauses.is_empty() {
            width += 20;
        }

        // " = " prefix for body
        width += 3;

        // Only include body width if it's short enough that breaking it would be ugly.
        // Short expressions like `x + y` look bad when broken (`x\n+ y`), so we prefer
        // to break params first. Longer expressions will break at natural points
        // (conditionals at else, chains at method calls, etc.) which is fine.
        let body_width = self.width_calc.width(func.body);
        if body_width != ALWAYS_STACKED && body_width <= SHORT_BODY_THRESHOLD {
            width += body_width;
        }

        width
    }

    pub(super) fn format_generic_params(&mut self, generics: ori_ir::GenericParamRange) {
        let generics_list = self.arena.get_generic_params(generics);
        if generics_list.is_empty() {
            return;
        }

        self.ctx.emit("<");
        for (i, param) in generics_list.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.ctx.emit(self.interner.lookup(param.name));
            if !param.bounds.is_empty() {
                self.ctx.emit(": ");
                self.format_trait_bounds(&param.bounds);
            }
        }
        self.ctx.emit(">");
    }

    pub(super) fn format_trait_bounds(&mut self, bounds: &[TraitBound]) {
        for (i, bound) in bounds.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(" + ");
            }
            self.format_trait_bound(bound);
        }
    }

    fn format_trait_bound(&mut self, bound: &TraitBound) {
        self.ctx.emit(self.interner.lookup(bound.first));
        for seg in &bound.rest {
            self.ctx.emit(".");
            self.ctx.emit(self.interner.lookup(*seg));
        }
    }

    pub(super) fn format_where_clauses(&mut self, where_clauses: &[WhereClause]) {
        if where_clauses.is_empty() {
            return;
        }

        self.ctx.emit(" where ");
        for (i, clause) in where_clauses.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.ctx.emit(self.interner.lookup(clause.param));
            if let Some(proj) = clause.projection {
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(proj));
            }
            self.ctx.emit(": ");
            self.format_trait_bounds(&clause.bounds);
        }
    }
}
