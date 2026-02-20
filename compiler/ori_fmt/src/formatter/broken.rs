//! Broken Formatting
//!
//! Methods for emitting expressions in broken (multi-line) format.
//! Used when expressions don't fit on a single line.

use ori_ir::{BinaryOp, ExprId, ExprKind, Name, StringLookup};

use crate::width::ALWAYS_STACKED;

use super::{binary_op_str, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit an expression in broken (multi-line) format.
    #[expect(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "exhaustive ExprKind broken formatting dispatch"
    )]
    pub(super) fn emit_broken(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Binary expression - break before operator
            ExprKind::Binary { op, left, right } => {
                self.emit_binary_operand_broken(*left, *op, true);
                self.ctx.emit_newline_indent();
                self.ctx.emit(binary_op_str(*op));
                self.ctx.emit_space();
                self.emit_binary_operand_broken(*right, *op, false);
            }

            // Calls - one argument per line
            ExprKind::Call { func, args } => {
                self.format_call_target(*func);
                self.ctx.emit("(");
                self.emit_broken_expr_list(*args);
                self.ctx.emit(")");
            }
            ExprKind::CallNamed { func, args } => {
                self.format_call_target(*func);
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
                if items.is_empty() {
                    self.ctx.emit("[]");
                } else {
                    let items_slice = self.arena.get_expr_list(*items);
                    self.ctx.emit("[");
                    self.emit_broken_list(items_slice);
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
            ExprKind::MapWithSpread(elements) => {
                let elements_list = self.arena.get_map_elements(*elements);
                if elements_list.is_empty() {
                    self.ctx.emit("{}");
                } else {
                    self.ctx.emit("{");
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    for (i, element) in elements_list.iter().enumerate() {
                        self.ctx.emit_indent();
                        match element {
                            ori_ir::MapElement::Entry(entry) => {
                                self.format(entry.key);
                                self.ctx.emit(": ");
                                self.format(entry.value);
                            }
                            ori_ir::MapElement::Spread { expr, .. } => {
                                self.ctx.emit("...");
                                self.format(*expr);
                            }
                        }
                        self.ctx.emit(",");
                        if i < elements_list.len() - 1 {
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
                if items.is_empty() {
                    self.ctx.emit("()");
                } else {
                    let items_slice = self.arena.get_expr_list(*items);
                    let items_len = items_slice.len();
                    self.ctx.emit("(");
                    self.ctx.emit_newline();
                    self.ctx.indent();
                    for (i, &item) in items_slice.iter().enumerate() {
                        self.ctx.emit_indent();
                        self.format(item);
                        self.ctx.emit(",");
                        if i < items_len - 1 {
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

                if else_branch.is_present() {
                    self.emit_else_branch(*else_branch);
                }
            }

            // Let binding
            // Per spec: mutable is default, $ prefix for immutable
            ExprKind::Let {
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
                label,
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                self.ctx.emit("for");
                if *label != Name::EMPTY {
                    self.ctx.emit(":");
                    self.ctx.emit(self.interner.lookup(*label));
                }
                self.ctx.emit(" ");
                self.ctx.emit(self.interner.lookup(*binding));
                self.ctx.emit(" in ");
                self.format_iter(*iter);
                if guard.is_present() {
                    self.ctx.emit(" if ");
                    self.format(*guard);
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

            // Block - delegate to stacked formatting when broken
            ExprKind::Block { .. } => self.emit_stacked(expr_id),

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

            if else_branch.is_present() {
                self.emit_else_branch(*else_branch);
            }
        } else {
            // Final else branch
            self.format(else_id);
        }
    }

    /// Emit a binary operand in broken format, wrapping in parentheses if needed.
    ///
    /// Parentheses are needed when the operand is a binary expression with lower
    /// precedence than the parent operator, or when associativity requires it.
    fn emit_binary_operand_broken(&mut self, operand: ExprId, parent_op: BinaryOp, is_left: bool) {
        let expr = self.arena.get_expr(operand);

        let needs_parens = match &expr.kind {
            ExprKind::Binary { op: child_op, .. } => {
                let parent_prec = parent_op.precedence();
                let child_prec = child_op.precedence();

                match child_prec.cmp(&parent_prec) {
                    std::cmp::Ordering::Greater => {
                        // Child has lower precedence (higher number) - needs parens
                        true
                    }
                    std::cmp::Ordering::Equal => {
                        // Same precedence - check associativity
                        // All ops are left-associative except ??
                        let is_right_assoc = matches!(parent_op, BinaryOp::Coalesce);
                        if is_right_assoc {
                            is_left
                        } else {
                            !is_left
                        }
                    }
                    std::cmp::Ordering::Less => false,
                }
            }
            ExprKind::Lambda { .. } | ExprKind::Let { .. } | ExprKind::If { .. } => true,
            _ => false,
        };

        if needs_parens {
            self.ctx.emit("(");
            self.format(operand);
            self.ctx.emit(")");
        } else {
            self.format(operand);
        }
    }
}
