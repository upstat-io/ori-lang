//! Stacked Formatting
//!
//! Methods for emitting always-stacked constructs (run, try, match, etc.)
//! that always render in multi-line format.

use ori_ir::{ArmRange, ExprId, ExprKind, SeqBinding, SeqBindingRange, StringLookup};

use super::Formatter;

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit an always-stacked construct (run, try, match, etc.).
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
                    self.ctx.emit_newline_indent();
                    self.format(*result);
                }
                self.ctx.dedent();
                self.ctx.emit_newline_indent();
                self.ctx.emit("}");
            }

            // For other always-stacked constructs, use broken format
            _ => self.emit_broken(expr_id),
        }
    }

    /// Emit a `function_seq` pattern (run, try, etc.).
    pub(super) fn emit_function_seq(&mut self, seq: &ori_ir::FunctionSeq) {
        match seq {
            // TODO(§0.10-cleanup): FunctionSeq::Run is dead — parser no longer produces Run nodes.
            // Remove when IR variant is removed (see roadmap section-00-parser.md § 0.10).
            ori_ir::FunctionSeq::Run {
                pre_checks,
                bindings,
                result,
                post_checks,
                span: _,
            } => {
                self.emit_run_with_checks(*pre_checks, *bindings, *result, *post_checks);
            }

            ori_ir::FunctionSeq::Try {
                bindings,
                result,
                span: _,
            } => {
                self.emit_try_block(*bindings, *result);
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
        let arm_count = arms_list.len();
        self.ctx.indent();
        for (i, arm) in arms_list.iter().enumerate() {
            self.ctx.emit_newline_indent();
            self.emit_match_pattern(&arm.pattern);
            if let Some(guard) = arm.guard {
                self.ctx.emit(" if ");
                self.format(guard);
            }
            self.ctx.emit(" -> ");
            self.format(arm.body);
            if i + 1 < arm_count {
                self.ctx.emit(",");
            }
        }
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
        self.ctx.emit("}");
    }

    /// Emit a run pattern with optional pre/post checks.
    ///
    /// TODO(§0.10-cleanup): Dead code — parser no longer produces `FunctionSeq::Run`.
    /// Remove when IR variant is removed (see roadmap section-00-parser.md § 0.10).
    fn emit_run_with_checks(
        &mut self,
        pre_checks: ori_ir::CheckRange,
        bindings: SeqBindingRange,
        result: ExprId,
        post_checks: ori_ir::CheckRange,
    ) {
        self.ctx.emit("run");
        self.ctx.emit("(");
        self.ctx.emit_newline();
        self.ctx.indent();

        for check in self.arena.get_checks(pre_checks) {
            self.ctx.emit_indent();
            self.ctx.emit("pre_check: ");
            self.format(check.expr);
            if let Some(msg) = check.message {
                self.ctx.emit(" | ");
                self.format(msg);
            }
            self.ctx.emit(",");
            self.ctx.emit_newline();
        }

        let bindings_list = self.arena.get_seq_bindings(bindings);
        for binding in bindings_list {
            self.ctx.emit_indent();
            self.emit_seq_binding(binding);
            self.ctx.emit(",");
            self.ctx.emit_newline();
        }

        self.ctx.emit_indent();
        self.format(result);
        self.ctx.emit(",");

        for check in self.arena.get_checks(post_checks) {
            self.ctx.emit_newline();
            self.ctx.emit_indent();
            self.ctx.emit("post_check: ");
            self.format(check.expr);
            if let Some(msg) = check.message {
                self.ctx.emit(" | ");
                self.format(msg);
            }
            self.ctx.emit(",");
        }

        self.ctx.dedent();
        self.ctx.emit_newline_indent();
        self.ctx.emit(")");
    }

    /// Emit `try { bindings; result }` using block syntax.
    fn emit_try_block(&mut self, bindings: SeqBindingRange, result: ExprId) {
        self.ctx.emit("try {");
        self.ctx.indent();

        let bindings_list = self.arena.get_seq_bindings(bindings);
        for binding in bindings_list {
            self.ctx.emit_newline_indent();
            self.emit_seq_binding(binding);
            self.ctx.emit(";");
        }

        if result.is_present() {
            self.ctx.emit_newline_indent();
            self.format(result);
        }

        self.ctx.dedent();
        self.ctx.emit_newline_indent();
        self.ctx.emit("}");
    }

    /// Emit a sequence binding.
    fn emit_seq_binding(&mut self, binding: &SeqBinding) {
        match binding {
            // Per spec: mutable is default, $ prefix for immutable
            SeqBinding::Let {
                pattern,
                ty: _,
                value,
                mutable,
                span: _,
            } => {
                if *mutable {
                    self.ctx.emit("let ");
                } else {
                    self.ctx.emit("let $");
                }
                let pat = self.arena.get_binding_pattern(*pattern);
                self.emit_binding_pattern(pat);
                self.ctx.emit(" = ");
                self.format(*value);
            }
            SeqBinding::Stmt { expr, span: _ } => {
                self.format(*expr);
            }
        }
    }

    /// Emit a statement.
    pub(super) fn emit_stmt(&mut self, stmt: &ori_ir::Stmt) {
        match &stmt.kind {
            ori_ir::StmtKind::Expr(expr) => self.format(*expr),
            // Per spec: mutable is default, $ prefix for immutable
            ori_ir::StmtKind::Let {
                pattern,
                ty: _,
                init,
                mutable,
            } => {
                if *mutable {
                    self.ctx.emit("let ");
                } else {
                    self.ctx.emit("let $");
                }
                let pat = self.arena.get_binding_pattern(*pattern);
                self.emit_binding_pattern(pat);
                self.ctx.emit(" = ");
                self.format(*init);
            }
        }
    }
}
