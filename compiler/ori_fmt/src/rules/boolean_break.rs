//! `BooleanBreakRule`: Break at `||` with leading operator.
//!
//! # Decision
//!
//! 3+ `||` clauses OR exceeds width triggers breaking.
//! When broken, each clause gets its own line with `||` at the start.
//!
//! # Spec Reference
//!
//! Lines 473-483

use ori_ir::{BinaryOp, ExprArena, ExprId, ExprKind};

/// Rule for boolean expression breaking.
///
/// # Principle
///
/// "When a boolean expression contains multiple || clauses,
///  each clause receives its own line with || at the start"
///
/// # Example
///
/// ```ori
/// // 2 clauses (no break unless exceeds width):
/// if a || b then x
///
/// // 3+ clauses (break with leading ||):
/// if user.active && user.verified
///     || user.is_admin
///     || user.bypass_check then x
/// ```
pub struct BooleanBreakRule;

impl BooleanBreakRule {
    /// Minimum number of `||` clauses to trigger automatic breaking.
    pub const OR_THRESHOLD: usize = 3;

    /// Check if expression should break at `||`.
    ///
    /// Returns true if there are 3+ top-level `||` clauses.
    pub fn should_break_at_or(arena: &ExprArena, expr_id: ExprId) -> bool {
        let or_count = Self::count_top_level_or(arena, expr_id);
        or_count >= Self::OR_THRESHOLD
    }

    /// Count top-level `||` operations (not nested inside other exprs).
    fn count_top_level_or(arena: &ExprArena, expr_id: ExprId) -> usize {
        let expr = arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Binary {
                op: BinaryOp::Or,
                left,
                ..
            } => 1 + Self::count_top_level_or(arena, *left),
            _ => 0,
        }
    }
}

/// Collect top-level `||` clauses from an expression.
///
/// Returns clauses in order from first to last.
/// For `a || b || c`, returns `[a, b, c]`.
pub fn collect_or_clauses(arena: &ExprArena, expr_id: ExprId) -> Vec<ExprId> {
    let mut clauses = Vec::new();
    collect_or_clauses_inner(arena, expr_id, &mut clauses);
    clauses.reverse(); // Collected in reverse order
    clauses
}

fn collect_or_clauses_inner(arena: &ExprArena, expr_id: ExprId, clauses: &mut Vec<ExprId>) {
    let expr = arena.get_expr(expr_id);

    match &expr.kind {
        ExprKind::Binary {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            // Right clause first (will be reversed)
            clauses.push(*right);
            // Recurse on left
            collect_or_clauses_inner(arena, *left, clauses);
        }
        _ => {
            // Base case - this is a clause
            clauses.push(expr_id);
        }
    }
}

/// Check if an expression is a boolean OR expression.
pub fn is_or_expression(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);
    matches!(
        &expr.kind,
        ExprKind::Binary {
            op: BinaryOp::Or,
            ..
        }
    )
}
