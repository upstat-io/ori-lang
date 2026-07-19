//! Single-line expression rendering.
//!
//! Control-flow forms delegate to [`control_flow`].

mod control_flow;

use ori_ir::{
    AccessStep, AccessStepRange, BinaryOp, CallArgRange, ExprId, ExprKind, ExprRange,
    FieldInitRange, FunctionExpId, ListElement, ListElementRange, MapElement, MapElementRange,
    MapEntryRange, Name, ParsedTypeId, StmtRange, StringLookup, StructLitField,
    StructLitFieldRange, TemplatePartRange, UnaryOp,
};

use crate::rules::{map_key_needs_brackets, needs_parens, ParenPosition};

use super::{needs_binary_parens, BinaryOperandSide, Formatter};

impl<I: StringLookup> Formatter<'_, I> {
    /// Emits a map key, bracketing computed expressions.
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

    /// Emits `expr_id` without line breaks.
    ///
    /// The exhaustive match keeps additions to [`ExprKind`] compiler-checked.
    pub(super) fn emit_inline(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Int(n) => self.emit_int(*n),
            ExprKind::Float(bits) => self.emit_float(f64::from_bits(*bits)),
            ExprKind::Bool(b) => self.ctx.emit(if *b { "true" } else { "false" }),
            ExprKind::String(name) => self.emit_string(self.interner.lookup(*name)),
            ExprKind::Char(c) => self.emit_char(*c),
            ExprKind::Unit => self.ctx.emit("()"),
            ExprKind::Duration { value, unit } => self.emit_duration(*value, *unit),
            ExprKind::Size { value, unit } => self.emit_size(*value, *unit),
            ExprKind::Ident(name) => self.ctx.emit(self.interner.lookup(*name)),
            ExprKind::Const(name) => self.emit_inline_prefixed_name("$", *name),
            ExprKind::SelfRef => self.ctx.emit("self"),
            ExprKind::FunctionRef(name) => self.emit_inline_prefixed_name("@", *name),
            ExprKind::HashLength => self.ctx.emit("#"),
            ExprKind::Binary { op, left, right } => self.emit_inline_binary(*left, *op, *right),
            ExprKind::Unary { op, operand } => self.emit_inline_unary(*op, *operand),
            ExprKind::Call { func, args } => self.emit_inline_call(*func, *args),
            ExprKind::CallNamed { func, args } => self.emit_inline_named_call(*func, *args),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.emit_inline_method_call(*receiver, *method, *args),
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.emit_inline_named_method_call(*receiver, *method, *args),
            ExprKind::Field { receiver, field } => self.emit_inline_field(*receiver, *field),
            ExprKind::Index { receiver, index } => self.emit_inline_index(*receiver, *index),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.emit_inline_if(*cond, *then_branch, *else_branch),
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => self.emit_inline_let(*pattern, *ty, *init),
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.emit_inline_lambda(*params, *ret_ty, *body),
            ExprKind::List(items) => self.emit_inline_list(*items),
            ExprKind::ListWithSpread(elements) => self.emit_inline_list_with_spread(*elements),
            ExprKind::Map(entries) => self.emit_inline_map(*entries),
            ExprKind::MapWithSpread(elements) => self.emit_inline_map_with_spread(*elements),
            ExprKind::Struct { name, fields } => self.emit_inline_struct(*name, *fields),
            ExprKind::StructWithSpread { name, fields } => {
                self.emit_inline_struct_with_spread(*name, *fields);
            }
            ExprKind::Tuple(items) => self.emit_inline_tuple(*items),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => self.emit_inline_range(*start, *end, *step, *inclusive),
            ExprKind::Ok(inner) => self.emit_wrapper_inline("Ok", *inner),
            ExprKind::Err(inner) => self.emit_wrapper_inline("Err", *inner),
            ExprKind::Some(inner) => self.emit_wrapper_inline_required("Some", *inner),
            ExprKind::None => self.ctx.emit("None"),
            ExprKind::Break { label, value } => self.emit_inline_jump("break", *label, *value),
            ExprKind::Continue { label, value } => {
                self.emit_inline_jump("continue", *label, *value);
            }
            ExprKind::Unsafe(inner) => self.emit_inline_prefixed_expr("unsafe ", *inner),
            ExprKind::Await(inner) => self.emit_inline_postfixed_expr(*inner, ".await"),
            ExprKind::Try(inner) => self.emit_inline_postfixed_expr(*inner, "?"),
            ExprKind::Cast { expr, ty, fallible } => self.emit_inline_cast(*expr, *ty, *fallible),
            ExprKind::Assign { target, value } => self.emit_inline_assign(*target, *value),
            ExprKind::AssignTarget { root, steps } => self.emit_inline_assign_target(*root, *steps),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => self.emit_inline_with_capability(*capability, *provider, *body),
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => self.emit_inline_for(*label, *pattern, *iter, *guard, *body, *is_yield),
            ExprKind::Loop { label, body } => self.emit_inline_loop(*label, *body),
            ExprKind::While { label, cond, body } => self.emit_inline_while(*label, *cond, *body),
            ExprKind::Block { stmts, result } => self.emit_inline_block(expr_id, *stmts, *result),
            ExprKind::Match { .. } | ExprKind::FunctionSeq(..) => self.emit_stacked(expr_id),
            ExprKind::FunctionExp(exp_id) => self.emit_inline_function_exp(*exp_id),
            ExprKind::TemplateFull(name) => self.emit_inline_template_full(*name),
            ExprKind::TemplateLiteral { head, parts } => {
                self.emit_inline_template_literal(*head, *parts);
            }
            ExprKind::Error => self.ctx.emit("/* error */"),
        }
    }

    fn emit_inline_prefixed_name(&mut self, prefix: &str, name: Name) {
        self.ctx.emit(prefix);
        self.ctx.emit(self.interner.lookup(name));
    }

    fn emit_inline_binary(&mut self, left: ExprId, op: BinaryOp, right: ExprId) {
        self.emit_binary_operand_inline(left, op, BinaryOperandSide::Left);
        self.ctx.emit_space();
        self.ctx.emit(op.as_symbol());
        self.ctx.emit_space();
        self.emit_binary_operand_inline(right, op, BinaryOperandSide::Right);
    }

    fn emit_inline_unary(&mut self, op: UnaryOp, operand: ExprId) {
        self.ctx.emit(op.as_symbol());
        let parenthesized = needs_parens(self.arena, operand, ParenPosition::UnaryOperand);
        self.emit_parenthesized_if(parenthesized, operand, Self::emit_inline);
    }

    fn emit_inline_call(&mut self, func: ExprId, args: ExprRange) {
        self.emit_call_target_inline(func);
        self.ctx.emit("(");
        self.emit_inline_expr_list(args);
        self.ctx.emit(")");
    }

    fn emit_inline_named_call(&mut self, func: ExprId, args: CallArgRange) {
        self.emit_call_target_inline(func);
        self.ctx.emit("(");
        self.emit_inline_call_args(args);
        self.ctx.emit(")");
    }

    fn emit_inline_method_call(&mut self, receiver: ExprId, method: Name, args: ExprRange) {
        self.emit_inline_method_prefix(receiver, method);
        self.emit_inline_expr_list(args);
        self.ctx.emit(")");
    }

    fn emit_inline_named_method_call(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
    ) {
        self.emit_inline_method_prefix(receiver, method);
        self.emit_inline_call_args(args);
        self.ctx.emit(")");
    }

    fn emit_inline_method_prefix(&mut self, receiver: ExprId, method: Name) {
        self.emit_receiver_inline(receiver);
        self.ctx.emit(".");
        self.ctx.emit(self.interner.lookup(method));
        self.ctx.emit("(");
    }

    fn emit_inline_field(&mut self, receiver: ExprId, field: Name) {
        self.emit_receiver_inline(receiver);
        self.ctx.emit(".");
        self.ctx.emit(self.interner.lookup(field));
    }

    fn emit_inline_index(&mut self, receiver: ExprId, index: ExprId) {
        self.emit_receiver_inline(receiver);
        self.ctx.emit("[");
        self.emit_inline(index);
        self.ctx.emit("]");
    }

    fn emit_inline_list(&mut self, items: ExprRange) {
        self.ctx.emit("[");
        self.emit_inline_expr_list(items);
        self.ctx.emit("]");
    }

    fn emit_inline_list_with_spread(&mut self, elements: ListElementRange) {
        self.ctx.emit("[");
        self.emit_inline_items(
            self.arena.get_list_elements(elements),
            |formatter, element| match element {
                ListElement::Expr { expr, .. } => formatter.emit_inline(*expr),
                ListElement::Spread { expr, .. } => {
                    formatter.ctx.emit("...");
                    formatter.emit_inline(*expr);
                }
            },
        );
        self.ctx.emit("]");
    }

    fn emit_inline_map(&mut self, entries: MapEntryRange) {
        self.ctx.emit("{");
        self.emit_inline_items(self.arena.get_map_entries(entries), |formatter, entry| {
            formatter.emit_inline_map_key(entry.key);
            formatter.ctx.emit(": ");
            formatter.emit_inline(entry.value);
        });
        self.ctx.emit("}");
    }

    fn emit_inline_map_with_spread(&mut self, elements: MapElementRange) {
        self.ctx.emit("{");
        self.emit_inline_items(
            self.arena.get_map_elements(elements),
            |formatter, element| match element {
                MapElement::Entry(entry) => {
                    formatter.emit_inline_map_key(entry.key);
                    formatter.ctx.emit(": ");
                    formatter.emit_inline(entry.value);
                }
                MapElement::Spread { expr, .. } => {
                    formatter.ctx.emit("...");
                    formatter.emit_inline(*expr);
                }
            },
        );
        self.ctx.emit("}");
    }

    fn emit_inline_struct(&mut self, name: Name, fields: FieldInitRange) {
        self.ctx.emit(self.interner.lookup(name));
        let fields = self.arena.get_field_inits(fields);
        if fields.is_empty() {
            self.ctx.emit(" {}");
            return;
        }
        self.ctx.emit(" { ");
        self.emit_inline_items(fields, |formatter, field| {
            formatter.ctx.emit(formatter.interner.lookup(field.name));
            if let Some(value) = field.value {
                formatter.ctx.emit(": ");
                formatter.emit_inline(value);
            }
        });
        self.ctx.emit(" }");
    }

    fn emit_inline_struct_with_spread(&mut self, name: Name, fields: StructLitFieldRange) {
        self.ctx.emit(self.interner.lookup(name));
        let fields = self.arena.get_struct_lit_fields(fields);
        if fields.is_empty() {
            self.ctx.emit(" {}");
            return;
        }
        self.ctx.emit(" { ");
        self.emit_inline_items(fields, |formatter, field| match field {
            StructLitField::Field(init) => {
                formatter.ctx.emit(formatter.interner.lookup(init.name));
                if let Some(value) = init.value {
                    formatter.ctx.emit(": ");
                    formatter.emit_inline(value);
                }
            }
            StructLitField::Spread { expr, .. } => {
                formatter.ctx.emit("...");
                formatter.emit_inline(*expr);
            }
        });
        self.ctx.emit(" }");
    }

    fn emit_inline_tuple(&mut self, items: ExprRange) {
        let items = self.arena.get_expr_list(items);
        self.ctx.emit("(");
        self.emit_inline_items(items, |formatter, &item| formatter.emit_inline(item));
        if items.len() == 1 {
            self.ctx.emit(",");
        }
        self.ctx.emit(")");
    }

    fn emit_inline_range(&mut self, start: ExprId, end: ExprId, step: ExprId, inclusive: bool) {
        if start.is_present() {
            self.emit_inline(start);
        }
        self.ctx.emit(if inclusive { "..=" } else { ".." });
        if end.is_present() {
            self.emit_inline(end);
        }
        if step.is_present() {
            self.ctx.emit(" by ");
            self.emit_inline(step);
        }
    }

    fn emit_inline_jump(&mut self, keyword: &str, label: Name, value: ExprId) {
        self.ctx.emit(keyword);
        if label != Name::EMPTY {
            self.ctx.emit(":");
            self.ctx.emit(self.interner.lookup(label));
        }
        if value.is_present() {
            self.ctx.emit_space();
            self.emit_inline(value);
        }
    }

    fn emit_inline_prefixed_expr(&mut self, prefix: &str, expr: ExprId) {
        self.ctx.emit(prefix);
        self.emit_inline(expr);
    }

    fn emit_inline_postfixed_expr(&mut self, expr: ExprId, postfix: &str) {
        self.emit_inline(expr);
        self.ctx.emit(postfix);
    }

    fn emit_inline_cast(&mut self, expr: ExprId, ty: ParsedTypeId, fallible: bool) {
        self.emit_inline(expr);
        self.ctx.emit(if fallible { " as? " } else { " as " });
        self.emit_type(self.arena.get_parsed_type(ty));
    }

    fn emit_inline_assign(&mut self, target: ExprId, value: ExprId) {
        self.emit_inline(target);
        self.ctx.emit(" = ");
        self.emit_inline(value);
    }

    fn emit_inline_assign_target(&mut self, root: ExprId, steps: AccessStepRange) {
        self.emit_receiver_inline(root);
        for step in self.arena.get_access_steps(steps) {
            match step {
                AccessStep::Field(field) => self.emit_inline_field_step(*field),
                AccessStep::Index(index) => self.emit_inline_index_step(*index),
            }
        }
    }

    fn emit_inline_field_step(&mut self, field: Name) {
        self.ctx.emit(".");
        self.ctx.emit(self.interner.lookup(field));
    }

    fn emit_inline_index_step(&mut self, index: ExprId) {
        self.ctx.emit("[");
        self.emit_inline(index);
        self.ctx.emit("]");
    }

    fn emit_inline_block(&mut self, expr_id: ExprId, stmts: StmtRange, result: ExprId) {
        if !self.arena.get_stmt_range(stmts).is_empty() {
            self.emit_stacked(expr_id);
        } else if result.is_present() {
            self.ctx.emit("{ ");
            self.emit_inline(result);
            self.ctx.emit(" }");
        } else {
            self.ctx.emit("{}");
        }
    }

    fn emit_inline_function_exp(&mut self, exp_id: FunctionExpId) {
        let exp = self.arena.get_function_exp(exp_id);
        self.ctx.emit(exp.kind.name());
        self.ctx.emit("(");
        self.emit_inline_items(self.arena.get_named_exprs(exp.props), |formatter, prop| {
            formatter.ctx.emit(formatter.interner.lookup(prop.name));
            formatter.ctx.emit(": ");
            formatter.emit_inline(prop.value);
        });
        self.ctx.emit(")");
    }

    fn emit_inline_template_full(&mut self, name: Name) {
        self.ctx.emit("`");
        self.ctx
            .emit(crate::escape_template_text(self.interner.lookup(name)));
        self.ctx.emit("`");
    }

    fn emit_inline_template_literal(&mut self, head: Name, parts: TemplatePartRange) {
        self.ctx.emit("`");
        self.ctx
            .emit(crate::escape_template_text(self.interner.lookup(head)));
        for part in self.arena.get_template_parts(parts) {
            self.ctx.emit("{");
            self.emit_inline(part.expr);
            if part.format_spec != Name::EMPTY {
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

    /// Emits a binary operand, adding parentheses when precedence requires them.
    fn emit_binary_operand_inline(
        &mut self,
        operand: ExprId,
        parent_op: BinaryOp,
        side: BinaryOperandSide,
    ) {
        let needs_parens = needs_binary_parens(self.arena, operand, parent_op, side);
        self.emit_parenthesized_if(needs_parens, operand, Self::emit_inline);
    }
}
