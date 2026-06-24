//! `ParenthesesRule`: Preserve user parens, add when semantically needed.
//!
//! # Decision
//!
//! Preserve all user parentheses. Add when semantically needed, never remove.
//!
//! # Current Limitation
//!
//! **User parentheses are NOT currently preserved.** The AST does not track whether
//! parentheses were explicitly written by the user, so [`ParenthesesRule::has_user_parens()`]
//! always returns `false`. This means user-added parentheses for clarity may be removed
//! if they are not semantically required.
//!
//! Future work: Track user parentheses in `ori_ir::Expr` to preserve user intent.
//!
//! Spec: Annex D §Method-Style Match, §Method Chains.

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Rule for parentheses handling.
///
/// # Principle
///
/// "Preserve all user parens. Add when semantically needed, never remove."
///
/// Parentheses are required in certain positions to maintain correct
/// parsing/precedence. User parentheses are always preserved even if
/// not strictly required - they represent user intent for clarity.
///
/// # Required Parentheses
///
/// - Method receiver: `(for x in items yield x).fold(...)`
/// - Call target: `(x -> x * 2)(5)`
/// - Iterator source: `for x in (inner) yield x`
pub struct ParenthesesRule;

impl ParenthesesRule {
    /// Whether the user wrote explicit parentheses around this expression.
    ///
    /// Always returns `false`: `ori_ir::Expr` does not track explicit
    /// parentheses, so user parens that are not semantically required are
    /// dropped. Preserving them requires a `has_explicit_parens` field set
    /// during parsing.
    pub fn has_user_parens(_arena: &ExprArena, _expr_id: ExprId) -> bool {
        false
    }
}

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

    /// Unary operand: -x — operand may need parens
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

        // Unary operands rarely need parens (handled by precedence)
        ParenPosition::UnaryOperand => {
            matches!(&expr.kind, ExprKind::Lambda { .. } | ExprKind::Let { .. })
        }
    }
}

/// Check if expression is "simple" (doesn't need parens in most contexts).
///
/// Simple expressions: identifiers, literals, calls, field access.
pub fn is_simple_expr(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);

    matches!(
        &expr.kind,
        ExprKind::Ident(_)
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::None
            | ExprKind::SelfRef
            | ExprKind::Const(_)
            | ExprKind::Call { .. }
            | ExprKind::CallNamed { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::MethodCallNamed { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index { .. }
    )
}
