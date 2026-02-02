//! Formatter Core (Layer 5: Orchestration)
//!
//! Top-down rendering engine that orchestrates all formatter layers to produce
//! formatted output. This is the main entry point for expression formatting.
//!
//! # 5-Layer Architecture
//!
//! This formatter integrates with the 5-layer architecture:
//!
//! - **Layer 1 (Spacing)**: O(1) token spacing rules via `spacing::RulesMap`
//! - **Layer 2 (Packing)**: Container packing decisions via `packing::Packing`
//! - **Layer 3 (Shape)**: Width tracking via `shape::Shape`
//! - **Layer 4 (Rules)**: Breaking rules via `rules::*Rule` structs
//! - **Layer 5 (Orchestration)**: This module - coordinates all layers
//!
//! # Algorithm
//!
//! 1. For each node, check if it's an always-stacked construct
//! 2. If not, check if inline width + current column <= 100
//! 3. If it fits, render inline
//! 4. Otherwise, render broken (consulting Layer 4 rules)
//!
//! Nested constructs break independently based on their own width.
//!
//! # Layer Integration Points
//!
//! - `helpers::is_simple_item()` → Layer 2 `packing::is_simple_item()`
//! - `helpers::format_receiver()` → Layer 4 `rules::needs_parens(Receiver)`
//! - `helpers::format_call_target()` → Layer 4 `rules::needs_parens(CallTarget)`
//! - `helpers::format_iter()` → Layer 4 `rules::needs_parens(IteratorSource)`
//!
//! # Modules
//!
//! - [`inline`]: Single-line expression rendering
//! - [`broken`]: Multi-line expression rendering
//! - [`stacked`]: Always-multi-line constructs (run, try, match)
//! - [`patterns`]: Match and binding pattern rendering
//! - [`literals`]: Literal value rendering
//! - [`helpers`]: Collection and wrapper helpers (Layer 2, 4 integration)

mod broken;
mod helpers;
mod inline;
mod literals;
mod patterns;
mod stacked;
#[cfg(test)]
mod tests;

use crate::context::{FormatConfig, FormatContext};
use crate::emitter::StringEmitter;
use crate::width::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{BinaryOp, ExprArena, ExprId, StringLookup, UnaryOp};

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

// Note: Parentheses logic moved to rules::ParenthesesRule (Layer 4)
// See rules::needs_parens() and rules::ParenPosition

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
}

/// Format an expression to a string.
pub fn format_expr<I: StringLookup>(arena: &ExprArena, interner: &I, expr_id: ExprId) -> String {
    Formatter::new(arena, interner).format_expr(expr_id)
}
