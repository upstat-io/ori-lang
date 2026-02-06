//! `RunRule`: Top-level vs nested run formatting.
//!
//! # Decision
//!
//! Top-level run = always stacked; nested run = width-based.
//!
//! # Spec Reference
//!
//! Lines 514-564

use crate::packing::Packing;
use ori_ir::{ExprArena, ExprId, ExprKind, FunctionSeq};

/// Rule for run expression formatting.
///
/// # Principle
///
/// "Top-level run = always stacked; nested run = width-based"
///
/// - Top-level run (function body, not inside another expression):
///   Always stacked with each statement on its own line.
///
/// - Nested run (inside another expression):
///   Can inline if it fits, otherwise stacks.
///
/// # Example
///
/// ```ori
/// // Top-level run (always stacked):
/// @main () -> void = run(
///     let x = 1,
///     print(msg: x.to_str()),
/// )
///
/// // Nested run (width-based, can inline if fits):
/// let logged = run(print(msg: value.to_str()), value)
/// ```
pub struct RunRule;

impl RunRule {
    /// Determine packing for a run expression.
    ///
    /// # Arguments
    ///
    /// * `is_top_level` - Whether this is a top-level run (function body)
    pub fn packing(is_top_level: bool) -> Packing {
        if is_top_level {
            Packing::AlwaysStacked
        } else {
            Packing::FitOrOnePerLine
        }
    }
}

/// Context for determining if a run is "top-level".
#[derive(Clone, Debug, Default)]
pub struct RunContext {
    /// Nesting depth (0 = top-level).
    pub depth: usize,

    /// Whether we're directly in a function body.
    pub is_function_body: bool,
}

impl RunContext {
    /// Create context for function body.
    pub fn function_body() -> Self {
        RunContext {
            depth: 0,
            is_function_body: true,
        }
    }

    /// Create context for nested expression.
    pub fn nested() -> Self {
        RunContext {
            depth: 1,
            is_function_body: false,
        }
    }

    /// Check if this is a top-level context.
    pub fn is_top_level(&self) -> bool {
        self.depth == 0 || self.is_function_body
    }

    /// Enter a nested context.
    #[must_use = "enter_nested returns a new RunContext for nested expressions"]
    pub fn enter_nested(&self) -> Self {
        RunContext {
            depth: self.depth + 1,
            is_function_body: false,
        }
    }
}

/// Check if an expression is a run expression.
pub fn is_run(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);

    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        let seq = arena.get_function_seq(*seq_id);
        return matches!(seq, FunctionSeq::Run { .. });
    }

    false
}

/// Check if an expression is a try expression.
pub fn is_try(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);

    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        let seq = arena.get_function_seq(*seq_id);
        return matches!(seq, FunctionSeq::Try { .. });
    }

    false
}

/// Check if an expression is a match (`function_seq`) expression.
pub fn is_match_seq(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);

    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        let seq = arena.get_function_seq(*seq_id);
        return matches!(seq, FunctionSeq::Match { .. });
    }

    false
}

/// Check if an expression is any `FunctionSeq` (run, try, match).
pub fn is_function_seq(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);
    matches!(expr.kind, ExprKind::FunctionSeq(_))
}

/// Get the `FunctionSeq` data if this is a `function_seq` expression.
pub fn get_function_seq(arena: &ExprArena, expr_id: ExprId) -> Option<&FunctionSeq> {
    let expr = arena.get_expr(expr_id);

    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        Some(arena.get_function_seq(*seq_id))
    } else {
        None
    }
}
