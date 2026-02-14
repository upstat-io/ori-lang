//! Function Declaration Formatting
//!
//! Formatting for function declarations including signatures and bodies.

use crate::formatter::Formatter;
use crate::width::ALWAYS_STACKED;
use ori_ir::ast::items::{Function, Param, TraitBound, WhereClause};
use ori_ir::{ExprId, StringLookup, Visibility};

use super::parsed_types::{calculate_type_width, format_const_expr, format_parsed_type};
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
        } else if self.should_break_body_to_newline(body, body_width) {
            // Break to newline when:
            // 1. Body is a control flow expr (if/for) - per spec
            // 2. Body is atomic but would fit on its own line with standard indent
            // Don't break if body is wider than available space anyway (e.g., long strings)
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

    /// Check if an expression should break to a new line when it doesn't fit.
    ///
    /// Returns true for:
    /// 1. Conditionals (if-then-else) and for loops - per spec, these break to newline
    /// 2. Method calls on For/If receivers - the receiver needs to break
    /// 3. Atomic expressions that cannot break internally, IF breaking would help
    ///
    /// Returns false for:
    /// - Expressions that can break internally (lists, maps, calls with args)
    /// - Atomic expressions too wide for even their own line (e.g., long strings)
    fn should_break_body_to_newline(&self, body: ExprId, body_width: usize) -> bool {
        let expr = self.arena.get_expr(body);

        match &expr.kind {
            // Per spec: If/For always break to newline (they have internal breaking structure)
            ori_ir::ExprKind::If { .. } | ori_ir::ExprKind::For { .. } => true,

            // Method calls: break if receiver is If/For (needs to break internally)
            ori_ir::ExprKind::MethodCall { receiver, args, .. } => {
                let args_empty = self.arena.get_expr_list(*args).is_empty();
                let receiver_is_complex = matches!(
                    &self.arena.get_expr(*receiver).kind,
                    ori_ir::ExprKind::If { .. } | ori_ir::ExprKind::For { .. }
                );
                // Break if receiver is complex with empty args, or recurse
                (args_empty && receiver_is_complex)
                    || self.should_break_body_to_newline(*receiver, body_width)
            }
            ori_ir::ExprKind::MethodCallNamed { receiver, args, .. } => {
                let args_empty = self.arena.get_call_args(*args).is_empty();
                let receiver_is_complex = matches!(
                    &self.arena.get_expr(*receiver).kind,
                    ori_ir::ExprKind::If { .. } | ori_ir::ExprKind::For { .. }
                );
                (args_empty && receiver_is_complex)
                    || self.should_break_body_to_newline(*receiver, body_width)
            }

            // Atomic expressions: only break if it would actually help
            // (body would fit on its own line at indent level 1)
            ori_ir::ExprKind::Int(_)
            | ori_ir::ExprKind::Float(_)
            | ori_ir::ExprKind::Bool(_)
            | ori_ir::ExprKind::String(_)
            | ori_ir::ExprKind::Char(_)
            | ori_ir::ExprKind::Unit
            | ori_ir::ExprKind::Duration { .. }
            | ori_ir::ExprKind::Size { .. }
            | ori_ir::ExprKind::Ident(_)
            | ori_ir::ExprKind::Const(_)
            | ori_ir::ExprKind::SelfRef
            | ori_ir::ExprKind::FunctionRef(_)
            | ori_ir::ExprKind::HashLength
            | ori_ir::ExprKind::Cast { .. }
            | ori_ir::ExprKind::Unary { .. } => {
                // Only break if body would fit on its own line
                let indent_width = self.config.indent_size;
                let max_width = self.config.max_width;
                body_width != ALWAYS_STACKED && body_width + indent_width <= max_width
            }

            // Everything else can break internally (calls, lists, maps, binary ops, etc.)
            _ => false,
        }
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
            if param.is_const {
                // Const generic parameter: `$N: int`, `$N: int = 10`
                self.ctx.emit("$");
                self.ctx.emit(self.interner.lookup(param.name));
                if let Some(ref ct) = param.const_type {
                    self.ctx.emit(": ");
                    format_parsed_type(ct, self.arena, self.interner, &mut self.ctx);
                }
                if let Some(dv) = param.default_value {
                    self.ctx.emit(" = ");
                    format_const_expr(dv, self.arena, self.interner, &mut self.ctx);
                }
            } else {
                // Type generic parameter: `T`, `T: Bound`, `T = DefaultType`
                self.ctx.emit(self.interner.lookup(param.name));
                if !param.bounds.is_empty() {
                    self.ctx.emit(": ");
                    self.format_trait_bounds(&param.bounds);
                }
                if let Some(ref default_ty) = param.default_type {
                    self.ctx.emit(" = ");
                    format_parsed_type(default_ty, self.arena, self.interner, &mut self.ctx);
                }
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
            match clause {
                WhereClause::TypeBound {
                    param,
                    projection,
                    bounds,
                    ..
                } => {
                    self.ctx.emit(self.interner.lookup(*param));
                    if let Some(proj) = projection {
                        self.ctx.emit(".");
                        self.ctx.emit(self.interner.lookup(*proj));
                    }
                    self.ctx.emit(": ");
                    self.format_trait_bounds(bounds);
                }
                WhereClause::ConstBound { expr, .. } => {
                    format_const_expr(*expr, self.arena, self.interner, &mut self.ctx);
                }
            }
        }
    }
}
