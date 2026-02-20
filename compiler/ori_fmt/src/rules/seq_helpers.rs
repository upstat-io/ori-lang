//! `FunctionSeq` helpers: query functions for try, match, and generic `FunctionSeq` expressions.

use ori_ir::{ExprArena, ExprId, ExprKind, FunctionSeq};

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

/// Check if an expression is any `FunctionSeq` (try, match, `for_pattern`).
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
