//! Broken Formatting
//!
//! Methods for emitting expressions in broken (multi-line) format.
//! Used when expressions don't fit on a single line.

use crate::width::ALWAYS_STACKED;
use ori_ir::{ExprId, ExprKind, StringLookup};

use super::{binary_op_str, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit an expression in broken (multi-line) format.
    pub(super) fn emit_broken(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Binary expression - break before operator
            ExprKind::Binary { op, left, right } => {
                self.format(*left);
                self.ctx.emit_newline_indent();
                self.ctx.emit(binary_op_str(*op));
                self.ctx.emit_space();
                self.format(*right);
            }

            // Calls - one argument per line
            ExprKind::Call { func, args } => {
                self.emit_inline(*func);
                self.ctx.emit("(");
                self.emit_broken_expr_list(*args);
                self.ctx.emit(")");
            }
            ExprKind::CallNamed { func, args } => {
                self.emit_inline(*func);
                self.ctx.emit("(");
                self.emit_broken_call_args(*args);
                self.ctx.emit(")");
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.format_receiver(*receiver);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*method));
                self.ctx.emit("(");
                self.emit_broken_expr_list(*args);
                self.ctx.emit(")");
            }
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => {
                self.format_receiver(*receiver);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*method));
                self.ctx.emit("(");
                self.emit_broken_call_args(*args);
                self.ctx.emit(")");
            }

            // Collections - one item per line for complex, wrap for simple
            ExprKind::List(items) => {
                let items_list = self.arena.get_expr_list(*items);
                if items_list.is_empty() {
                    self.ctx.emit("[]");
                } else {
                    self.ctx.emit("[");
                    self.emit_broken_list(items_list);
                    self.ctx.emit("]");
                }
            }
            ExprKind::Map(entries) => {
                let entries_list = self.arena.get_map_entries(*entries);
                if entries_list.is_empty() {
                    self.ctx.emit("{}");
                } else {
                    self.ctx.emit("{");
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    for (i, entry) in entries_list.iter().enumerate() {
                        self.ctx.emit_indent();
                        self.format(entry.key);
                        self.ctx.emit(": ");
                        self.format(entry.value);
                        self.ctx.emit(",");
                        if i < entries_list.len() - 1 {
                            self.ctx.emit_newline();
                        }
                    }
                    self.ctx.dedent();
                    self.ctx.emit_newline_indent();
                    self.ctx.emit("}");
                }
            }
            ExprKind::Struct { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" {");
                let fields_list = self.arena.get_field_inits(*fields);
                if fields_list.is_empty() {
                    self.ctx.emit("}");
                } else {
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    for (i, field) in fields_list.iter().enumerate() {
                        self.ctx.emit_indent();
                        self.ctx.emit(self.interner.lookup(field.name));
                        if let Some(value) = field.value {
                            self.ctx.emit(": ");
                            self.format(value);
                        }
                        self.ctx.emit(",");
                        if i < fields_list.len() - 1 {
                            self.ctx.emit_newline();
                        }
                    }
                    self.ctx.dedent();
                    self.ctx.emit_newline_indent();
                    self.ctx.emit("}");
                }
            }
            ExprKind::Tuple(items) => {
                let items_list = self.arena.get_expr_list(*items);
                if items_list.is_empty() {
                    self.ctx.emit("()");
                } else {
                    self.ctx.emit("(");
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    for (i, item) in items_list.iter().enumerate() {
                        self.ctx.emit_indent();
                        self.format(*item);
                        self.ctx.emit(",");
                        if i < items_list.len() - 1 {
                            self.ctx.emit_newline();
                        }
                    }
                    self.ctx.dedent();
                    self.ctx.emit_newline_indent();
                    self.ctx.emit(")");
                }
            }

            // If - break at else, keeping "else if" chains flat
            // Check if the initial "if cond then branch" segment fits on current line
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                // Calculate width of initial segment: "if " + cond + " then " + branch
                let cond_width = self.width_calc.width(*cond);
                let then_width = self.width_calc.width(*then_branch);

                // Check if the initial segment fits
                // 3 = "if ", 6 = " then "
                let initial_fits = cond_width != ALWAYS_STACKED
                    && then_width != ALWAYS_STACKED
                    && self.ctx.fits(3 + cond_width + 6 + then_width);

                if initial_fits {
                    // Emit "if cond then branch" inline, then break for else
                    self.ctx.emit("if ");
                    self.emit_inline(*cond);
                    self.ctx.emit(" then ");
                    self.emit_inline(*then_branch);
                } else {
                    // Initial segment is too long, break the then_branch to new line
                    self.ctx.emit("if ");
                    self.format(*cond);
                    self.ctx.emit(" then");
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    self.ctx.emit_indent();
                    self.format(*then_branch);
                    self.ctx.dedent();
                }

                if let Some(else_id) = else_branch {
                    self.emit_else_branch(*else_id);
                }
            }

            // Let binding
            // Note: mutable is default, immutable uses $ prefix in pattern
            ExprKind::Let {
                pattern,
                ty: _,
                init,
                mutable: _,
            } => {
                self.ctx.emit("let ");
                self.emit_binding_pattern(pattern);
                self.ctx.emit(" =");
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                self.format(*init);
                self.ctx.dedent();
            }

            // Lambda with body on new line
            ExprKind::Lambda {
                params,
                ret_ty: _,
                body,
            } => {
                let params_list = self.arena.get_params(*params);
                if params_list.len() == 1 {
                    self.ctx.emit(self.interner.lookup(params_list[0].name));
                } else {
                    self.ctx.emit("(");
                    for (i, param) in params_list.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit(self.interner.lookup(param.name));
                    }
                    self.ctx.emit(")");
                }
                self.ctx.emit(" ->");
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                self.format(*body);
                self.ctx.dedent();
            }

            // With capability - body on new line
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => {
                self.ctx.emit("with ");
                self.ctx.emit(self.interner.lookup(*capability));
                self.ctx.emit(" = ");
                self.format(*provider);
                self.ctx.emit(" in");
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                self.format(*body);
                self.ctx.dedent();
            }

            // For - body on new line if needed
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                self.ctx.emit("for ");
                self.ctx.emit(self.interner.lookup(*binding));
                self.ctx.emit(" in ");
                self.format(*iter);
                if let Some(guard_id) = guard {
                    self.ctx.emit(" if ");
                    self.format(*guard_id);
                }
                if *is_yield {
                    self.ctx.emit(" yield");
                } else {
                    self.ctx.emit(" do");
                }
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                self.format(*body);
                self.ctx.dedent();
            }

            // Fallback to inline for things that don't have special broken format
            _ => self.emit_inline(expr_id),
        }
    }

    /// Emit an else branch, handling else-if chains with proper line breaking.
    ///
    /// For chained else-if, each else clause goes on a new line, with the
    /// `else if cond then branch` together on that line:
    /// ```text
    /// if cond1 then branch1
    /// else if cond2 then branch2
    /// else if cond3 then branch3
    /// else branch4
    /// ```
    pub(super) fn emit_else_branch(&mut self, else_id: ExprId) {
        self.ctx.emit_newline_indent();
        self.ctx.emit("else ");

        let else_expr = self.arena.get_expr(else_id);
        if let ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } = &else_expr.kind
        {
            // else-if chain: check if "if cond then branch" fits on this line
            let cond_width = self.width_calc.width(*cond);
            let then_width = self.width_calc.width(*then_branch);

            // Check if the segment fits: "if " + cond + " then " + branch
            let segment_fits = cond_width != ALWAYS_STACKED
                && then_width != ALWAYS_STACKED
                && self.ctx.fits(3 + cond_width + 6 + then_width);

            if segment_fits {
                // Emit "if cond then branch" inline
                self.ctx.emit("if ");
                self.emit_inline(*cond);
                self.ctx.emit(" then ");
                self.emit_inline(*then_branch);
            } else {
                // Segment too long, break the then_branch to new line
                self.ctx.emit("if ");
                self.format(*cond);
                self.ctx.emit(" then");
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                self.format(*then_branch);
                self.ctx.dedent();
            }

            if let Some(next_else_id) = else_branch {
                self.emit_else_branch(*next_else_id);
            }
        } else {
            // Final else branch
            self.format(else_id);
        }
    }
}
