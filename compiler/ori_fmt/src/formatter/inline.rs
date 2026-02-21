//! Inline Formatting
//!
//! Methods for emitting expressions inline (single line).
//! Used when expressions fit within the line width.

use ori_ir::{BinaryOp, ExprId, ExprKind, Name, StringLookup};

use super::{binary_op_str, needs_binary_parens, unary_op_str, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit an expression inline (single line).
    #[expect(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
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
                self.ctx.emit(binary_op_str(*op));
                self.ctx.emit_space();
                self.emit_binary_operand_inline(*right, *op, false);
            }
            ExprKind::Unary { op, operand } => {
                self.ctx.emit(unary_op_str(*op));
                // Unary operators bind tighter than binary - wrap binary operands
                let operand_expr = self.arena.get_expr(*operand);
                let needs_parens = matches!(
                    &operand_expr.kind,
                    ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Lambda { .. }
                );
                if needs_parens {
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

            // Control flow
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.ctx.emit("if ");
                self.emit_inline(*cond);
                self.ctx.emit(" then ");
                self.emit_inline(*then_branch);
                if else_branch.is_present() {
                    self.ctx.emit(" else ");
                    self.emit_inline(*else_branch);
                }
            }

            // Let binding
            // Per spec: mutable is default, $ prefix for immutable
            // The $ prefix is emitted by emit_binding_pattern(), not here
            ExprKind::Let {
                pattern,
                ty: _,
                init,
                mutable: _,
            } => {
                self.ctx.emit("let ");
                let pat = self.arena.get_binding_pattern(*pattern);
                self.emit_binding_pattern(pat);
                self.ctx.emit(" = ");
                self.emit_inline(*init);
            }

            // Lambda
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
                self.ctx.emit(" -> ");
                self.emit_inline(*body);
            }

            // Collections
            ExprKind::List(items) => {
                self.ctx.emit("[");
                for (i, item) in self.arena.get_expr_list(*items).iter().copied().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_inline(item);
                }
                self.ctx.emit("]");
            }
            ExprKind::ListWithSpread(elements) => {
                self.ctx.emit("[");
                for (i, element) in self.arena.get_list_elements(*elements).iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    match element {
                        ori_ir::ListElement::Expr { expr, .. } => {
                            self.emit_inline(*expr);
                        }
                        ori_ir::ListElement::Spread { expr, .. } => {
                            self.ctx.emit("...");
                            self.emit_inline(*expr);
                        }
                    }
                }
                self.ctx.emit("]");
            }
            ExprKind::Map(entries) => {
                let entries_list = self.arena.get_map_entries(*entries);
                self.ctx.emit("{");
                for (i, entry) in entries_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_inline(entry.key);
                    self.ctx.emit(": ");
                    self.emit_inline(entry.value);
                }
                self.ctx.emit("}");
            }
            ExprKind::MapWithSpread(elements) => {
                self.ctx.emit("{");
                for (i, element) in self.arena.get_map_elements(*elements).iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    match element {
                        ori_ir::MapElement::Entry(entry) => {
                            self.emit_inline(entry.key);
                            self.ctx.emit(": ");
                            self.emit_inline(entry.value);
                        }
                        ori_ir::MapElement::Spread { expr, .. } => {
                            self.ctx.emit("...");
                            self.emit_inline(*expr);
                        }
                    }
                }
                self.ctx.emit("}");
            }
            ExprKind::Struct { name, fields } => {
                self.ctx.emit(self.interner.lookup(*name));
                let fields_list = self.arena.get_field_inits(*fields);
                if fields_list.is_empty() {
                    self.ctx.emit(" {}");
                } else {
                    self.ctx.emit(" { ");
                    for (i, field) in fields_list.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit(self.interner.lookup(field.name));
                        if let Some(value) = field.value {
                            self.ctx.emit(": ");
                            self.emit_inline(value);
                        }
                    }
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
                    for (i, field) in fields_list.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        match field {
                            ori_ir::StructLitField::Field(init) => {
                                self.ctx.emit(self.interner.lookup(init.name));
                                if let Some(value) = init.value {
                                    self.ctx.emit(": ");
                                    self.emit_inline(value);
                                }
                            }
                            ori_ir::StructLitField::Spread { expr, .. } => {
                                self.ctx.emit("...");
                                self.emit_inline(*expr);
                            }
                        }
                    }
                    self.ctx.emit(" }");
                }
            }
            ExprKind::Tuple(items) => {
                let items_slice = self.arena.get_expr_list(*items);
                let items_len = items_slice.len();
                self.ctx.emit("(");
                for (i, &item) in items_slice.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_inline(item);
                }
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

            // Capability
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => {
                self.ctx.emit("with ");
                self.ctx.emit(self.interner.lookup(*capability));
                self.ctx.emit(" = ");
                self.emit_inline(*provider);
                self.ctx.emit(" in ");
                self.emit_inline(*body);
            }

            // For loop
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
                self.emit_iter_inline(*iter);
                if guard.is_present() {
                    self.ctx.emit(" if ");
                    self.emit_inline(*guard);
                }
                if *is_yield {
                    self.ctx.emit(" yield ");
                } else {
                    self.ctx.emit(" do ");
                }
                self.emit_inline(*body);
            }

            // Loop
            ExprKind::Loop { label, body } => {
                self.ctx.emit("loop");
                if *label != Name::EMPTY {
                    self.ctx.emit(":");
                    self.ctx.emit(self.interner.lookup(*label));
                }
                self.ctx.emit(" ");
                self.emit_inline(*body);
            }

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
                for (i, prop) in props.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit(self.interner.lookup(prop.name));
                    self.ctx.emit(": ");
                    self.emit_inline(prop.value);
                }
                self.ctx.emit(")");
            }

            // Template literals
            ExprKind::TemplateFull(name) => {
                self.ctx.emit("`");
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit("`");
            }
            ExprKind::TemplateLiteral { head, parts } => {
                self.ctx.emit("`");
                self.ctx.emit(self.interner.lookup(*head));
                for part in self.arena.get_template_parts(*parts) {
                    self.ctx.emit("{");
                    self.emit_inline(part.expr);
                    if part.format_spec != ori_ir::Name::EMPTY {
                        self.ctx.emit(":");
                        self.ctx.emit(self.interner.lookup(part.format_spec));
                    }
                    self.ctx.emit("}");
                    self.ctx.emit(self.interner.lookup(part.text_after));
                }
                self.ctx.emit("`");
            }

            // Error node (preserve as-is, shouldn't format)
            ExprKind::Error => self.ctx.emit("/* error */"),
        }
    }

    /// Emit a binary operand inline, wrapping in parentheses if needed for precedence.
    fn emit_binary_operand_inline(&mut self, operand: ExprId, parent_op: BinaryOp, is_left: bool) {
        if needs_binary_parens(self.arena, operand, parent_op, is_left) {
            self.ctx.emit("(");
            self.emit_inline(operand);
            self.ctx.emit(")");
        } else {
            self.emit_inline(operand);
        }
    }
}
