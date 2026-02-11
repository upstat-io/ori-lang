//! Tests for control flow implementations.
//!
//! Relocated from `exec/control.rs` per coding guidelines (>200 lines).

use crate::exec::control::{to_loop_action, LoopAction};
use ori_patterns::{ControlAction, Value};

mod to_loop_action_tests {
    use super::*;

    #[test]
    fn control_flow_continue_returns_continue() {
        let action = to_loop_action(ControlAction::Continue(Value::Void));
        assert!(matches!(action, LoopAction::Continue));
    }

    #[test]
    fn control_flow_continue_with_value_returns_continue_with() {
        let action = to_loop_action(ControlAction::Continue(Value::int(42)));
        if let LoopAction::ContinueWith(v) = action {
            assert_eq!(v, Value::int(42));
        } else {
            panic!("expected LoopAction::ContinueWith, got {action:?}");
        }
    }

    #[test]
    fn control_flow_break_returns_break_with_value() {
        let action = to_loop_action(ControlAction::Break(Value::int(99)));
        if let LoopAction::Break(v) = action {
            assert_eq!(v, Value::int(99));
        } else {
            panic!("expected LoopAction::Break");
        }
    }

    #[test]
    fn control_flow_break_void_returns_break_void() {
        let action = to_loop_action(ControlAction::Break(Value::Void));
        if let LoopAction::Break(v) = action {
            assert_eq!(v, Value::Void);
        } else {
            panic!("expected LoopAction::Break(Void)");
        }
    }

    #[test]
    fn eval_error_becomes_loop_error() {
        let err = ori_patterns::EvalError::new("test error");
        let action = to_loop_action(ControlAction::from(err));
        if let LoopAction::Error(e) = action {
            assert!(matches!(e, ControlAction::Error(_)));
        } else {
            panic!("expected LoopAction::Error");
        }
    }
}
