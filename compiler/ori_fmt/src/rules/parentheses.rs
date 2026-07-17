//! Parentheses rules: preserve user parens, add when semantically needed.
//!
//! # Decision
//!
//! Preserve all user parentheses. Add when semantically needed, never remove.
//!
//! # Current Limitation
//!
//! **User parentheses are NOT currently preserved.** The AST does not track
//! whether parentheses were explicitly written by the user, so user-added
//! parentheses for clarity may be removed if they are not semantically
//! required. Preserving them requires a `has_explicit_parens` field set
//! during parsing.
//!
//! Spec: Annex D §Method-Style Match, §Method Chains.

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Position where parentheses may be required.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParenPosition {
    /// Method receiver: `x.method()` — x needs parens if complex
    Receiver,

    /// Call target: f(args) — f needs parens if complex
    CallTarget,

    /// Iterator source: for x in y — y needs parens if complex
    IteratorSource,

    /// Binary operand: a + b — operand may need parens for precedence
    BinaryOperand,

    /// Unary operand: -x — operand may need parens for precedence
    UnaryOperand,
}

/// Check if an expression needs parentheses in a given position.
///
/// # Arguments
///
/// * `arena` - Expression arena
/// * `expr_id` - The expression to check
/// * `position` - Where the expression appears
///
/// # Returns
///
/// `true` if parentheses are semantically required.
pub fn needs_parens(arena: &ExprArena, expr_id: ExprId, position: ParenPosition) -> bool {
    let expr = arena.get_expr(expr_id);

    match position {
        // Spec: Annex D §Method Chains — method receiver needs parens for complex exprs
        ParenPosition::Receiver => matches!(
            &expr.kind,
            ExprKind::Binary { .. }
                | ExprKind::Unary { .. }
                | ExprKind::If { .. }
                | ExprKind::Lambda { .. }
                | ExprKind::Let { .. }
                | ExprKind::Range { .. }
                | ExprKind::For { .. }
                | ExprKind::Loop { .. }
                | ExprKind::While { .. }
                | ExprKind::FunctionSeq(_)
                | ExprKind::FunctionExp(_)
        ),

        // Spec: Annex D §Method Chains — call target needs parens for complex exprs
        ParenPosition::CallTarget => matches!(
            &expr.kind,
            ExprKind::Binary { .. }
                | ExprKind::Unary { .. }
                | ExprKind::If { .. }
                | ExprKind::Lambda { .. }
                | ExprKind::Let { .. }
                | ExprKind::Range { .. }
                | ExprKind::For { .. }
                | ExprKind::Loop { .. }
                | ExprKind::While { .. }
        ),

        // Spec: Annex D §Width-Based Breaking — iterator source needs parens for for/if/lambda/let
        ParenPosition::IteratorSource => matches!(
            &expr.kind,
            ExprKind::For { .. }
                | ExprKind::If { .. }
                | ExprKind::Lambda { .. }
                | ExprKind::Let { .. }
        ),

        // Binary operands may need parens for precedence
        ParenPosition::BinaryOperand => {
            // Parens needed if operand is lower precedence binary or lambda
            matches!(
                &expr.kind,
                ExprKind::Lambda { .. } | ExprKind::Let { .. } | ExprKind::If { .. }
            )
        }

        // Unary operands need parens for lower-precedence / ambiguous forms.
        ParenPosition::UnaryOperand => matches!(
            &expr.kind,
            ExprKind::Binary { .. }
                | ExprKind::If { .. }
                | ExprKind::Lambda { .. }
                | ExprKind::Let { .. }
        ),
    }
}
