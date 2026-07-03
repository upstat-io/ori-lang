//! Inline Formatting
//!
//! Methods for emitting expressions inline (single line).
//! Used when expressions fit within the line width.
//!
//! `if`/`let`/lambda/`with`/`for`/`loop`/`while` rendering lives in
//! [`control_flow`] — split out to keep this dispatch file under the
//! workspace file-size limit.

mod control_flow;

use ori_ir::{BinaryOp, ExprId, ExprKind, Name, StringLookup, UnaryOp};

use crate::rules::{map_key_needs_brackets, needs_parens, ParenPosition};

use super::{needs_binary_parens, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit a map-literal key inline, wrapping a computed key in `[ ]` per
    /// [`map_key_needs_brackets`].
    pub(super) fn emit_inline_map_key(&mut self, key: ExprId) {
        let bracketed = map_key_needs_brackets(&self.arena.get_expr(key).kind);
        if bracketed {
            self.ctx.emit("[");
        }
        self.emit_inline(key);
        if bracketed {
            self.ctx.emit("]");
        }
    }

    /// Emit an expression inline (single line).
    ///
    /// **Invariant:** This match is exhaustive with no wildcard `_ =>` arm.
    /// Every `ExprKind` variant is listed explicitly so that adding a new
    /// variant causes a compile error here, in `emit_broken()`, and in
    /// `calculate_width()`. The `inline_dispatch_has_no_wildcard` test
    /// enforces this at the source level.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind formatting dispatch"
    )]
    pub(super) fn emit_inline(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Literals
            ExprKind::Int(n) => self.emit_int(*n),
            ExprKind::Float(bits) => self.emit_float(f64::from_bits(*bits)),
            ExprKind::Bool(b) => self.ctx.emit(if *b { "true" } else { "false" }),
            ExprKind::String(name) => self.emit_string(self.interner.lookup(*name)),
            ExprKind::Char(c) => self.emit_char(*c),
            ExprKind::Unit => self.ctx.emit("()"),
            ExprKind::Duration { value, unit } => self.emit_duration(*value, *unit),
            ExprKind::Size { value, unit } => self.emit_size(*value, *unit),

            // Identifiers
            ExprKind::Ident(name) => self.ctx.emit(self.interner.lookup(*name)),
            ExprKind::Const(name) => {
                self.ctx.emit("$");
                self.ctx.emit(self.interner.lookup(*name));
            }
            ExprKind::SelfRef => self.ctx.emit("self"),
            ExprKind::FunctionRef(name) => {
                self.ctx.emit("@");
                self.ctx.emit(self.interner.lookup(*name));
            }
            ExprKind::HashLength => self.ctx.emit("#"),

            // Binary/unary operations
            ExprKind::Binary { op, left, right } => {
                self.emit_binary_operand_inline(*left, *op, true);
                self.ctx.emit_space();
                self.ctx.emit(op.as_symbol());
                self.ctx.emit_space();
                self.emit_binary_operand_inline(*right, *op, false);
            }
            ExprKind::Unary { op, operand } => {
                self.ctx.emit(op.as_symbol());
                // Unary operators bind tighter than binary - wrap the operand
                // per the shared Layer 4 paren rule (`Neg` additionally
                // guards the nested-Neg token-adjacency hazard).
                let position = if *op == UnaryOp::Neg {
                    ParenPosition::UnaryNegOperand
                } else {
                    ParenPosition::UnaryOperand
                };
                if needs_parens(self.arena, *operand, position) {
                    self.ctx.emit("(");
                    self.emit_inline(*operand);
                    self.ctx.emit(")");
                } else {
                    self.emit_inline(*operand);
                }
            }

            // Calls
            ExprKind::Call { func, args } => {
                self.emit_call_target_inline(*func);
                self.ctx.emit("(");
                self.emit_inline_expr_list(*args);
                self.ctx.emit(")");
            }
            ExprKind::CallNamed { func, args } => {
                self.emit_call_target_inline(*func);
                self.ctx.emit("(");
                self.emit_inline_call_args(*args);
                self.ctx.emit(")");
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.emit_receiver_inline(*receiver);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*method));
                self.ctx.emit("(");
                self.emit_inline_expr_list(*args);
                self.ctx.emit(")");
            }
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => {
                self.emit_receiver_inline(*receiver);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*method));
                self.ctx.emit("(");
                self.emit_inline_call_args(*args);
                self.ctx.emit(")");
            }

            // Access
            ExprKind::Field { receiver, field } => {
                self.emit_receiver_inline(*receiver);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*field));
            }
            ExprKind::Index { receiver, index } => {
                self.emit_receiver_inline(*receiver);
                self.ctx.emit("[");
                self.emit_inline(*index);
                self.ctx.emit("]");
            }

            // Control flow.
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.emit_inline_if(*cond, *then_branch, *else_branch),

            // Let binding.
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => self.emit_inline_let(*pattern, *ty, *init),

            // Lambda.
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.emit_inline_lambda(*params, *ret_ty, *body),

            // Collections
            ExprKind::List(items) => {
                self.ctx.emit("[");
                self.emit_inline_expr_list(*items);
                self.ctx.emit("]");
            }
            ExprKind::ListWithSpread(elements) => {
                let elements_list = self.arena.get_list_elements(*elements);
                self.ctx.emit("[");
                self.emit_inline_items(elements_list, |s, element| match element {
                    ori_ir::ListElement::Expr { expr, .. } => {
                        s.emit_inline(*expr);
                    }
                    ori_ir::ListElement::Spread { expr, .. } => {
                        s.ctx.emit("...");
                        s.emit_inline(*expr);
                    }
                });
                self.ctx.emit("]");
            }
            ExprKind::Map(entries) => {
                let entries_list = self.arena.get_map_entries(*entries);
                self.ctx.emit("{");
                self.emit_inline_items(entries_list, |s, entry| {
                    s.emit_inline_map_key(entry.key);
                    s.ctx.emit(": ");
                    s.emit_inline(entry.value);
                });
                self.ctx.emit("}");
            }
            ExprKind::MapWithSpread(elements) => {
                let elements_list = self.arena.get_map_elements(*elements);
                self.ctx.emit("{");
                self.emit_inline_items(elements_list, |s, element| match element {
                    ori_ir::MapElement::Entry(entry) => {
                        s.emit_inline_map_key(entry.key);
                        s.ctx.emit(": ");
                        s.emit_inline(entry.value);
                    }
                    ori_ir::MapElement::Spread { expr, .. } => {
                        s.ctx.emit("...");
                        s.emit_inline(*expr);
                    }
                });
                self.ctx.emit("}");
            }
            ExprKind::Struct { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                let fields_list = self.arena.get_field_inits(*fields);
                if fields_list.is_empty() {
                    self.ctx.emit(" {}");
                } else {
                    self.ctx.emit(" { ");
                    self.emit_inline_items(fields_list, |s, field| {
                        s.ctx.emit(s.interner.lookup(field.name));
                        if let Some(value) = field.value {
                            s.ctx.emit(": ");
                            s.emit_inline(value);
                        }
                    });
                    self.ctx.emit(" }");
                }
            }
            ExprKind::StructWithSpread { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                let fields_list = self.arena.get_struct_lit_fields(*fields);
                if fields_list.is_empty() {
                    self.ctx.emit(" {}");
                } else {
                    self.ctx.emit(" { ");
                    self.emit_inline_items(fields_list, |s, field| match field {
                        ori_ir::StructLitField::Field(init) => {
                            s.ctx.emit(s.interner.lookup(init.name));
                            if let Some(value) = init.value {
                                s.ctx.emit(": ");
                                s.emit_inline(value);
                            }
                        }
                        ori_ir::StructLitField::Spread { expr, .. } => {
                            s.ctx.emit("...");
                            s.emit_inline(*expr);
                        }
                    });
                    self.ctx.emit(" }");
                }
            }
            ExprKind::Tuple(items) => {
                let items_slice = self.arena.get_expr_list(*items);
                let items_len = items_slice.len();
                self.ctx.emit("(");
                self.emit_inline_items(items_slice, |s, &item| s.emit_inline(item));
                // Single-element tuples need trailing comma: (42,) vs (42)
                if items_len == 1 {
                    self.ctx.emit(",");
                }
                self.ctx.emit(")");
            }
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => {
                if start.is_present() {
                    self.emit_inline(*start);
                }
                if *inclusive {
                    self.ctx.emit("..=");
                } else {
                    self.ctx.emit("..");
                }
                if end.is_present() {
                    self.emit_inline(*end);
                }
                if step.is_present() {
                    self.ctx.emit(" by ");
                    self.emit_inline(*step);
                }
            }

            // Result/Option wrappers
            ExprKind::Ok(inner) => self.emit_wrapper_inline("Ok", *inner),
            ExprKind::Err(inner) => self.emit_wrapper_inline("Err", *inner),
            ExprKind::Some(inner) => self.emit_wrapper_inline_required("Some", *inner),
            ExprKind::None => self.ctx.emit("None"),

            // Control flow jumps
            ExprKind::Break { label, value } => {
                self.ctx.emit("break");
                if *label != Name::EMPTY {
                    self.ctx.emit(":");
                    self.ctx.emit(self.interner.lookup(*label));
                }
                if value.is_present() {
                    self.ctx.emit_space();
                    self.emit_inline(*value);
                }
            }
            ExprKind::Continue { label, value } => {
                self.ctx.emit("continue");
                if *label != Name::EMPTY {
                    self.ctx.emit(":");
                    self.ctx.emit(self.interner.lookup(*label));
                }
                if value.is_present() {
                    self.ctx.emit_space();
                    self.emit_inline(*value);
                }
            }

            // Postfix operators
            ExprKind::Unsafe(inner) => {
                self.ctx.emit("unsafe ");
                self.emit_inline(*inner);
            }
            ExprKind::Await(inner) => {
                self.emit_inline(*inner);
                self.ctx.emit(".await");
            }
            ExprKind::Try(inner) => {
                self.emit_inline(*inner);
                self.ctx.emit("?");
            }
            ExprKind::Cast { expr, ty, fallible } => {
                self.emit_inline(*expr);
                if *fallible {
                    self.ctx.emit(" as? ");
                } else {
                    self.ctx.emit(" as ");
                }
                self.emit_type(self.arena.get_parsed_type(*ty));
            }

            // Assignment
            ExprKind::Assign { target, value } => {
                self.emit_inline(*target);
                self.ctx.emit(" = ");
                self.emit_inline(*value);
            }
            ExprKind::AssignTarget { root, steps } => {
                self.emit_receiver_inline(*root);
                for step in self.arena.get_access_steps(*steps) {
                    match step {
                        ori_ir::AccessStep::Field(field) => {
                            self.ctx.emit(".");
                            self.ctx.emit(self.interner.lookup(*field));
                        }
                        ori_ir::AccessStep::Index(index) => {
                            self.ctx.emit("[");
                            self.emit_inline(*index);
                            self.ctx.emit("]");
                        }
                    }
                }
            }

            // Capability.
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => self.emit_inline_with_capability(*capability, *provider, *body),

            // For loop.
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => self.emit_inline_for(*label, *pattern, *iter, *guard, *body, *is_yield),

            // Loop.
            ExprKind::Loop { label, body } => self.emit_inline_loop(*label, *body),

            // While.
            ExprKind::While { label, cond, body } => self.emit_inline_while(*label, *cond, *body),

            // Block
            ExprKind::Block { stmts, result } => {
                let stmts_list = self.arena.get_stmt_range(*stmts);
                if stmts_list.is_empty() {
                    if result.is_present() {
                        self.ctx.emit("{ ");
                        self.emit_inline(*result);
                        self.ctx.emit(" }");
                    } else {
                        self.ctx.emit("{}");
                    }
                } else {
                    // Blocks with statements always break
                    self.emit_stacked(expr_id);
                }
            }

            // Match (always stacked, should not reach here)
            #[expect(
                clippy::match_same_arms,
                reason = "Keeping Match and FunctionSeq as separate arms for documentation clarity"
            )]
            ExprKind::Match { .. } => self.emit_stacked(expr_id),

            // Sequential patterns (always stacked)
            ExprKind::FunctionSeq(..) => self.emit_stacked(expr_id),

            // Named expression patterns
            ExprKind::FunctionExp(exp_id) => {
                let exp = self.arena.get_function_exp(*exp_id);
                self.ctx.emit(exp.kind.name());
                self.ctx.emit("(");
                let props = self.arena.get_named_exprs(exp.props);
                self.emit_inline_items(props, |s, prop| {
                    s.ctx.emit(s.interner.lookup(prop.name));
                    s.ctx.emit(": ");
                    s.emit_inline(prop.value);
                });
                self.ctx.emit(")");
            }

            // Template literals — literal text segments are re-escaped back to
            // canonical source (inverse of the lexer cooker) so the output
            // round-trips; interpolation delimiters stay structural.
            ExprKind::TemplateFull(name) => {
                self.ctx.emit("`");
                self.ctx
                    .emit(crate::escape_template_text(self.interner.lookup(*name)));
                self.ctx.emit("`");
            }
            ExprKind::TemplateLiteral { head, parts } => {
                self.ctx.emit("`");
                self.ctx
                    .emit(crate::escape_template_text(self.interner.lookup(*head)));
                for part in self.arena.get_template_parts(*parts) {
                    self.ctx.emit("{");
                    self.emit_inline(part.expr);
                    if part.format_spec != ori_ir::Name::EMPTY {
                        self.ctx.emit(":");
                        self.ctx.emit(self.interner.lookup(part.format_spec));
                    }
                    self.ctx.emit("}");
                    self.ctx.emit(crate::escape_template_text(
                        self.interner.lookup(part.text_after),
                    ));
                }
                self.ctx.emit("`");
            }

            // Error node (preserve as-is, shouldn't format)
            ExprKind::Error => self.ctx.emit("/* error */"),
        }
    }

    /// Emit a binary operand inline, wrapping in parentheses if needed for precedence.
    fn emit_binary_operand_inline(&mut self, operand: ExprId, parent_op: BinaryOp, is_left: bool) {
        let needs_parens = needs_binary_parens(self.arena, operand, parent_op, is_left);
        self.emit_parenthesized_if(needs_parens, operand, Self::emit_inline);
    }
}
