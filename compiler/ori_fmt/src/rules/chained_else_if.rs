//! `ChainedElseIfRule`: Kotlin-style if-else-if formatting.
//!
//! # Decision
//!
//! Kotlin style: first `if` stays with assignment, else clauses indented.
//!
//! # Spec Update Required
//!
//! This rule differs from current spec. Lines 432-436 will need updating.
//!
//! # Spec Reference
//!
//! Lines 428-444

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Rule for chained if-else-if formatting.
///
/// # Principle
///
/// Kotlin style: first `if` stays with assignment, else clauses on own lines.
///
/// # Example (NEW - Kotlin style)
///
/// ```ori
/// let size = if n < 10 then "small"
///     else if n < 100 then "medium"
///     else "large"
/// ```
///
/// # Example (OLD spec - to be replaced)
///
/// ```ori
/// let size =
///     if n < 10 then "small"
///     else if n < 100 then "medium"
///     else "large"
/// ```
pub struct ChainedElseIfRule;

impl ChainedElseIfRule {
    /// Check if an if expression has else-if chains.
    pub fn has_else_if_chain(arena: &ExprArena, expr_id: ExprId) -> bool {
        let expr = arena.get_expr(expr_id);

        if let ExprKind::If { else_branch, .. } = &expr.kind {
            if else_branch.is_present() {
                let else_expr = arena.get_expr(*else_branch);
                return matches!(else_expr.kind, ExprKind::If { .. });
            }
        }

        false
    }

    /// Count the depth of else-if chains.
    ///
    /// Returns 0 for simple if, 1 for if-else, 2+ for if-else-if chains.
    pub fn chain_depth(arena: &ExprArena, expr_id: ExprId) -> usize {
        let expr = arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::If { else_branch, .. } if else_branch.is_present() => {
                let else_expr = arena.get_expr(*else_branch);
                if matches!(else_expr.kind, ExprKind::If { .. }) {
                    1 + Self::chain_depth(arena, *else_branch)
                } else {
                    1
                }
            }
            // For simple if (no else) or non-if expressions, depth is 0
            _ => 0,
        }
    }
}

/// Collected if-else-if chain for formatting.
#[derive(Debug)]
pub struct IfChain {
    /// The initial if condition.
    pub condition: ExprId,

    /// The then branch.
    pub then_branch: ExprId,

    /// Collected else-if branches.
    pub else_ifs: Vec<ElseIfBranch>,

    /// Final else branch (if any).
    pub final_else: Option<ExprId>,
}

/// An else-if branch in the chain.
#[derive(Debug)]
pub struct ElseIfBranch {
    /// The condition for this else-if.
    pub condition: ExprId,

    /// The then branch.
    pub then_branch: ExprId,
}

impl IfChain {
    /// Total number of branches (including initial if).
    pub fn branch_count(&self) -> usize {
        1 + self.else_ifs.len() + usize::from(self.final_else.is_some())
    }

    /// Check if this is a simple if (no else-if, maybe else).
    pub fn is_simple(&self) -> bool {
        self.else_ifs.is_empty()
    }
}

/// Collect an if-else-if chain from an if expression.
pub fn collect_if_chain(arena: &ExprArena, expr_id: ExprId) -> Option<IfChain> {
    let expr = arena.get_expr(expr_id);

    let ExprKind::If {
        cond,
        then_branch,
        else_branch,
    } = &expr.kind
    else {
        return None;
    };

    let mut else_ifs = Vec::new();
    let mut current_else = *else_branch;

    // Walk through else-if chain
    while current_else.is_present() {
        let else_expr = arena.get_expr(current_else);

        if let ExprKind::If {
            cond: else_cond,
            then_branch: else_then,
            else_branch: next_else,
        } = &else_expr.kind
        {
            else_ifs.push(ElseIfBranch {
                condition: *else_cond,
                then_branch: *else_then,
            });
            current_else = *next_else;
        } else {
            // Final else (not an if)
            return Some(IfChain {
                condition: *cond,
                then_branch: *then_branch,
                else_ifs,
                final_else: Some(current_else),
            });
        }
    }

    Some(IfChain {
        condition: *cond,
        then_branch: *then_branch,
        else_ifs,
        final_else: None,
    })
}
