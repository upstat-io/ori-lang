//! `LoopRule`: Complex body breaks.
//!
//! # Decision
//!
//! Complex body (contains run/try/match/for) always breaks.
//!
//! # Spec Reference
//!
//! Lines 589-617

use ori_ir::{ExprArena, ExprId, ExprKind, FunctionSeq};

/// Rule for loop expression formatting.
///
/// # Principle
///
/// "When loop contains complex body (run, try, match, for), break after loop("
///
/// Simple loop bodies can stay inline. Complex bodies always break.
///
/// # Example
///
/// ```ori
/// // Simple body (can inline):
/// loop(if done then break else continue)
///
/// // Complex body (always breaks):
/// loop(
///     run(
///         let input = read_line(),
///         if input == "quit" then break else continue,
///     )
/// )
/// ```
pub struct LoopRule;

impl LoopRule {
    /// Check if a loop body is "complex" (requires breaking).
    ///
    /// Complex bodies contain:
    /// - run, try, match expressions
    /// - for loops
    /// - Other loops
    pub fn has_complex_body(arena: &ExprArena, body: ExprId) -> bool {
        let body_expr = arena.get_expr(body);

        match &body_expr.kind {
            // Function sequences are complex
            ExprKind::FunctionSeq(seq) => matches!(
                seq,
                FunctionSeq::Run { .. } | FunctionSeq::Try { .. } | FunctionSeq::Match { .. }
            ),

            // For loops, nested loops, and match expressions are complex
            ExprKind::For { .. } | ExprKind::Loop { .. } | ExprKind::Match { .. } => true,

            // Everything else is simple
            _ => false,
        }
    }

    /// Check if a loop can potentially be inlined.
    ///
    /// Returns `true` if the body is simple enough to consider inlining.
    pub fn can_try_inline(arena: &ExprArena, body: ExprId) -> bool {
        !Self::has_complex_body(arena, body)
    }
}

/// Check if an expression is a loop expression.
pub fn is_loop(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);
    matches!(expr.kind, ExprKind::Loop { .. })
}

/// Get the body of a loop expression.
pub fn get_loop_body(arena: &ExprArena, expr_id: ExprId) -> Option<ExprId> {
    let expr = arena.get_expr(expr_id);

    if let ExprKind::Loop { body } = &expr.kind {
        Some(*body)
    } else {
        None
    }
}

/// Check if a loop body is a simple conditional (common pattern).
///
/// Returns true for patterns like:
/// - `if condition then break else continue`
/// - `if condition then break(value)`
pub fn is_simple_conditional_body(arena: &ExprArena, body: ExprId) -> bool {
    let body_expr = arena.get_expr(body);

    if let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &body_expr.kind
    {
        // Check if then branch is break/continue
        let then_expr = arena.get_expr(*then_branch);
        let then_simple = matches!(then_expr.kind, ExprKind::Break(_) | ExprKind::Continue(_));

        // Check if else branch is break/continue (if present)
        let else_simple = else_branch.is_none_or(|else_id| {
            let else_expr = arena.get_expr(else_id);
            matches!(else_expr.kind, ExprKind::Break(_) | ExprKind::Continue(_))
        });

        return then_simple && else_simple;
    }

    false
}
