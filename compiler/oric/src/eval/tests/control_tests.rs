//! Tests for control flow evaluation (loop actions).

use crate::eval::exec::control::{to_loop_action, LoopAction};
use ori_patterns::{ControlAction, EvalError, Value};

// Loop Control Tests (using ControlAction enum)

mod loop_control {
    use super::*;

    #[test]
    fn continue_signal_returns_continue() {
        let action = ControlAction::Continue(Value::Void);
        match to_loop_action(action) {
            LoopAction::Continue => {}
            other => panic!("expected Continue, got {other:?}"),
        }
    }

    #[test]
    fn continue_with_value_returns_continue_with() {
        let action = ControlAction::Continue(Value::int(42));
        match to_loop_action(action) {
            LoopAction::ContinueWith(v) => assert_eq!(v, Value::int(42)),
            other => panic!("expected ContinueWith, got {other:?}"),
        }
    }

    #[test]
    fn break_void_returns_break_void() {
        let action = ControlAction::Break(Value::Void);
        match to_loop_action(action) {
            LoopAction::Break(Value::Void) => {}
            other => panic!("expected Break(Void), got {other:?}"),
        }
    }

    #[test]
    fn break_with_value_returns_break_with_value() {
        let action = ControlAction::Break(Value::int(42));
        match to_loop_action(action) {
            LoopAction::Break(v) => assert_eq!(v, Value::int(42)),
            other => panic!("expected Break with value, got {other:?}"),
        }
    }

    #[test]
    fn regular_error_returns_error() {
        let action = ControlAction::from(EvalError::new("some error"));
        match to_loop_action(action) {
            LoopAction::Error(e) => assert_eq!(e.into_eval_error().message, "some error"),
            other => panic!("expected Error, got {other:?}"),
        }
    }
}
