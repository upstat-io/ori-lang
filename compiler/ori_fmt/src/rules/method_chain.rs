//! `MethodChainRule`: All-or-nothing method chain breaking.
//!
//! # Decision
//!
//! Strict all-or-nothing: all chain elements break together.
//! The receiver stays on the current line, all methods break.
//!
//! # Spec Reference
//!
//! Lines 493-510

use ori_ir::{ExprArena, ExprId, ExprKind, Name};

/// Rule for method chain formatting.
///
/// # Principle
///
/// "Receiver stays on assignment/yield line, break at every `.` once any break needed"
///
/// Method chains either:
/// - Fit entirely on one line
/// - Break at EVERY `.` with all methods indented
///
/// This is an all-or-nothing decision - no selective breaking.
///
/// # Example
///
/// ```ori
/// // Fits inline:
/// items.map(x -> x * 2).filter(x -> x > 0)
///
/// // Breaks all together:
/// items
///     .map(x -> x * 2)
///     .filter(x -> x > 0)
///     .take(n: 10)
/// ```
pub struct MethodChainRule;

impl MethodChainRule {
    /// All methods break together (not selective).
    pub const ALL_METHODS_BREAK: bool = true;

    /// Minimum number of chained calls to consider for breaking.
    /// Single method calls don't trigger chain breaking logic.
    pub const MIN_CHAIN_LENGTH: usize = 2;
}

/// A collected method chain for formatting.
#[derive(Debug)]
pub struct MethodChain {
    /// The initial receiver expression.
    pub receiver: ExprId,

    /// The chain of method calls.
    pub calls: Vec<ChainedCall>,
}

/// A single call in a method chain.
#[derive(Debug)]
pub struct ChainedCall {
    /// Method name.
    pub name: Name,

    /// Whether this has named args (`MethodCallNamed` vs `MethodCall`).
    pub has_named_args: bool,

    /// The expression ID for this method call.
    pub expr_id: ExprId,
}

impl MethodChain {
    /// Check if this chain is trivial (single method call).
    #[inline]
    pub fn is_trivial(&self) -> bool {
        self.calls.len() < MethodChainRule::MIN_CHAIN_LENGTH
    }

    /// Get the chain length.
    #[inline]
    pub fn len(&self) -> usize {
        self.calls.len()
    }

    /// Check if chain is empty (just receiver, no calls).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

/// Collect a method chain from an expression.
///
/// Walks backwards from the outermost method call to collect
/// the full chain in order (receiver first, then calls).
pub fn collect_method_chain(arena: &ExprArena, expr_id: ExprId) -> Option<MethodChain> {
    let mut calls = Vec::new();
    let mut current = expr_id;

    // Walk back through the chain
    loop {
        let expr = arena.get_expr(current);

        match &expr.kind {
            ExprKind::MethodCall {
                receiver, method, ..
            } => {
                calls.push(ChainedCall {
                    name: *method,
                    has_named_args: false,
                    expr_id: current,
                });
                current = *receiver;
            }
            ExprKind::MethodCallNamed {
                receiver, method, ..
            } => {
                calls.push(ChainedCall {
                    name: *method,
                    has_named_args: true,
                    expr_id: current,
                });
                current = *receiver;
            }
            _ => {
                // End of chain - current is the receiver
                break;
            }
        }
    }

    if calls.is_empty() {
        return None;
    }

    // Reverse to get receiver-first order
    calls.reverse();

    Some(MethodChain {
        receiver: current,
        calls,
    })
}

/// Check if an expression is a method chain.
pub fn is_method_chain(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);
    matches!(
        expr.kind,
        ExprKind::MethodCall { .. } | ExprKind::MethodCallNamed { .. }
    )
}
