//! Control flow evaluation helpers.
//!
//! This module provides loop action types used by the canonical evaluation
//! path (`eval_can`) for break/continue/error handling in loops.

use ori_ir::Name;
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

/// Convert a `ControlAction` to a `LoopAction` for the loop labeled `loop_label`.
///
/// Spec: Clause 16.3.3 — labels scope through arbitrary nesting. A break/continue
/// is consumed by THIS loop only when its target label is empty (innermost) or
/// matches `loop_label`; a non-matching labeled signal re-propagates outward
/// (`LoopAction::Error`) so the enclosing loop with the matching label consumes
/// it. Errors and `?`-propagation ALWAYS re-propagate, regardless of labels.
pub fn to_loop_action(action: ControlAction, loop_label: Name) -> LoopAction {
    match action {
        // Error / Propagate short-circuit ALL loops regardless of label — checked
        // FIRST so a runtime error or `?` never participates in label matching.
        ControlAction::Error(_) | ControlAction::Propagate(_) => LoopAction::Error(action),
        // A labeled signal whose target is neither empty nor this loop's label
        // belongs to an enclosing loop — re-propagate it unchanged.
        ControlAction::Break(_, label) | ControlAction::Continue(_, label)
            if label != Name::EMPTY && label != loop_label =>
        {
            LoopAction::Error(action)
        }
        // Targets this loop (empty = innermost, or label matches).
        ControlAction::Continue(v, _) if !matches!(v, Value::Void) => LoopAction::ContinueWith(v),
        ControlAction::Continue(_, _) => LoopAction::Continue,
        ControlAction::Break(v, _) => LoopAction::Break(v),
    }
}
