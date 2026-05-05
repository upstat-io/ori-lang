//! `ChainedElseIfRule`: chained `else if` always breaks (each on own line).
//!
//! # Decision
//!
//! When an `if` expression has at least one `else if` clause, the formatter
//! breaks the chain so each `else if` and the final `else` sit on their own
//! line, regardless of whether the whole expression would fit within the
//! width budget.
//!
//! # Spec Reference
//!
//! Spec: Annex D §If-Then-Else line 755 — chained `else if` each on own line.

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Rule for chained `else if` formatting.
///
/// # Principle
///
/// First `if` stays with the assignment; every `else if` and the final
/// `else` go on their own indented line.
///
/// # Example
///
/// ```ori
/// let size = if n < 10 then "small"
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
