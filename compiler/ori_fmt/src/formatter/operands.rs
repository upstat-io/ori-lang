//! Operand Emission
//!
//! Formatter methods for emitting the operands of compound expressions:
//! collections (lists, tuples), call arguments, receivers, call targets,
//! iterator sources, and Result/Option wrappers, inline or broken.
//!
//! # Layer Integration
//!
//! This module integrates with:
//! - Layer 2 (Packing): Uses `is_simple_item()` for list packing decisions
//! - Layer 4 (Rules): Uses `needs_parens` for parentheses decisions

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

    /// Emit `items` comma-separated on one line, each rendered via
    /// `emit_item` — the inline-mode counterpart of [`Self::emit_broken_items`];
    /// the SSOT loop skeleton shared by every comma-joined inline
    /// collection / call-argument / struct-literal emitter.
    pub(super) fn emit_inline_items<T>(
        &mut self,
        items: &[T],
        mut emit_item: impl FnMut(&mut Self, &T),
    ) {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            emit_item(self, item);
        }
    }

    pub(super) fn emit_inline_expr_list(&mut self, list: ExprRange) {
        let items = self.arena.get_expr_list(list);
        self.emit_inline_items(items, |s, &item| s.emit_inline(item));
    }

    pub(super) fn emit_inline_call_args(&mut self, range: CallArgRange) {
        let args = self.arena.get_call_args(range);
        self.emit_inline_items(args, |s, arg| {
            if let Some(name) = arg.name {
                s.ctx.emit(s.interner.lookup(name));
                s.ctx.emit(": ");
            }
            s.emit_inline(arg.value);
        });
    }

    /// Emit `expr` via `emit`, wrapping it in parentheses first when
    /// `needs_parens` is true — the SSOT paren-wrap skeleton shared by every
    /// operand position needing conditional parenthesization (receiver, call
    /// target, iterator source, binary operand).
    pub(super) fn emit_parenthesized_if(
        &mut self,
        needs_parens: bool,
        expr: ExprId,
        emit: impl FnOnce(&mut Self, ExprId),
    ) {
        if needs_parens {
            self.ctx.emit("(");
            emit(self, expr);
            self.ctx.emit(")");
        } else {
            emit(self, expr);
        }
    }

    /// Emit a receiver expression inline, wrapping in parentheses if needed for precedence.
    ///
    /// # Layer Integration
    ///
    /// Uses `needs_parens` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn emit_receiver_inline(&mut self, receiver: ExprId) {
        let needs_parens =
            crate::rules::needs_parens(self.arena, receiver, ParenPosition::Receiver);
        self.emit_parenthesized_if(needs_parens, receiver, Self::emit_inline);
    }

    /// Force broken rendering when `receiver` is itself a method chain, so
    /// all chain elements break together (`MethodChainRule::ALL_METHODS_BREAK`).
    ///
    /// INVARIANT: an outer broken call must not re-`format()` (fit-check) its
    /// chain receiver — doing so could pick inline rendering and produce
    /// partial chain breaks that violate idempotence across passes.
    pub(super) fn format_receiver_broken(&mut self, receiver: ExprId) {
        let is_chain = matches!(
            self.arena.get_expr(receiver).kind,
            ori_ir::ExprKind::MethodCall { .. } | ori_ir::ExprKind::MethodCallNamed { .. }
        );
        let needs_parens =
            crate::rules::needs_parens(self.arena, receiver, ParenPosition::Receiver);
        self.emit_parenthesized_if(needs_parens, receiver, |s, r| {
            if is_chain {
                s.format_broken(r);
            } else {
                s.format(r);
            }
        });
    }

    /// Emit a call target expression inline, wrapping in parentheses if needed for precedence.
    ///
    /// Call targets like `(x -> x + 1)(5)` or `(if cond then f else g)(5)` need
    /// parentheses to be parsed correctly.
    ///
    /// # Layer Integration
    ///
    /// Uses `needs_parens` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn emit_call_target_inline(&mut self, func: ExprId) {
        let needs_parens = crate::rules::needs_parens(self.arena, func, ParenPosition::CallTarget);
        self.emit_parenthesized_if(needs_parens, func, Self::emit_inline);
    }

    /// Format a call target expression, wrapping in parentheses if needed for precedence.
    ///
    /// # Layer Integration
    ///
    /// Uses `needs_parens` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn format_call_target(&mut self, func: ExprId) {
        let needs_parens = crate::rules::needs_parens(self.arena, func, ParenPosition::CallTarget);
        self.emit_parenthesized_if(needs_parens, func, Self::format);
    }

    /// Emit a for-loop iterator expression inline, wrapping in parentheses if needed.
    ///
    /// Iterator expressions like `(for y in items yield y)` need parentheses
    /// to avoid ambiguity with the outer `for` loop.
    ///
    /// # Layer Integration
    ///
    /// Uses `needs_parens` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn emit_iter_inline(&mut self, iter: ExprId) {
        let needs_parens =
            crate::rules::needs_parens(self.arena, iter, ParenPosition::IteratorSource);
        self.emit_parenthesized_if(needs_parens, iter, Self::emit_inline);
    }

    /// Format a for-loop iterator expression, wrapping in parentheses if needed.
    ///
    /// # Layer Integration
    ///
    /// Uses `needs_parens` from Layer 4 (Breaking Rules) for parentheses decisions.
    pub(super) fn format_iter(&mut self, iter: ExprId) {
        let needs_parens =
            crate::rules::needs_parens(self.arena, iter, ParenPosition::IteratorSource);
        self.emit_parenthesized_if(needs_parens, iter, Self::format);
    }

    /// Emit `items` one per line, each rendered via `emit_item`, comma-separated,
    /// wrapped in `emit_newline`/`indent` before the first item and
    /// `dedent`/`emit_newline_indent` after the last. Caller opens/closes the
    /// surrounding delimiter and handles the empty-collection case — the SSOT
    /// loop skeleton shared by every broken (multi-line) collection,
    /// call-argument, and struct-literal emitter.
    pub(super) fn emit_broken_items<T>(
        &mut self,
        items: &[T],
        mut emit_item: impl FnMut(&mut Self, &T),
    ) {
        let len = items.len();
        self.ctx.emit_newline();
        self.ctx.indent();
        for (i, item) in items.iter().enumerate() {
            self.ctx.emit_indent();
            emit_item(self, item);
            self.ctx.emit(",");
            if i < len - 1 {
                self.ctx.emit_newline();
            }
        }
        self.ctx.dedent();
        self.ctx.emit_newline_indent();
    }

    pub(super) fn emit_broken_expr_list(&mut self, list: ExprRange) {
        if list.is_empty() {
            return;
        }

        let items = self.arena.get_expr_list(list);
        self.emit_broken_items(items, |s, &item| s.format(item));
    }

    pub(super) fn emit_broken_call_args(&mut self, range: CallArgRange) {
        let args = self.arena.get_call_args(range);
        if args.is_empty() {
            return;
        }

        self.emit_broken_items(args, |s, arg| {
            if let Some(name) = arg.name {
                s.ctx.emit(s.interner.lookup(name));
                s.ctx.emit(": ");
            }
            s.format(arg.value);
        });
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
        self.emit_broken_items(items, |s, &item| s.format(item));
    }
}
