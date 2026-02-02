//! `NestedForRule`: Rust-style indentation for nested for loops.
//!
//! # Decision
//!
//! Rust-style: each nested for increases indentation.
//!
//! # Spec Reference
//!
//! Lines 818-830

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Rule for nested for loop formatting.
///
/// # Principle
///
/// "Each nesting level gets its own line with incremented indentation"
///
/// Nested `for` loops form a pyramid with increasing indentation.
/// The body of the innermost `for` stays with `yield` if short.
///
/// # Example
///
/// ```ori
/// for user in users yield
///     for permission in user.permissions yield
///         for action in permission.actions yield
///             action.name
/// ```
pub struct NestedForRule;

impl NestedForRule {
    /// Check if expression is a nested for loop.
    pub fn is_nested_for(arena: &ExprArena, expr_id: ExprId) -> bool {
        let expr = arena.get_expr(expr_id);

        if let ExprKind::For { body, .. } = &expr.kind {
            let body_expr = arena.get_expr(*body);
            return matches!(body_expr.kind, ExprKind::For { .. });
        }

        false
    }

    /// Count nesting depth of for loops.
    ///
    /// Returns 1 for a single for loop, 2 for for-in-for, etc.
    pub fn nesting_depth(arena: &ExprArena, expr_id: ExprId) -> usize {
        let expr = arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::For { body, .. } => 1 + Self::body_for_depth(arena, *body),
            _ => 0,
        }
    }

    /// Count for depth starting from a body expression.
    fn body_for_depth(arena: &ExprArena, expr_id: ExprId) -> usize {
        let expr = arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::For { body, .. } => 1 + Self::body_for_depth(arena, *body),
            _ => 0,
        }
    }
}

/// Collected nested for chain for formatting.
#[derive(Debug)]
pub struct ForChain {
    /// The for loop levels, from outermost to innermost.
    pub levels: Vec<ForLevel>,

    /// The final body expression (not a for).
    pub body: ExprId,
}

/// A single level in a nested for chain.
#[derive(Debug)]
pub struct ForLevel {
    /// The binding name.
    pub binding: ori_ir::Name,

    /// The iterator expression.
    pub iter: ExprId,

    /// Optional guard condition.
    pub guard: Option<ExprId>,

    /// Whether this is a yield (vs do).
    pub is_yield: bool,
}

impl ForChain {
    /// Check if this is a single (non-nested) for.
    pub fn is_single(&self) -> bool {
        self.levels.len() == 1
    }

    /// Get the nesting depth.
    pub fn depth(&self) -> usize {
        self.levels.len()
    }
}

/// Collect a nested for chain from a for expression.
pub fn collect_for_chain(arena: &ExprArena, expr_id: ExprId) -> Option<ForChain> {
    let mut levels = Vec::new();
    let mut current = expr_id;

    loop {
        let expr = arena.get_expr(current);

        if let ExprKind::For {
            binding,
            iter,
            guard,
            body,
            is_yield,
        } = &expr.kind
        {
            levels.push(ForLevel {
                binding: *binding,
                iter: *iter,
                guard: *guard,
                is_yield: *is_yield,
            });

            // Check if body is another for
            let body_expr = arena.get_expr(*body);
            if matches!(body_expr.kind, ExprKind::For { .. }) {
                current = *body;
            } else {
                // End of chain
                return Some(ForChain {
                    levels,
                    body: *body,
                });
            }
        } else {
            break;
        }
    }

    if levels.is_empty() {
        None
    } else {
        // Should not reach here if we started with a for
        unreachable!("collect_for_chain called on non-for expression")
    }
}

/// Check if an expression is a for loop.
pub fn is_for_expression(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);
    matches!(expr.kind, ExprKind::For { .. })
}
