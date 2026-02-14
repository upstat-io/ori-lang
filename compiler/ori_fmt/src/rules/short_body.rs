//! `ShortBodyRule`: Keep short bodies with yield/do.
//!
//! # Decision
//!
//! ~20 character threshold. Bodies under 20 chars stay with yield/do.
//!
//! # Spec Reference
//!
//! Lines 751-766

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Rule for short body formatting.
///
/// # Principle
///
/// "A simple body must remain with yield/do even when overall line is long"
/// "A lone identifier or literal never appears on its own line"
///
/// This prevents awkward formatting like:
/// ```ori
/// // Bad (identifier alone on line):
/// for user in users yield
///     user
///
/// // Good (short body stays with yield):
/// for user in users yield user
/// ```
///
/// # Threshold
///
/// Bodies of ~20 characters or less stay with yield/do.
/// This includes single identifiers, literals, and short expressions.
pub struct ShortBodyRule;

impl ShortBodyRule {
    /// Maximum characters for a "short" body.
    pub const THRESHOLD: usize = 20;
}

/// Where to break a for expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakPoint {
    /// Break before `for` (entire expression on new line).
    BeforeFor,

    /// Break after `yield`/`do` (body on new line).
    AfterYield,

    /// No break needed (fits inline).
    NoBreak,
}

/// Check if an expression is a "short body" that should stay with yield/do.
///
/// Short bodies include:
/// - Identifiers
/// - Literals (int, float, string, char, bool)
/// - None, Unit
/// - Expressions whose inline width is ≤ threshold
pub fn is_short_body(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);

    match &expr.kind {
        // Short expressions: identifiers, literals, none, unit, self, config,
        // and continue/break without value
        ExprKind::Ident(_)
        | ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Bool(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::None
        | ExprKind::Unit
        | ExprKind::SelfRef
        | ExprKind::Const(_) => true,
        ExprKind::Continue { value, .. } | ExprKind::Break { value, .. } if !value.is_present() => {
            true
        }

        // For other expressions, we'd need width calculation
        // which is done at the orchestration layer
        _ => false,
    }
}

/// Check if a body expression is "always short" (definitely under threshold).
///
/// These are expressions that are guaranteed to be short enough
/// without needing width calculation.
pub fn is_always_short(arena: &ExprArena, expr_id: ExprId) -> bool {
    // Same logic as is_short_body for now
    // The distinction will matter when we integrate with width calculation
    is_short_body(arena, expr_id)
}

/// Determine break point for a for expression based on body.
///
/// # Returns
///
/// - `NoBreak` if the entire expression fits
/// - `AfterYield` if body is complex (break after yield, indent body)
/// - `BeforeFor` if body is short but line is long (break before for)
///
/// Note: This returns a suggestion; actual decision depends on width.
pub fn suggest_break_point(arena: &ExprArena, body: ExprId) -> BreakPoint {
    if is_short_body(arena, body) {
        // Short body: prefer keeping with yield
        // If line is too long, break before `for`, not after `yield`
        BreakPoint::BeforeFor
    } else {
        // Complex body: break after yield
        BreakPoint::AfterYield
    }
}
