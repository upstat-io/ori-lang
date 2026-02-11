//! Control flow evaluation helpers.
//!
//! This module provides loop action types used by the canonical evaluation
//! path (`eval_can`) for break/continue/error handling in loops.

use ori_patterns::{ControlAction, Value};

/// Result of a for loop iteration.
#[derive(Debug)]
pub enum LoopAction {
    /// Skip current iteration (continue without value)
    Continue,
    /// Substitute yielded value (continue with value in for...yield)
    ContinueWith(Value),
    /// Exit loop with value
    Break(Value),
    /// Propagate error or other non-loop control action
    Error(ControlAction),
}

/// Convert a `ControlAction` to a `LoopAction`.
///
/// Control flow signals (break/continue) become the corresponding `LoopAction`
/// variants. Errors and propagation signals are wrapped in `LoopAction::Error`
/// for re-raising after the loop.
pub fn to_loop_action(action: ControlAction) -> LoopAction {
    match action {
        ControlAction::Continue(v) if !matches!(v, Value::Void) => LoopAction::ContinueWith(v),
        ControlAction::Continue(_) => LoopAction::Continue,
        ControlAction::Break(v) => LoopAction::Break(v),
        other => LoopAction::Error(other),
    }
}
