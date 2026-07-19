//! Function Declaration Formatting
//!
//! Formatting for function declarations including signatures and bodies.

use crate::width::ALWAYS_STACKED;
use ori_ir::ast::items::{Function, Param, TraitBound, WhereClause};
use ori_ir::{ExprId, StringLookup, Visibility};

use super::parsed_types::{
    calculate_type_width, const_expr_render_width, format_const_expr, format_parsed_type,
};
use super::{BodyBreakPolicy, ModuleFormatter};

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a function declaration including signature and body.
    pub fn format_function(&mut self, func: &Function) {
        // Item-level conditional attributes (Spec §25.4)
        if let Some(ref target) = func.target_attr {
            self.emit_item_target_attr(target);
        }
        if let Some(ref cfg) = func.cfg_attr {
            self.emit_item_cfg_attr(cfg);
        }

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

    /// Format a function body. Delegates the inline-vs-newline-vs-internal-break
    /// decision to the shared `emit_expr_body` (SSOT in `function_body`).
    /// Function bodies use `should_break_body_to_newline`
    /// (if/for control flow + atomic-that-fits) ahead of the over-width-head guard.
    pub(super) fn format_function_body(&mut self, body: ExprId) {
        self.emit_expr_body(body, BodyBreakPolicy::AllowStructuralNewline);
    }

    /// Whether an over-width expression body breaks to its own line.
    /// True: conditionals (if-then-else) + for loops (per spec); method calls
    /// on For/If receivers; atomic exprs that cannot break internally IF
    /// breaking helps. False: exprs that break internally (lists, maps, calls
    /// with args); atomic exprs too wide for even their own line (long strings).
    pub(super) fn should_break_body_to_newline(&self, body: ExprId, body_width: usize) -> bool {
        let expr = self.arena.get_expr(body);

        match &expr.kind {
            // Per spec: If/For/While always break to newline (internal breaking structure)
            ori_ir::ExprKind::If { .. }
            | ori_ir::ExprKind::For { .. }
            | ori_ir::ExprKind::While { .. } => true,

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
                let config = self.ctx.config();
                let indent_width = config.indent_size;
                let max_width = config.max_width;
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
        let mut width_of_expr = |e| const_expr_render_width(self.arena, self.interner, e);
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                width += 2; // ", "
            }
            width += self.interner.lookup(param.name).len();
            if let Some(ref ty) = param.ty {
                width += 2; // ": "
                width += calculate_type_width(ty, self.arena, self.interner, &mut width_of_expr);
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
            let mut width_of_expr = |e| const_expr_render_width(self.arena, self.interner, e);
            width += calculate_type_width(ret_ty, self.arena, self.interner, &mut width_of_expr);
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

        // Why: short bodies (<= SHORT_BODY_THRESHOLD) break to params first —
        // breaking `x + y` as `x\n+ y` looks bad; long bodies break at natural
        // points (else, method chains), so their width is excluded here.
        let body_width = self.width_calc.width(func.body);
        if body_width != ALWAYS_STACKED && body_width <= SHORT_BODY_THRESHOLD {
            width += body_width;
        }

        width
    }

    /// Format generic parameters `<...>`; emits nothing when the list is empty.
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

    /// Format trait bounds joined with ` + ` (e.g. `A + B`).
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

    /// Format a ` where ...` clause list; emits nothing when empty.
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
