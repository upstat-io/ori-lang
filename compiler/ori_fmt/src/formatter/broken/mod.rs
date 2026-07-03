//! Broken Formatting
//!
//! Methods for emitting expressions in broken (multi-line) format.
//! Used when expressions don't fit on a single line.
//!
//! `if`/`let`/lambda/`with`/`for`/`while` rendering lives in
//! [`control_flow`] — split out to keep this dispatch file under the
//! workspace file-size limit.

mod control_flow;

use ori_ir::{BinaryOp, ExprId, ExprKind, StringLookup};

use crate::rules::map_key_needs_brackets;

use super::{needs_binary_parens, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit a map-literal key in broken format, wrapping a computed key in `[ ]`
    /// per [`map_key_needs_brackets`].
    fn format_map_key(&mut self, key: ExprId) {
        let bracketed = map_key_needs_brackets(&self.arena.get_expr(key).kind);
        if bracketed {
            self.ctx.emit("[");
        }
        self.format(key);
        if bracketed {
            self.ctx.emit("]");
        }
    }

    /// Emit an expression in broken (multi-line) format.
    ///
    /// **Invariant:** This match is exhaustive with no wildcard `_ =>` arm.
    /// Every `ExprKind` variant is listed explicitly so that adding a new variant
    /// causes a compile error here, in `emit_inline()`, and in `calculate_width()`.
    /// The `broken_dispatch_has_no_wildcard` test enforces this at the source level.
    ///
    /// #### Variant groups
    /// - **Custom broken**: Compound expressions with multi-line rendering logic
    /// - **Always-stacked**: Block, Match, `FunctionSeq`, `FunctionExp` → `emit_stacked()`
    /// - **Leaf/atom**: Irreducible expressions → `emit_inline()` (parent breaks around them)
    /// - **Simple compound**: Subexpressions that don't benefit from breaking → `emit_inline()`
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind broken formatting dispatch"
    )]
    pub(super) fn emit_broken(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Binary expression - break before operator
            ExprKind::Binary { op, left, right } => {
                self.emit_binary_operand_broken(*left, *op, true);
                self.ctx.emit_newline_indent();
                self.ctx.emit(op.as_symbol());
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
                // All-or-nothing chain breaking "Idempotency" +
                // MethodChainRule::ALL_METHODS_BREAK: when this call breaks,
                // any chained receiver must also break.
                self.format_receiver_broken(*receiver);
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
                self.format_receiver_broken(*receiver);
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
                    self.emit_broken_items(entries_list, |s, entry| {
                        s.format_map_key(entry.key);
                        s.ctx.emit(": ");
                        s.format(entry.value);
                    });
                    self.ctx.emit("}");
                }
            }
            ExprKind::MapWithSpread(elements) => {
                let elements_list = self.arena.get_map_elements(*elements);
                if elements_list.is_empty() {
                    self.ctx.emit("{}");
                } else {
                    self.ctx.emit("{");
                    self.emit_broken_items(elements_list, |s, element| match element {
                        ori_ir::MapElement::Entry(entry) => {
                            s.format_map_key(entry.key);
                            s.ctx.emit(": ");
                            s.format(entry.value);
                        }
                        ori_ir::MapElement::Spread { expr, .. } => {
                            s.ctx.emit("...");
                            s.format(*expr);
                        }
                    });
                    self.ctx.emit("}");
                }
            }
            ExprKind::ListWithSpread(elements) => {
                let elements_list = self.arena.get_list_elements(*elements);
                if elements_list.is_empty() {
                    self.ctx.emit("[]");
                } else {
                    self.ctx.emit("[");
                    self.emit_broken_items(elements_list, |s, element| match element {
                        ori_ir::ListElement::Expr { expr, .. } => {
                            s.format(*expr);
                        }
                        ori_ir::ListElement::Spread { expr, .. } => {
                            s.ctx.emit("...");
                            s.format(*expr);
                        }
                    });
                    self.ctx.emit("]");
                }
            }
            ExprKind::Struct { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" {");
                let fields_list = self.arena.get_field_inits(*fields);
                if fields_list.is_empty() {
                    self.ctx.emit("}");
                } else {
                    self.emit_broken_items(fields_list, |s, field| {
                        s.ctx.emit(s.interner.lookup(field.name));
                        if let Some(value) = field.value {
                            s.ctx.emit(": ");
                            s.format(value);
                        }
                    });
                    self.ctx.emit("}");
                }
            }
            ExprKind::StructWithSpread { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" {");
                let fields_list = self.arena.get_struct_lit_fields(*fields);
                if fields_list.is_empty() {
                    self.ctx.emit("}");
                } else {
                    self.emit_broken_items(fields_list, |s, field| match field {
                        ori_ir::StructLitField::Field(init) => {
                            s.ctx.emit(s.interner.lookup(init.name));
                            if let Some(value) = init.value {
                                s.ctx.emit(": ");
                                s.format(value);
                            }
                        }
                        ori_ir::StructLitField::Spread { expr, .. } => {
                            s.ctx.emit("...");
                            s.format(*expr);
                        }
                    });
                    self.ctx.emit("}");
                }
            }
            ExprKind::Tuple(items) => {
                if items.is_empty() {
                    self.ctx.emit("()");
                } else {
                    let items_slice = self.arena.get_expr_list(*items);
                    self.ctx.emit("(");
                    self.emit_broken_items(items_slice, |s, &item| s.format(item));
                    self.ctx.emit(")");
                }
            }

            // If - break at else, keeping "else if" chains flat.
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.emit_broken_if(*cond, *then_branch, *else_branch),

            // Let binding — preserve type annotation per Annex D.
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => self.emit_broken_let(*pattern, *ty, *init),

            // Lambda with body on new line.
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.emit_broken_lambda(*params, *ret_ty, *body),

            // With capability - body on new line.
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => self.emit_broken_with_capability(*capability, *provider, *body),

            // For - body on new line if needed.
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => self.emit_broken_for(*label, *pattern, *iter, *guard, *body, *is_yield),

            // While - block body opens inline after `do`; non-block body on
            // a new line.
            ExprKind::While { label, cond, body } => {
                self.emit_broken_while(*label, *cond, *body);
            }

            // Always-stacked constructs: delegate to stacked rendering
            ExprKind::Block { .. }
            | ExprKind::Match { .. }
            | ExprKind::FunctionSeq(..)
            | ExprKind::FunctionExp(..) => self.emit_stacked(expr_id),

            // Inline-adequate expressions: either leaf/atoms with no substructure
            // to break, or simple compounds where breaking wouldn't help readability.
            // When these exceed the line width, the *parent* expression breaks.
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
            | ExprKind::Unsafe(_)
            | ExprKind::Await(_)
            | ExprKind::Try(_)
            | ExprKind::Cast { .. }
            | ExprKind::Assign { .. }
            | ExprKind::AssignTarget { .. }
            | ExprKind::Loop { .. }
            | ExprKind::Range { .. }
            | ExprKind::TemplateLiteral { .. } => self.emit_inline(expr_id),
        }
    }

    /// Emit a binary operand in broken format, wrapping in parentheses if needed.
    fn emit_binary_operand_broken(&mut self, operand: ExprId, parent_op: BinaryOp, is_left: bool) {
        let needs_parens = needs_binary_parens(self.arena, operand, parent_op, is_left);
        self.emit_parenthesized_if(needs_parens, operand, Self::format);
    }
}
