//! Formatter Core
//!
//! Top-down rendering engine that decides inline vs broken format for each node.
//! Uses width calculations to make formatting decisions.
//!
//! # Algorithm
//!
//! 1. For each node, check if it's an always-stacked construct
//! 2. If not, check if inline width + current column <= 100
//! 3. If it fits, render inline
//! 4. Otherwise, render broken
//!
//! Nested constructs break independently based on their own width.

#[cfg(test)]
mod tests;

use crate::context::{FormatConfig, FormatContext};
use crate::emitter::StringEmitter;
use crate::width::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{
    BinaryOp, BindingPattern, CallArgRange, ExprArena, ExprId, ExprKind, ExprRange, MatchPattern,
    SeqBinding, SeqBindingRange, StringLookup, UnaryOp,
};

/// Get string representation of a binary operator.
fn binary_op_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::FloorDiv => "div",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::Range => "..",
        BinaryOp::RangeInclusive => "..=",
        BinaryOp::Coalesce => "??",
    }
}

/// Get string representation of a unary operator.
fn unary_op_str(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
        UnaryOp::Try => "?",
    }
}

/// Check if an expression needs parentheses when used as a receiver for method call,
/// field access, or indexing. This is needed for expressions with lower precedence
/// than member access (`.`), which has the highest precedence.
///
/// Expressions that need parentheses as receivers:
/// - Binary operations (all have lower precedence than `.`)
/// - Unary operations (lower precedence than `.`)
/// - Conditionals, lambdas, etc.
fn needs_receiver_parens(expr: &ori_ir::Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::If { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::Let { .. }
            | ExprKind::Range { .. }
    )
}

/// Formatter for Ori source code.
///
/// Wraps a width calculator and format context to produce formatted output.
/// The formatter makes inline vs broken decisions based on pre-calculated widths.
pub struct Formatter<'a, I: StringLookup> {
    arena: &'a ExprArena,
    interner: &'a I,
    width_calc: WidthCalculator<'a, I>,
    pub(crate) ctx: FormatContext<StringEmitter>,
}

impl<'a, I: StringLookup> Formatter<'a, I> {
    /// Create a new formatter with default config.
    pub fn new(arena: &'a ExprArena, interner: &'a I) -> Self {
        Self::with_config(arena, interner, FormatConfig::default())
    }

    /// Create a new formatter with custom config.
    pub fn with_config(arena: &'a ExprArena, interner: &'a I, config: FormatConfig) -> Self {
        Self {
            arena,
            interner,
            width_calc: WidthCalculator::new(arena, interner),
            ctx: FormatContext::with_config(config),
        }
    }

    /// Set the starting column position for formatting.
    ///
    /// Use this when formatting sub-expressions that continue on the same line
    /// as previous content (e.g., function body after `= `).
    #[must_use]
    pub fn with_starting_column(mut self, column: usize) -> Self {
        self.ctx.set_column(column);
        self
    }

    /// Set the starting indentation level for formatting.
    ///
    /// Use this when formatting sub-expressions that should inherit a specific
    /// indentation level (e.g., function body that breaks to a new line).
    #[must_use]
    pub fn with_indent_level(mut self, level: usize) -> Self {
        for _ in 0..level {
            self.ctx.indent();
        }
        self
    }

    /// Format an expression and return the formatted string.
    pub fn format_expr(mut self, expr_id: ExprId) -> String {
        self.format(expr_id);
        self.ctx.finalize()
    }

    /// Format an expression to the current context.
    pub fn format(&mut self, expr_id: ExprId) {
        let width = self.width_calc.width(expr_id);

        if width == ALWAYS_STACKED {
            self.emit_stacked(expr_id);
        } else if self.ctx.fits(width) {
            self.emit_inline(expr_id);
        } else {
            self.emit_broken(expr_id);
        }
    }

    /// Format an expression in broken mode (force multi-line).
    ///
    /// Use this when the caller has already decided the expression needs to break,
    /// and we don't want the formatter to re-evaluate fit at the current position.
    pub fn format_broken(&mut self, expr_id: ExprId) {
        let width = self.width_calc.width(expr_id);

        if width == ALWAYS_STACKED {
            self.emit_stacked(expr_id);
        } else {
            self.emit_broken(expr_id);
        }
    }

    /// Emit an expression inline (single line).
    fn emit_inline(&mut self, expr_id: ExprId) {
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
                self.emit_inline(*func);
                self.ctx.emit("(");
                self.emit_inline_expr_list(*args);
                self.ctx.emit(")");
            }
            ExprKind::CallNamed { func, args } => {
                self.emit_inline(*func);
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
            }

