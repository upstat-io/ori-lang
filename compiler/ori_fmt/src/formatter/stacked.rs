//! Stacked Formatting
//!
//! Methods for emitting always-stacked constructs (try, match, etc.)
//! that always render in multi-line format.

use ori_ir::{ArmRange, ExprId, ExprKind, StmtRange, StringLookup};

use super::Formatter;

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit an always-stacked construct (try, match, etc.).
    ///
    /// **Invariant:** This match is exhaustive with no wildcard `_ =>` arm.
    /// Every `ExprKind` variant is listed explicitly so that adding a new variant
    /// causes a compile error. The `stacked_dispatch_has_no_wildcard` test enforces this.
    ///
    /// Variant groups:
    /// - **Custom stacked**: Block, Match, `FunctionSeq`, `FunctionExp` — multi-line rendering
    /// - **Custom broken**: Compound expressions → `emit_broken()` for line-breaking logic
    /// - **Leaf/atom + simple compound**: → `emit_inline()` (no structure to stack)
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind stacked formatting dispatch"
    )]
    pub(super) fn emit_stacked(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Match { scrutinee, arms } => {
                self.emit_match_construct(*scrutinee, *arms);
            }

            ExprKind::FunctionSeq(seq_id) => {
                let seq = self.arena.get_function_seq(*seq_id);
                self.emit_function_seq(seq);
            }

            ExprKind::FunctionExp(exp_id) => {
                let exp = self.arena.get_function_exp(*exp_id);
                self.ctx.emit(exp.kind.name());
                self.ctx.emit("(");
                let props = self.arena.get_named_exprs(exp.props);
                if !props.is_empty() {
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    for (i, prop) in props.iter().enumerate() {
                        self.ctx.emit_indent();
                        self.ctx.emit(self.interner.lookup(prop.name));
                        self.ctx.emit(": ");
                        self.format(prop.value);
                        self.ctx.emit(",");
                        if i < props.len() - 1 {
                            self.ctx.emit_newline();
                        }
                    }
                    self.ctx.dedent();
                    self.ctx.emit_newline_indent();
                }
                self.ctx.emit(")");
            }

            ExprKind::Block { stmts, result } => {
                let stmts_list = self.arena.get_stmt_range(*stmts);
                self.ctx.emit("{");
                self.ctx.indent();
                for stmt in stmts_list {
                    self.ctx.emit_newline_indent();
                    self.emit_stmt(stmt);
                    self.ctx.emit(";");
                }
                if result.is_present() {
                    // Blank line before result when 2+ statements precede it
                    if stmts_list.len() >= 2 {
                        self.ctx.emit_newline();
                    }
                    self.ctx.emit_newline_indent();
                    self.format(*result);
                }
                self.ctx.dedent();
                self.ctx.emit_newline_indent();
                self.ctx.emit("}");
            }

            // Compound expressions with custom broken rendering
            ExprKind::Binary { .. }
            | ExprKind::Call { .. }
            | ExprKind::CallNamed { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::MethodCallNamed { .. }
            | ExprKind::List(_)
            | ExprKind::Map(_)
            | ExprKind::MapWithSpread(_)
            | ExprKind::ListWithSpread(_)
            | ExprKind::Struct { .. }
            | ExprKind::StructWithSpread { .. }
            | ExprKind::Tuple(_)
            | ExprKind::If { .. }
            | ExprKind::Let { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::WithCapability { .. }
            | ExprKind::For { .. } => self.emit_broken(expr_id),

            // Inline-adequate: leaf/atoms and simple compounds
            //
            // Leaf/atoms
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::Ident(_)
            | ExprKind::Const(_)
            | ExprKind::SelfRef
            | ExprKind::FunctionRef(_)
            | ExprKind::HashLength
            | ExprKind::None
            | ExprKind::TemplateFull(_)
            | ExprKind::Error
            // Simple compounds
            | ExprKind::Unary { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index { .. }
            | ExprKind::Ok(_)
            | ExprKind::Err(_)
            | ExprKind::Some(_)
            | ExprKind::Break { .. }
            | ExprKind::Continue { .. }
            | ExprKind::Await(_)
            | ExprKind::Try(_)
            | ExprKind::Cast { .. }
            | ExprKind::Assign { .. }
            | ExprKind::Loop { .. }
            | ExprKind::Range { .. }
            | ExprKind::TemplateLiteral { .. } => self.emit_inline(expr_id),
        }
    }

    /// Emit a `function_seq` pattern (try, match, etc.).
    pub(super) fn emit_function_seq(&mut self, seq: &ori_ir::FunctionSeq) {
        match seq {
            ori_ir::FunctionSeq::Try {
                stmts,
                result,
                span: _,
            } => {
                self.emit_try_block(*stmts, *result);
            }

            ori_ir::FunctionSeq::Match {
                scrutinee,
                arms,
                span: _,
            } => {
                self.emit_match_construct(*scrutinee, *arms);
            }

            ori_ir::FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default,
                span: _,
            } => {
                self.ctx.emit("for(");
                self.ctx.emit_newline();
                self.ctx.indent();

                self.ctx.emit_indent();
                self.ctx.emit("over: ");
                self.format(*over);
                self.ctx.emit(",");
                self.ctx.emit_newline();

                if let Some(m) = map {
                    self.ctx.emit_indent();
                    self.ctx.emit("map: ");
                    self.format(*m);
                    self.ctx.emit(",");
                    self.ctx.emit_newline();
                }

                self.ctx.emit_indent();
                self.ctx.emit("match: ");
                self.emit_match_pattern(&arm.pattern);
                self.ctx.emit(" -> ");
                self.format(arm.body);
                self.ctx.emit(",");
                self.ctx.emit_newline();

                self.ctx.emit_indent();
                self.ctx.emit("default: ");
                self.format(*default);
                self.ctx.emit(",");

                self.ctx.dedent();
                self.ctx.emit_newline_indent();
                self.ctx.emit(")");
            }
        }
    }

    /// Emit a match construct (shared by `ExprKind::Match` and `FunctionSeq::Match`).
    ///
    /// Format:
    /// ```text
    /// match scrutinee {
    ///     pattern -> body,
    ///     pattern if guard -> body,
    /// }
    /// ```
    fn emit_match_construct(&mut self, scrutinee: ExprId, arms: ArmRange) {
        self.ctx.emit("match ");
        self.format(scrutinee);
        self.ctx.emit(" {");
        let arms_list = self.arena.get_arms(arms);
        self.ctx.indent();
        for arm in arms_list {
            self.ctx.emit_newline_indent();
            self.emit_match_pattern(&arm.pattern);
            if let Some(guard) = arm.guard {
                self.ctx.emit(" if ");
                self.format(guard);
            }
            self.ctx.emit(" -> ");
            self.format(arm.body);
            self.ctx.emit(",");
        }
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
        self.ctx.emit("}");
    }

    /// Emit `try { stmts; result }` using block syntax.
    fn emit_try_block(&mut self, stmts: StmtRange, result: ExprId) {
        self.ctx.emit("try {");
        self.ctx.indent();

        let stmts_list = self.arena.get_stmt_range(stmts);
        for stmt in stmts_list {
            self.ctx.emit_newline_indent();
            self.emit_stmt(stmt);
            self.ctx.emit(";");
        }

        if result.is_present() {
            // Blank line before result when 2+ statements precede it
            if stmts_list.len() >= 2 {
                self.ctx.emit_newline();
            }
            self.ctx.emit_newline_indent();
            self.format(result);
        }

        self.ctx.dedent();
        self.ctx.emit_newline_indent();
        self.ctx.emit("}");
    }

    /// Emit a statement.
    pub(super) fn emit_stmt(&mut self, stmt: &ori_ir::Stmt) {
        match &stmt.kind {
            ori_ir::StmtKind::Expr(expr) => self.format(*expr),
            // Per spec: mutable is default, $ prefix for immutable
            // The $ prefix is emitted by emit_binding_pattern(), not here
            ori_ir::StmtKind::Let {
                pattern,
                ty: _,
                init,
                mutable: _,
            } => {
                self.ctx.emit("let ");
                let pat = self.arena.get_binding_pattern(*pattern);
                self.emit_binding_pattern(pat);
                self.ctx.emit(" = ");
                self.format(*init);
            }
        }
    }
}
