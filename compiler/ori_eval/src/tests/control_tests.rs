//! Tests for control flow implementations.
//!
//! Relocated from `exec/control.rs` per coding guidelines (>200 lines).

use crate::environment::{Environment, Mutability};
use crate::exec::control::{bind_pattern, eval_if, parse_loop_control, to_loop_action, LoopAction};
use ori_ir::{BindingPattern, ExprId, Name};
use ori_patterns::{EvalError, Value};

mod parse_loop_control_tests {
    use super::*;

    #[test]
    fn continue_returns_continue() {
        let action = parse_loop_control("continue");
        assert!(matches!(action, LoopAction::Continue));
    }

    #[test]
    fn break_void_returns_break_void() {
        let action = parse_loop_control("break:void");
        if let LoopAction::Break(v) = action {
            assert!(matches!(v, Value::Void));
        } else {
            panic!("expected LoopAction::Break");
        }
    }

    #[test]
    fn break_with_value_returns_void_for_now() {
        // Current implementation simplifies to void
        let action = parse_loop_control("break:42");
        if let LoopAction::Break(v) = action {
            assert!(matches!(v, Value::Void));
        } else {
            panic!("expected LoopAction::Break");
        }
    }

    #[test]
    fn unknown_message_returns_error() {
        let action = parse_loop_control("unknown");
        if let LoopAction::Error(e) = action {
            assert_eq!(e.message, "unknown");
        } else {
            panic!("expected LoopAction::Error");
        }
    }
}

mod to_loop_action_tests {
    use super::*;

    #[test]
    fn control_flow_continue_returns_continue() {
        let err = EvalError::continue_signal();
        let action = to_loop_action(err);
        assert!(matches!(action, LoopAction::Continue));
    }

    #[test]
    fn control_flow_break_returns_break_with_value() {
        let err = EvalError::break_with(Value::int(42));
        let action = to_loop_action(err);
        if let LoopAction::Break(v) = action {
            assert_eq!(v, Value::int(42));
        } else {
            panic!("expected LoopAction::Break");
        }
    }

    #[test]
    fn no_control_flow_falls_back_to_string_parsing() {
        let err = EvalError::new("continue");
        let action = to_loop_action(err);
        assert!(matches!(action, LoopAction::Continue));
    }
}

mod bind_pattern_tests {
    use super::*;

    #[test]
    fn name_pattern_binds_value() {
        let mut env = Environment::new();
        let name = Name::from_raw(1);
        let pattern = BindingPattern::Name(name);
        bind_pattern(&pattern, Value::int(42), Mutability::Immutable, &mut env).unwrap();
        assert_eq!(env.lookup(name), Some(Value::int(42)));
    }

    #[test]
    fn wildcard_pattern_succeeds_without_binding() {
        let mut env = Environment::new();
        let result = bind_pattern(
            &BindingPattern::Wildcard,
            Value::int(42),
            Mutability::Immutable,
            &mut env,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn tuple_pattern_binds_elements() {
        let mut env = Environment::new();
        let name1 = Name::from_raw(1);
        let name2 = Name::from_raw(2);
        let pattern = BindingPattern::Tuple(vec![
            BindingPattern::Name(name1),
            BindingPattern::Name(name2),
        ]);
        let tuple = Value::tuple(vec![Value::int(1), Value::int(2)]);
        bind_pattern(&pattern, tuple, Mutability::Immutable, &mut env).unwrap();
        assert_eq!(env.lookup(name1), Some(Value::int(1)));
        assert_eq!(env.lookup(name2), Some(Value::int(2)));
    }

    #[test]
    fn tuple_pattern_mismatch_errors() {
        let mut env = Environment::new();
        let name1 = Name::from_raw(1);
        let pattern = BindingPattern::Tuple(vec![BindingPattern::Name(name1)]);
        let tuple = Value::tuple(vec![Value::int(1), Value::int(2)]);
        let result = bind_pattern(&pattern, tuple, Mutability::Immutable, &mut env);
        assert!(result.is_err());
    }

    #[test]
    fn tuple_pattern_non_tuple_errors() {
        let mut env = Environment::new();
        let pattern = BindingPattern::Tuple(vec![]);
        let result = bind_pattern(&pattern, Value::int(42), Mutability::Immutable, &mut env);
        assert!(result.is_err());
    }

    #[test]
    fn list_pattern_binds_elements() {
        let mut env = Environment::new();
        let name1 = Name::from_raw(1);
        let rest_name = Name::from_raw(2);
        let pattern = BindingPattern::List {
            elements: vec![BindingPattern::Name(name1)],
            rest: Some(rest_name),
        };
        let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
        bind_pattern(&pattern, list, Mutability::Immutable, &mut env).unwrap();
        assert_eq!(env.lookup(name1), Some(Value::int(1)));
        let rest = env.lookup(rest_name).unwrap();
        if let Value::List(items) = rest {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn list_pattern_too_short_errors() {
        let mut env = Environment::new();
        let name1 = Name::from_raw(1);
        let name2 = Name::from_raw(2);
        let pattern = BindingPattern::List {
            elements: vec![BindingPattern::Name(name1), BindingPattern::Name(name2)],
            rest: None,
        };
        let list = Value::list(vec![Value::int(1)]);
        let result = bind_pattern(&pattern, list, Mutability::Immutable, &mut env);
        assert!(result.is_err());
    }
}

mod eval_if_tests {
    use super::*;

    #[test]
    fn true_condition_returns_then_branch() {
        let cond = ExprId::new(1);
        let then_branch = ExprId::new(2);
        let else_branch = Some(ExprId::new(3));

        let mut call_count = 0;
        let result = eval_if(cond, then_branch, else_branch, |_id| {
            call_count += 1;
            if call_count == 1 {
                // Condition
                Ok(Value::Bool(true))
            } else {
                // Then branch
                Ok(Value::int(42))
            }
        });
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn false_condition_returns_else_branch() {
        let cond = ExprId::new(1);
        let then_branch = ExprId::new(2);
        let else_branch = Some(ExprId::new(3));

        let mut call_count = 0;
        let result = eval_if(cond, then_branch, else_branch, |_id| {
            call_count += 1;
            if call_count == 1 {
                // Condition
                Ok(Value::Bool(false))
            } else {
                // Else branch
                Ok(Value::int(99))
            }
        });
        assert_eq!(result.unwrap(), Value::int(99));
    }

    #[test]
    fn false_condition_no_else_returns_void() {
        let cond = ExprId::new(1);
        let then_branch = ExprId::new(2);

        let result = eval_if(cond, then_branch, None, |_| Ok(Value::Bool(false)));
        assert_eq!(result.unwrap(), Value::Void);
    }

    #[test]
    fn condition_error_propagates() {
        let cond = ExprId::new(1);
        let then_branch = ExprId::new(2);

        let result = eval_if(cond, then_branch, None, |_| {
            Err(EvalError::new("test error"))
        });
        assert!(result.is_err());
    }
}