            // Result/Option wrappers
            ExprKind::Ok(inner) => self.emit_wrapper_inline("Ok", *inner),
            ExprKind::Err(inner) => self.emit_wrapper_inline("Err", *inner),
            ExprKind::Some(inner) => self.emit_wrapper_inline_required("Some", *inner),
            ExprKind::None => self.ctx.emit("None"),

            // Control flow jumps
            ExprKind::Return(val) => {
                self.ctx.emit("return");
                if let Some(val_id) = val {
                    self.ctx.emit_space();
                    self.emit_inline(*val_id);
                }
            }
            ExprKind::Break(val) => {
                self.ctx.emit("break");
                if let Some(val_id) = val {
                    self.ctx.emit_space();
                    self.emit_inline(*val_id);
                }
            }
            ExprKind::Continue => self.ctx.emit("continue"),

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
                self.emit_inline(*iter);
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

    /// Emit an expression in broken (multi-line) format.
    fn emit_broken(&mut self, expr_id: ExprId) {
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
    fn emit_else_branch(&mut self, else_id: ExprId) {
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

    /// Emit an always-stacked construct (run, try, match, etc.).
    fn emit_stacked(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Match { scrutinee, arms } => {
                self.ctx.emit("match(");
                self.format(*scrutinee);
                self.ctx.emit(",");
                let arms_list = self.arena.get_arms(*arms);
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
    fn emit_function_seq(&mut self, seq: &ori_ir::FunctionSeq) {
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
                self.ctx.emit("match(");
                self.format(*scrutinee);
                self.ctx.emit(",");
                let arms_list = self.arena.get_arms(*arms);
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
            // Note: mutable is default, immutable uses $ prefix in pattern
            SeqBinding::Let {
                pattern,
                ty: _,
                value,
                mutable: _,
                span: _,
            } => {
                self.ctx.emit("let ");
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
    fn emit_stmt(&mut self, stmt: &ori_ir::Stmt) {
        match &stmt.kind {
            ori_ir::StmtKind::Expr(expr) => self.format(*expr),
            // Note: mutable is default, immutable uses $ prefix in pattern
            ori_ir::StmtKind::Let {
                pattern,
                ty: _,
                init,
                mutable: _,
            } => {
                self.ctx.emit("let ");
                self.emit_binding_pattern(pattern);
                self.ctx.emit(" = ");
                self.format(*init);
            }
        }
    }

    /// Emit a match pattern.
    fn emit_match_pattern(&mut self, pattern: &MatchPattern) {
        match pattern {
            MatchPattern::Wildcard => self.ctx.emit("_"),
            MatchPattern::Binding(name) => {
                self.ctx.emit(self.interner.lookup(*name));
            }
            MatchPattern::Literal(expr_id) => {
                self.emit_inline(*expr_id);
            }
            MatchPattern::Variant { name, inner } => {
                self.ctx.emit(self.interner.lookup(*name));
                let inner_list = self.arena.get_match_pattern_list(*inner);
                if !inner_list.is_empty() {
                    self.ctx.emit("(");
                    for (i, pat_id) in inner_list.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        let pat = self.arena.get_match_pattern(*pat_id);
                        self.emit_match_pattern(pat);
                    }
                    self.ctx.emit(")");
                }
            }
            MatchPattern::Struct { fields } => {
                self.ctx.emit("{ ");
                for (i, (field_name, pat_opt)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit(self.interner.lookup(*field_name));
                    if let Some(pat_id) = pat_opt {
                        self.ctx.emit(": ");
                        let pat = self.arena.get_match_pattern(*pat_id);
                        self.emit_match_pattern(pat);
                    }
                }
                self.ctx.emit(" }");
            }
            MatchPattern::Tuple(items) => {
                let items_list = self.arena.get_match_pattern_list(*items);
                self.ctx.emit("(");
                for (i, pat_id) in items_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let pat = self.arena.get_match_pattern(*pat_id);
                    self.emit_match_pattern(pat);
                }
                // Single-element tuples need trailing comma: (x,) vs (x)
                if items_list.len() == 1 {
                    self.ctx.emit(",");
                }
                self.ctx.emit(")");
            }
            MatchPattern::List { elements, rest } => {
                let elements_list = self.arena.get_match_pattern_list(*elements);
                self.ctx.emit("[");
                for (i, pat_id) in elements_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let pat = self.arena.get_match_pattern(*pat_id);
                    self.emit_match_pattern(pat);
                }
                if let Some(rest_name) = rest {
                    if !elements_list.is_empty() {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("..");
                    self.ctx.emit(self.interner.lookup(*rest_name));
                }
                self.ctx.emit("]");
            }
            MatchPattern::Range {
                start,
                end,
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
            }
            MatchPattern::Or(patterns) => {
                let patterns_list = self.arena.get_match_pattern_list(*patterns);
                for (i, pat_id) in patterns_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(" | ");
                    }
                    let pat = self.arena.get_match_pattern(*pat_id);
                    self.emit_match_pattern(pat);
                }
            }
            MatchPattern::At { name, pattern } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" @ ");
                let pat = self.arena.get_match_pattern(*pattern);
                self.emit_match_pattern(pat);
            }
        }
    }

