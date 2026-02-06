//! Collection and Wrapper Helpers
//!
//! Helper methods for emitting collections (lists, tuples), call arguments,
//! and Result/Option wrappers.
//!
//! # Layer Integration
//!
//! This module integrates with:
//! - Layer 2 (Packing): Uses `is_simple_item()` for list packing decisions
//! - Layer 4 (Rules): Uses `ParenthesesRule` for parentheses decisions

use crate::packing;
use crate::rules::ParenPosition;
use crate::width::ALWAYS_STACKED;
use ori_ir::{CallArgRange, ExprId, ExprRange, StringLookup};

use super::Formatter;

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit a wrapper with an optionally-present inner value (Ok, Err).
    ///
    /// Uses `ExprId::INVALID` sentinel to represent absent values.
    pub(super) fn emit_wrapper_inline(&mut self, name: &str, inner: ExprId) {
        self.ctx.emit(name);
        self.ctx.emit("(");
        if inner.is_present() {
            self.emit_inline(inner);
        }
        self.ctx.emit(")");
    }

    /// Emit a wrapper with a required inner value (Some).
    pub(super) fn emit_wrapper_inline_required(&mut self, name: &str, inner: ExprId) {
        self.ctx.emit(name);
        self.ctx.emit("(");
        self.emit_inline(inner);
        self.ctx.emit(")");
    }

    pub(super) fn emit_inline_expr_list(&mut self, list: ExprRange) {
        for (i, item) in self.arena.get_expr_list(list).iter().copied().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.emit_inline(item);
        }
    }

    pub(super) fn emit_inline_call_args(&mut self, range: CallArgRange) {
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
    ///
    /// # Layer Integration
    ///
    /// Uses `ParenthesesRule` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn emit_receiver_inline(&mut self, receiver: ExprId) {
        if crate::rules::needs_parens(self.arena, receiver, ParenPosition::Receiver) {
            self.ctx.emit("(");
            self.emit_inline(receiver);
            self.ctx.emit(")");
        } else {
            self.emit_inline(receiver);
        }
    }

    /// Format a receiver expression, wrapping in parentheses if needed for precedence.
    ///
    /// # Layer Integration
    ///
    /// Uses `ParenthesesRule` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn format_receiver(&mut self, receiver: ExprId) {
        if crate::rules::needs_parens(self.arena, receiver, ParenPosition::Receiver) {
            self.ctx.emit("(");
            self.format(receiver);
            self.ctx.emit(")");
        } else {
            self.format(receiver);
        }
    }

    /// Emit a call target expression inline, wrapping in parentheses if needed for precedence.
    ///
    /// Call targets like `(x -> x + 1)(5)` or `(if cond then f else g)(5)` need
    /// parentheses to be parsed correctly.
    ///
    /// # Layer Integration
    ///
    /// Uses `ParenthesesRule` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn emit_call_target_inline(&mut self, func: ExprId) {
        if crate::rules::needs_parens(self.arena, func, ParenPosition::CallTarget) {
            self.ctx.emit("(");
            self.emit_inline(func);
            self.ctx.emit(")");
        } else {
            self.emit_inline(func);
        }
    }

    /// Format a call target expression, wrapping in parentheses if needed for precedence.
    ///
    /// # Layer Integration
    ///
    /// Uses `ParenthesesRule` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn format_call_target(&mut self, func: ExprId) {
        if crate::rules::needs_parens(self.arena, func, ParenPosition::CallTarget) {
            self.ctx.emit("(");
            self.format(func);
            self.ctx.emit(")");
        } else {
            self.format(func);
        }
    }

    /// Emit a for-loop iterator expression inline, wrapping in parentheses if needed.
    ///
    /// Iterator expressions like `(for y in items yield y)` need parentheses
    /// to avoid ambiguity with the outer `for` loop.
    ///
    /// # Layer Integration
    ///
    /// Uses `ParenthesesRule` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn emit_iter_inline(&mut self, iter: ExprId) {
        if crate::rules::needs_parens(self.arena, iter, ParenPosition::IteratorSource) {
            self.ctx.emit("(");
            self.emit_inline(iter);
            self.ctx.emit(")");
        } else {
            self.emit_inline(iter);
        }
    }

    /// Format a for-loop iterator expression, wrapping in parentheses if needed.
    ///
    /// # Layer Integration
    ///
    /// Uses `ParenthesesRule` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn format_iter(&mut self, iter: ExprId) {
        if crate::rules::needs_parens(self.arena, iter, ParenPosition::IteratorSource) {
            self.ctx.emit("(");
            self.format(iter);
            self.ctx.emit(")");
        } else {
            self.format(iter);
        }
    }

    pub(super) fn emit_broken_expr_list(&mut self, list: ExprRange) {
        if list.is_empty() {
            return;
        }

        let items: Vec<_> = self.arena.get_expr_list(list).to_vec();
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

    pub(super) fn emit_broken_call_args(&mut self, range: CallArgRange) {
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
    ///
    /// # Layer Integration
    ///
    /// Delegates to `packing::is_simple_item()` from Layer 2 (Container Packing).
    pub(super) fn is_simple_item(&self, expr_id: ExprId) -> bool {
        packing::is_simple_item(self.arena, expr_id)
    }

    pub(super) fn emit_broken_list(&mut self, items: &[ExprId]) {
        // If any item is complex, format one per line
        let all_simple = items.iter().all(|id| self.is_simple_item(*id));

        if all_simple {
            self.emit_broken_list_wrap(items);
        } else {
            self.emit_broken_list_one_per_line(items);
        }
    }

    /// Emit broken list with multiple simple items per line (wrapping).
    pub(super) fn emit_broken_list_wrap(&mut self, items: &[ExprId]) {
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
    pub(super) fn emit_broken_list_one_per_line(&mut self, items: &[ExprId]) {
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
