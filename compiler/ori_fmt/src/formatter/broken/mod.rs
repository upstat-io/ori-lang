//! Broken Formatting
//!
//! Methods for emitting expressions in broken (multi-line) format.
//! Used when expressions don't fit on a single line.
//!
//! [`control_flow`] owns broken rendering for control-flow expressions.

mod control_flow;

use ori_ir::{BinaryOp, ExprId, ExprKind, StringLookup};

use crate::rules::map_key_needs_brackets;

use super::{needs_binary_parens, BinaryOperandSide, Formatter};

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
    pub(super) fn emit_broken(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Binary expression - break before operator
            ExprKind::Binary { op, left, right } => {
                self.emit_binary_operand_broken(*left, *op, BinaryOperandSide::Left);
                self.ctx.emit_newline_indent();
                self.ctx.emit(op.as_symbol());
                self.ctx.emit_space();
                self.emit_binary_operand_broken(*right, *op, BinaryOperandSide::Right);
            }

            kind @ (ExprKind::Call { .. }
            | ExprKind::CallNamed { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::MethodCallNamed { .. }) => self.emit_broken_call(kind),

            kind @ (ExprKind::List(_)
            | ExprKind::ListWithSpread(_)
            | ExprKind::Tuple(_)) => self.emit_broken_sequence(kind),
            kind @ (ExprKind::Map(_) | ExprKind::MapWithSpread(_)) => {
                self.emit_broken_map(kind);
            }
            kind @ (ExprKind::Struct { .. } | ExprKind::StructWithSpread { .. }) => {
                self.emit_broken_struct(kind);
            }

            kind @ (ExprKind::If { .. }
            | ExprKind::Let { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::WithCapability { .. }
            | ExprKind::For { .. }
            | ExprKind::While { .. }) => self.emit_broken_control_flow(kind),

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

    fn emit_broken_call(&mut self, kind: &ExprKind) {
        match kind {
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
            unexpected => unreachable!("unexpected broken call kind: {unexpected:?}"),
        }
    }

    fn emit_broken_sequence(&mut self, kind: &ExprKind) {
        match kind {
            ExprKind::List(items) => {
                if items.is_empty() {
                    self.ctx.emit("[]");
                } else {
                    let items = self.arena.get_expr_list(*items);
                    self.ctx.emit("[");
                    self.emit_broken_list(items);
                    self.ctx.emit("]");
                }
            }
            ExprKind::ListWithSpread(elements) => {
                let elements = self.arena.get_list_elements(*elements);
                if elements.is_empty() {
                    self.ctx.emit("[]");
                } else {
                    self.ctx.emit("[");
                    self.emit_broken_items(elements, |formatter, element| match element {
                        ori_ir::ListElement::Expr { expr, .. } => formatter.format(*expr),
                        ori_ir::ListElement::Spread { expr, .. } => {
                            formatter.ctx.emit("...");
                            formatter.format(*expr);
                        }
                    });
                    self.ctx.emit("]");
                }
            }
            ExprKind::Tuple(items) => {
                if items.is_empty() {
                    self.ctx.emit("()");
                } else {
                    let items = self.arena.get_expr_list(*items);
                    self.ctx.emit("(");
                    self.emit_broken_items(items, |formatter, &item| formatter.format(item));
                    self.ctx.emit(")");
                }
            }
            unexpected => unreachable!("unexpected broken sequence kind: {unexpected:?}"),
        }
    }

    fn emit_broken_map(&mut self, kind: &ExprKind) {
        match kind {
            ExprKind::Map(entries) => {
                let entries = self.arena.get_map_entries(*entries);
                if entries.is_empty() {
                    self.ctx.emit("{}");
                } else {
                    self.ctx.emit("{");
                    self.emit_broken_items(entries, |formatter, entry| {
                        formatter.format_map_key(entry.key);
                        formatter.ctx.emit(": ");
                        formatter.format(entry.value);
                    });
                    self.ctx.emit("}");
                }
            }
            ExprKind::MapWithSpread(elements) => {
                let elements = self.arena.get_map_elements(*elements);
                if elements.is_empty() {
                    self.ctx.emit("{}");
                } else {
                    self.ctx.emit("{");
                    self.emit_broken_items(elements, |formatter, element| match element {
                        ori_ir::MapElement::Entry(entry) => {
                            formatter.format_map_key(entry.key);
                            formatter.ctx.emit(": ");
                            formatter.format(entry.value);
                        }
                        ori_ir::MapElement::Spread { expr, .. } => {
                            formatter.ctx.emit("...");
                            formatter.format(*expr);
                        }
                    });
                    self.ctx.emit("}");
                }
            }
            unexpected => unreachable!("unexpected broken map kind: {unexpected:?}"),
        }
    }

    fn emit_broken_struct(&mut self, kind: &ExprKind) {
        match kind {
            ExprKind::Struct { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" {");
                let fields = self.arena.get_field_inits(*fields);
                if fields.is_empty() {
                    self.ctx.emit("}");
                } else {
                    self.emit_broken_items(fields, |formatter, field| {
                        formatter.ctx.emit(formatter.interner.lookup(field.name));
                        if let Some(value) = field.value {
                            formatter.ctx.emit(": ");
                            formatter.format(value);
                        }
                    });
                    self.ctx.emit("}");
                }
            }
            ExprKind::StructWithSpread { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" {");
                let fields = self.arena.get_struct_lit_fields(*fields);
                if fields.is_empty() {
                    self.ctx.emit("}");
                } else {
                    self.emit_broken_items(fields, |formatter, field| match field {
                        ori_ir::StructLitField::Field(init) => {
                            formatter.ctx.emit(formatter.interner.lookup(init.name));
                            if let Some(value) = init.value {
                                formatter.ctx.emit(": ");
                                formatter.format(value);
                            }
                        }
                        ori_ir::StructLitField::Spread { expr, .. } => {
                            formatter.ctx.emit("...");
                            formatter.format(*expr);
                        }
                    });
                    self.ctx.emit("}");
                }
            }
            unexpected => unreachable!("unexpected broken struct kind: {unexpected:?}"),
        }
    }

    fn emit_broken_control_flow(&mut self, kind: &ExprKind) {
        match kind {
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.emit_broken_if(*cond, *then_branch, *else_branch),
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => self.emit_broken_let(*pattern, *ty, *init),
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.emit_broken_lambda(*params, *ret_ty, *body),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => self.emit_broken_with_capability(*capability, *provider, *body),
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => self.emit_broken_for(*label, *pattern, *iter, *guard, *body, *is_yield),
            ExprKind::While { label, cond, body } => {
                self.emit_broken_while(*label, *cond, *body);
            }
            unexpected => unreachable!("unexpected broken control-flow kind: {unexpected:?}"),
        }
    }

    /// Emit a binary operand in broken format, wrapping in parentheses if needed.
    fn emit_binary_operand_broken(
        &mut self,
        operand: ExprId,
        parent_op: BinaryOp,
        side: BinaryOperandSide,
    ) {
        let needs_parens = needs_binary_parens(self.arena, operand, parent_op, side);
        self.emit_parenthesized_if(needs_parens, operand, Self::format);
    }
}