    /// Emit a binding pattern.
    fn emit_binding_pattern(&mut self, pattern: &BindingPattern) {
        match pattern {
            BindingPattern::Name(name) => {
                self.ctx.emit(self.interner.lookup(*name));
            }
            BindingPattern::Tuple(items) => {
                self.ctx.emit("(");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_binding_pattern(item);
                }
                // Single-element tuples need trailing comma: (x,) vs (x)
                if items.len() == 1 {
                    self.ctx.emit(",");
                }
                self.ctx.emit(")");
            }
            BindingPattern::Struct { fields } => {
                self.ctx.emit("{ ");
                for (i, (field_name, rename)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit(self.interner.lookup(*field_name));
                    if let Some(pat) = rename {
                        self.ctx.emit(": ");
                        self.emit_binding_pattern(pat);
                    }
                }
                self.ctx.emit(" }");
            }
            BindingPattern::List { elements, rest } => {
                self.ctx.emit("[");
                for (i, item) in elements.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_binding_pattern(item);
                }
                if let Some(rest_name) = rest {
                    if !elements.is_empty() {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("..");
                    self.ctx.emit(self.interner.lookup(*rest_name));
                }
                self.ctx.emit("]");
            }
            BindingPattern::Wildcard => {
                self.ctx.emit("_");
            }
        }
    }

    // Helper methods for emitting literals

    fn emit_int(&mut self, n: i64) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        let _ = write!(buf, "{n}");
        self.ctx.emit(&buf);
    }

    fn emit_float(&mut self, f: f64) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        if f.fract() == 0.0 {
            let _ = write!(buf, "{f:.1}");
        } else {
            let _ = write!(buf, "{f}");
        }
        self.ctx.emit(&buf);
    }

    fn emit_string(&mut self, s: &str) {
        self.ctx.emit("\"");
        for c in s.chars() {
            match c {
                '\\' => self.ctx.emit("\\\\"),
                '"' => self.ctx.emit("\\\""),
                '\n' => self.ctx.emit("\\n"),
                '\t' => self.ctx.emit("\\t"),
                '\r' => self.ctx.emit("\\r"),
                '\0' => self.ctx.emit("\\0"),
                _ => {
                    let mut buf = [0; 4];
                    self.ctx.emit(c.encode_utf8(&mut buf));
                }
            }
        }
        self.ctx.emit("\"");
    }

    fn emit_char(&mut self, c: char) {
        self.ctx.emit("'");
        match c {
            '\\' => self.ctx.emit("\\\\"),
            '\'' => self.ctx.emit("\\'"),
            '\n' => self.ctx.emit("\\n"),
            '\t' => self.ctx.emit("\\t"),
            '\r' => self.ctx.emit("\\r"),
            '\0' => self.ctx.emit("\\0"),
            _ => {
                let mut buf = [0; 4];
                self.ctx.emit(c.encode_utf8(&mut buf));
            }
        }
        self.ctx.emit("'");
    }

    fn emit_duration(&mut self, value: u64, unit: ori_ir::DurationUnit) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        let _ = write!(buf, "{value}");
        self.ctx.emit(&buf);
        self.ctx.emit(unit.suffix());
    }

    fn emit_size(&mut self, value: u64, unit: ori_ir::SizeUnit) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        let _ = write!(buf, "{value}");
        self.ctx.emit(&buf);
        self.ctx.emit(unit.suffix());
    }

    // Helper methods for Result/Option wrappers

    /// Emit a wrapper with an optional inner value (Ok, Err).
    fn emit_wrapper_inline(&mut self, name: &str, inner: Option<ExprId>) {
        self.ctx.emit(name);
        self.ctx.emit("(");
        if let Some(val) = inner {
            self.emit_inline(val);
        }
        self.ctx.emit(")");
    }

    /// Emit a wrapper with a required inner value (Some).
    fn emit_wrapper_inline_required(&mut self, name: &str, inner: ExprId) {
        self.ctx.emit(name);
        self.ctx.emit("(");
        self.emit_inline(inner);
        self.ctx.emit(")");
    }

    // Helper methods for emitting collections

    fn emit_inline_expr_list(&mut self, range: ExprRange) {
        let items = self.arena.get_expr_list(range);
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.emit_inline(*item);
        }
    }

    fn emit_inline_call_args(&mut self, range: CallArgRange) {
        let args = self.arena.get_call_args(range);
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            if let Some(name) = arg.name {
                self.ctx.emit(self.interner.lookup(name));
                self.ctx.emit(": ");
            }
            self.emit_inline(arg.value);
        }
    }

    /// Emit a receiver expression inline, wrapping in parentheses if needed for precedence.
    fn emit_receiver_inline(&mut self, receiver: ExprId) {
        let expr = self.arena.get_expr(receiver);
        if needs_receiver_parens(expr) {
            self.ctx.emit("(");
            self.emit_inline(receiver);
            self.ctx.emit(")");
        } else {
            self.emit_inline(receiver);
        }
    }

    /// Format a receiver expression, wrapping in parentheses if needed for precedence.
    fn format_receiver(&mut self, receiver: ExprId) {
        let expr = self.arena.get_expr(receiver);
        if needs_receiver_parens(expr) {
            self.ctx.emit("(");
            self.format(receiver);
            self.ctx.emit(")");
        } else {
            self.format(receiver);
        }
    }

    fn emit_broken_expr_list(&mut self, range: ExprRange) {
        let items = self.arena.get_expr_list(range);
        if items.is_empty() {
            return;
        }

        self.ctx.emit_newline();
        self.ctx.indent();
        for (i, item) in items.iter().enumerate() {
            self.ctx.emit_indent();
            self.format(*item);
            self.ctx.emit(",");
            if i < items.len() - 1 {
                self.ctx.emit_newline();
            }
        }
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
    }

    fn emit_broken_call_args(&mut self, range: CallArgRange) {
        let args = self.arena.get_call_args(range);
        if args.is_empty() {
            return;
        }

        self.ctx.emit_newline();
        self.ctx.indent();
        for (i, arg) in args.iter().enumerate() {
            self.ctx.emit_indent();
            if let Some(name) = arg.name {
                self.ctx.emit(self.interner.lookup(name));
                self.ctx.emit(": ");
            }
            self.format(arg.value);
            self.ctx.emit(",");
            if i < args.len() - 1 {
                self.ctx.emit_newline();
            }
        }
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
    }

    /// Check if an expression is "simple" (literal or identifier).
    ///
    /// Simple items wrap multiple per line when broken.
    /// Complex items (structs, calls, nested collections) go one per line.
    fn is_simple_item(&self, expr_id: ExprId) -> bool {
        let expr = self.arena.get_expr(expr_id);
        matches!(
            expr.kind,
            ExprKind::Int(_)
                | ExprKind::Float(_)
                | ExprKind::Bool(_)
                | ExprKind::String(_)
                | ExprKind::Char(_)
                | ExprKind::Unit
                | ExprKind::Duration { .. }
                | ExprKind::Size { .. }
                | ExprKind::Ident(_)
                | ExprKind::Config(_)
                | ExprKind::FunctionRef(_)
                | ExprKind::SelfRef
                | ExprKind::HashLength
                | ExprKind::None
        )
    }

    fn emit_broken_list(&mut self, items: &[ExprId]) {
        // If any item is complex, format one per line
        let all_simple = items.iter().all(|id| self.is_simple_item(*id));

        if all_simple {
            self.emit_broken_list_wrap(items);
        } else {
            self.emit_broken_list_one_per_line(items);
        }
    }

    /// Emit broken list with multiple simple items per line (wrapping).
    fn emit_broken_list_wrap(&mut self, items: &[ExprId]) {
        self.ctx.emit_newline();
        self.ctx.indent();
        self.ctx.emit_indent();
        let line_start = self.ctx.column();
        let max_width = self.ctx.max_width();

        for (i, item) in items.iter().enumerate() {
            let item_width = self.width_calc.width(*item);

            // Check if we need to wrap to a new line
            if item_width != ALWAYS_STACKED
                && self.ctx.column() > line_start
                && self.ctx.column() + item_width + 2 > max_width
            {
                self.ctx.emit(",");
                self.ctx.emit_newline();
                self.ctx.emit_indent();
            } else if i > 0 {
                self.ctx.emit(", ");
            }

            self.format(*item);
        }
        self.ctx.emit(",");
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
    }

    /// Emit broken list with one complex item per line.
    fn emit_broken_list_one_per_line(&mut self, items: &[ExprId]) {
        self.ctx.emit_newline();
        self.ctx.indent();
        for (i, item) in items.iter().enumerate() {
            self.ctx.emit_indent();
            self.format(*item);
            self.ctx.emit(",");
            if i < items.len() - 1 {
                self.ctx.emit_newline();
            }
        }
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
    }
}

/// Format an expression to a string.
pub fn format_expr<I: StringLookup>(arena: &ExprArena, interner: &I, expr_id: ExprId) -> String {
    Formatter::new(arena, interner).format_expr(expr_id)
}
