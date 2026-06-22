//! Tests for control flow implementations.
//!
//! Relocated from `exec/control.rs` per coding guidelines (>200 lines).

use crate::exec::control::{to_loop_action, LoopAction};
use ori_patterns::{ControlAction, Value};

mod to_loop_action_tests {
    use super::*;

    #[test]
    fn control_flow_continue_returns_continue() {
        let action = to_loop_action(
            ControlAction::Continue(Value::Void, ori_ir::Name::EMPTY),
            ori_ir::Name::EMPTY,
        );
        assert!(matches!(action, LoopAction::Continue));
    }

    #[test]
    fn control_flow_continue_with_value_returns_continue_with() {
        let action = to_loop_action(
            ControlAction::Continue(Value::int(42), ori_ir::Name::EMPTY),
            ori_ir::Name::EMPTY,
        );
        if let LoopAction::ContinueWith(v) = action {
            assert_eq!(v, Value::int(42));
        } else {
            panic!("expected LoopAction::ContinueWith, got {action:?}");
        }
    }

    #[test]
    fn control_flow_break_returns_break_with_value() {
        let action = to_loop_action(
            ControlAction::Break(Value::int(99), ori_ir::Name::EMPTY),
            ori_ir::Name::EMPTY,
        );
        if let LoopAction::Break(v) = action {
            assert_eq!(v, Value::int(99));
        } else {
            panic!("expected LoopAction::Break");
        }
    }

    #[test]
    fn control_flow_break_void_returns_break_void() {
        let action = to_loop_action(
            ControlAction::Break(Value::Void, ori_ir::Name::EMPTY),
            ori_ir::Name::EMPTY,
        );
        if let LoopAction::Break(v) = action {
            assert_eq!(v, Value::Void);
        } else {
            panic!("expected LoopAction::Break(Void)");
        }
    }

    #[test]
    fn eval_error_becomes_loop_error() {
        let err = ori_patterns::EvalError::new("test error");
        let action = to_loop_action(ControlAction::from(err), ori_ir::Name::EMPTY);
        if let LoopAction::Error(e) = action {
            assert!(matches!(e, ControlAction::Error(_)));
        } else {
            panic!("expected LoopAction::Error");
        }
    }

    // Spec: Clause 16.3.3 — `?`-propagation short-circuits ALL loops regardless
    // of label. Propagate must NEVER be consumed as a labeled loop signal; it
    // re-propagates so every enclosing loop unwinds. Negative pin for the
    // Error/Propagate-first match ordering.
    #[test]
    fn propagate_bypasses_label_match_and_re_raises() {
        let action = to_loop_action(ControlAction::Propagate(Value::None), ori_ir::Name::EMPTY);
        if let LoopAction::Error(e) = action {
            assert!(matches!(e, ControlAction::Propagate(_)));
        } else {
            panic!("expected Propagate to re-raise as LoopAction::Error, got {action:?}");
        }
    }
}
