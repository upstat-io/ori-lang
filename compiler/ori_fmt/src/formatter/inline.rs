//! Inline Formatting
//!
//! Methods for emitting expressions inline (single line).
//! Used when expressions fit within the line width.

use ori_ir::{ExprId, ExprKind, StringLookup};

use super::{binary_op_str, unary_op_str, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit an expression inline (single line).
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
            ExprKind::Config(name) => {
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
                self.emit_inline(*left);
                self.ctx.emit_space();
                self.ctx.emit(binary_op_str(*op));
                self.ctx.emit_space();
                self.emit_inline(*right);
            }
            ExprKind::Unary { op, operand } => {
                self.ctx.emit(unary_op_str(*op));
                self.emit_inline(*operand);
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
                if let Some(else_id) = else_branch {
                    self.ctx.emit(" else ");
                    self.emit_inline(*else_id);
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
                let items_list = self.arena.get_expr_list(*items);
                self.ctx.emit("[");
                for (i, item) in items_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_inline(*item);
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
            ExprKind::Tuple(items) => {
                let items_list = self.arena.get_expr_list(*items);
                self.ctx.emit("(");
                for (i, item) in items_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_inline(*item);
                }
                // Single-element tuples need trailing comma: (42,) vs (42)
                if items_list.len() == 1 {
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
                if let Some(s) = start {
                    self.emit_inline(*s);
                }
                if *inclusive {
                    self.ctx.emit("..=");
                } else {
                    self.ctx.emit("..");
                }
                if let Some(e) = end {
                    self.emit_inline(*e);
                }
                if let Some(step_expr) = step {
                    self.ctx.emit(" by ");
                    self.emit_inline(*step_expr);
                }
            }

            // Result/Option wrappers
            ExprKind::Ok(inner) => self.emit_wrapper_inline("Ok", *inner),
            ExprKind::Err(inner) => self.emit_wrapper_inline("Err", *inner),
            ExprKind::Some(inner) => self.emit_wrapper_inline_required("Some", *inner),
            ExprKind::None => self.ctx.emit("None"),

            // Control flow jumps
            ExprKind::Break(val) => {
                self.ctx.emit("break");
                if let Some(val_id) = val {
                    self.ctx.emit_space();
                    self.emit_inline(*val_id);
                }
            }
            ExprKind::Continue(val) => {
                self.ctx.emit("continue");
                if let Some(val_id) = val {
                    self.ctx.emit_space();
                    self.emit_inline(*val_id);
                }
            }

            // Postfix operators
            ExprKind::Await(inner) => {
                self.emit_inline(*inner);
                self.ctx.emit(".await");
            }
            ExprKind::Try(inner) => {
                self.emit_inline(*inner);
                self.ctx.emit("?");
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
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                self.ctx.emit("for ");
                self.ctx.emit(self.interner.lookup(*binding));
                self.ctx.emit(" in ");
                self.emit_iter_inline(*iter);
                if let Some(guard_id) = guard {
                    self.ctx.emit(" if ");
                    self.emit_inline(*guard_id);
                }
                if *is_yield {
                    self.ctx.emit(" yield ");
                } else {
                    self.ctx.emit(" do ");
                }
                self.emit_inline(*body);
            }

            // Loop
            ExprKind::Loop { body } => {
                self.ctx.emit("loop(");
                self.emit_inline(*body);
                self.ctx.emit(")");
            }

            // Block
            ExprKind::Block { stmts, result } => {
                let stmts_list = self.arena.get_stmt_range(*stmts);
                if stmts_list.is_empty() {
                    if let Some(r) = result {
                        self.emit_inline(*r);
                    } else {
                        self.ctx.emit("()");
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
            ExprKind::FunctionExp(exp) => {
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

            // Error node (preserve as-is, shouldn't format)
            ExprKind::Error => self.ctx.emit("/* error */"),
        }
    }
}
