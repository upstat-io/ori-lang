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

            ExprKind::FunctionSeq(seq) => {
                self.emit_function_seq(seq);
            }

            ExprKind::FunctionExp(exp) => {
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
                for stmt in stmts_list {
                    self.emit_stmt(stmt);
                    self.ctx.emit_newline_indent();
                }
                if let Some(r) = result {
                    self.format(*r);
                }
            }

            // For other always-stacked constructs, use broken format
            _ => self.emit_broken(expr_id),
        }
    }

    /// Emit a `function_seq` pattern (run, try, etc.).
    pub(super) fn emit_function_seq(&mut self, seq: &ori_ir::FunctionSeq) {
        match seq {
            ori_ir::FunctionSeq::Run {
                bindings,
                result,
                span: _,
            } => {
                self.emit_seq_with_bindings("run", *bindings, *result);
            }

            ori_ir::FunctionSeq::Try {
                bindings,
                result,
                span: _,
            } => {
                self.emit_seq_with_bindings("try", *bindings, *result);
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
    /// match(scrutinee,
    ///     pattern -> body,
    ///     pattern.match(guard) -> body,
    /// )
    /// ```
    fn emit_match_construct(&mut self, scrutinee: ExprId, arms: ArmRange) {
        self.ctx.emit("match(");
        self.format(scrutinee);
        self.ctx.emit(",");
        let arms_list = self.arena.get_arms(arms);
        self.ctx.emit_newline();
        self.ctx.indent();
        for arm in arms_list {
            self.ctx.emit_indent();
            self.emit_match_pattern(&arm.pattern);
            if let Some(guard) = arm.guard {
                self.ctx.emit(".match(");
                self.format(guard);
                self.ctx.emit(")");
            }
            self.ctx.emit(" -> ");
            self.format(arm.body);
            self.ctx.emit(",");
            self.ctx.emit_newline();
        }
        self.ctx.dedent();
        self.ctx.emit_indent();
        self.ctx.emit(")");
    }

    /// Emit a sequential pattern with bindings (shared by run/try).
    ///
    /// Format:
    /// ```text
    /// keyword(
    ///     binding1,
    ///     binding2,
    ///     result,
    /// )
    /// ```
    fn emit_seq_with_bindings(&mut self, keyword: &str, bindings: SeqBindingRange, result: ExprId) {
        self.ctx.emit(keyword);
        self.ctx.emit("(");
        self.ctx.emit_newline();
        self.ctx.indent();

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
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
        self.ctx.emit(")");
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
                self.emit_binding_pattern(pattern);
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
                self.emit_binding_pattern(pattern);
                self.ctx.emit(" = ");
                self.format(*init);
            }
        }
    }
}
