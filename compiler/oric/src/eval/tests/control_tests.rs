//! Tests for control flow evaluation (if/else, loops, pattern binding, match).

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::exec::control::{bind_pattern, eval_if, to_loop_action, LoopAction};
use crate::eval::{Environment, EvalError, Mutability, Value};
use crate::ir::{BindingPattern, ExprId, SharedInterner};

// If/Else Tests

mod if_else {
    use super::*;

    #[test]
    fn true_branch() {
        let mut call_count = 0;
        let result = eval_if(ExprId::new(0), ExprId::new(1), ExprId::INVALID, |_| {
            call_count += 1;
            if call_count == 1 {
                Ok(Value::Bool(true)) // condition
            } else {
                Ok(Value::int(42)) // then branch
            }
        });
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn false_no_else() {
        let result = eval_if(ExprId::new(0), ExprId::new(1), ExprId::INVALID, |_| {
            Ok(Value::Bool(false))
        });
        assert_eq!(result.unwrap(), Value::Void);
    }

    #[test]
    fn false_with_else() {
        let mut call_count = 0;
        let result = eval_if(ExprId::new(0), ExprId::new(1), ExprId::new(2), |id| {
            call_count += 1;
            match id.raw() {
                0 => Ok(Value::Bool(false)), // condition
                2 => Ok(Value::int(99)),     // else branch
                _ => Ok(Value::Void),
            }
        });
        assert_eq!(result.unwrap(), Value::int(99));
    }

    #[test]
    fn truthy_int_nonzero() {
        let mut call_count = 0;
        let result = eval_if(ExprId::new(0), ExprId::new(1), ExprId::INVALID, |_| {
            call_count += 1;
            if call_count == 1 {
                Ok(Value::int(1)) // truthy: nonzero
            } else {
                Ok(Value::int(42))
            }
        });
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn falsy_int_zero() {
        let result = eval_if(
            ExprId::new(0),
            ExprId::new(1),
            ExprId::INVALID,
            |_| Ok(Value::int(0)), // falsy: zero
        );
        assert_eq!(result.unwrap(), Value::Void);
    }

    #[test]
    fn condition_error_propagates() {
        let result = eval_if(ExprId::new(0), ExprId::new(1), ExprId::INVALID, |_| {
            Err(crate::eval::EvalError::new("condition error"))
        });
        assert!(result.is_err());
    }
}

// Pattern Binding Tests

mod pattern_binding {
    use super::*;

    mod name_pattern {
        use super::*;

        #[test]
        fn simple_binding() {
            let interner = SharedInterner::default();
            let x = interner.intern("x");
            let pattern = BindingPattern::Name(x);

            let mut env = Environment::new();
            bind_pattern(&pattern, Value::int(42), Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(x), Some(Value::int(42)));
        }

        #[test]
        fn mutable_binding() {
            let interner = SharedInterner::default();
            let x = interner.intern("x");
            let pattern = BindingPattern::Name(x);

            let mut env = Environment::new();
            bind_pattern(&pattern, Value::int(42), Mutability::Mutable, &mut env).unwrap();

            // Mutable binding can be reassigned
            assert!(env.assign(x, Value::int(100)).is_ok());
        }

        #[test]
        fn immutable_binding() {
            let interner = SharedInterner::default();
            let x = interner.intern("x");
            let pattern = BindingPattern::Name(x);

            let mut env = Environment::new();
            bind_pattern(&pattern, Value::int(42), Mutability::Immutable, &mut env).unwrap();

            // Immutable binding cannot be reassigned
            assert!(env.assign(x, Value::int(100)).is_err());
        }
    }

    mod wildcard_pattern {
        use super::*;

        #[test]
        fn ignores_value() {
            let mut env = Environment::new();
            let result = bind_pattern(
                &BindingPattern::Wildcard,
                Value::int(42),
                Mutability::Immutable,
                &mut env,
            );
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Value::Void);
        }
    }

    mod tuple_pattern {
        use super::*;

        #[test]
        fn simple_tuple() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let b = interner.intern("b");
            let pattern =
                BindingPattern::Tuple(vec![BindingPattern::Name(a), BindingPattern::Name(b)]);

            let mut env = Environment::new();
            let value = Value::tuple(vec![Value::int(1), Value::int(2)]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(a), Some(Value::int(1)));
            assert_eq!(env.lookup(b), Some(Value::int(2)));
        }

        #[test]
        fn nested_tuple() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let b = interner.intern("b");
            let c = interner.intern("c");
            let pattern = BindingPattern::Tuple(vec![
                BindingPattern::Name(a),
                BindingPattern::Tuple(vec![BindingPattern::Name(b), BindingPattern::Name(c)]),
            ]);

            let mut env = Environment::new();
            let value = Value::tuple(vec![
                Value::int(1),
                Value::tuple(vec![Value::int(2), Value::int(3)]),
            ]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(a), Some(Value::int(1)));
            assert_eq!(env.lookup(b), Some(Value::int(2)));
            assert_eq!(env.lookup(c), Some(Value::int(3)));
        }

        #[test]
        fn with_wildcard() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let pattern =
                BindingPattern::Tuple(vec![BindingPattern::Name(a), BindingPattern::Wildcard]);

            let mut env = Environment::new();
            let value = Value::tuple(vec![Value::int(1), Value::int(2)]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(a), Some(Value::int(1)));
        }

        #[test]
        fn length_mismatch_error() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let pattern = BindingPattern::Tuple(vec![BindingPattern::Name(a)]);

            let mut env = Environment::new();
            let value = Value::tuple(vec![Value::int(1), Value::int(2)]);
            let result = bind_pattern(&pattern, value, Mutability::Immutable, &mut env);
            assert!(result.is_err());
        }

        #[test]
        fn not_tuple_error() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let pattern = BindingPattern::Tuple(vec![BindingPattern::Name(a)]);

            let mut env = Environment::new();
            let result = bind_pattern(&pattern, Value::int(42), Mutability::Immutable, &mut env);
            assert!(result.is_err());
        }

        #[test]
        fn empty_tuple() {
            let pattern = BindingPattern::Tuple(vec![]);

            let mut env = Environment::new();
            let value = Value::tuple(vec![]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();
        }
    }

    mod list_pattern {
        use super::*;

        #[test]
        fn exact_match() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let b = interner.intern("b");
            let pattern = BindingPattern::List {
                elements: vec![BindingPattern::Name(a), BindingPattern::Name(b)],
                rest: None,
            };

            let mut env = Environment::new();
            let value = Value::list(vec![Value::int(1), Value::int(2)]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(a), Some(Value::int(1)));
            assert_eq!(env.lookup(b), Some(Value::int(2)));
        }

        #[test]
        fn with_rest() {
            let interner = SharedInterner::default();
            let head = interner.intern("head");
            let tail = interner.intern("tail");
            let pattern = BindingPattern::List {
                elements: vec![BindingPattern::Name(head)],
                rest: Some(tail),
            };

            let mut env = Environment::new();
            let value = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(head), Some(Value::int(1)));
            assert_eq!(
                env.lookup(tail),
                Some(Value::list(vec![Value::int(2), Value::int(3)]))
            );
        }

        #[test]
        fn rest_empty() {
            let interner = SharedInterner::default();
            let head = interner.intern("head");
            let tail = interner.intern("tail");
            let pattern = BindingPattern::List {
                elements: vec![BindingPattern::Name(head)],
                rest: Some(tail),
            };

            let mut env = Environment::new();
            let value = Value::list(vec![Value::int(1)]);
            bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();

            assert_eq!(env.lookup(head), Some(Value::int(1)));
            assert_eq!(env.lookup(tail), Some(Value::list(vec![])));
        }

        #[test]
        fn too_short_error() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let b = interner.intern("b");
            let pattern = BindingPattern::List {
                elements: vec![BindingPattern::Name(a), BindingPattern::Name(b)],
                rest: None,
            };

            let mut env = Environment::new();
            let value = Value::list(vec![Value::int(1)]);
            let result = bind_pattern(&pattern, value, Mutability::Immutable, &mut env);
            assert!(result.is_err());
        }

        #[test]
        fn not_list_error() {
            let interner = SharedInterner::default();
            let a = interner.intern("a");
            let pattern = BindingPattern::List {
                elements: vec![BindingPattern::Name(a)],
                rest: None,
            };

            let mut env = Environment::new();
            let result = bind_pattern(&pattern, Value::int(42), Mutability::Immutable, &mut env);
            assert!(result.is_err());
        }
    }
}

// Loop Control Tests (using typed ControlFlow enum)

mod loop_control {
    use super::*;

    #[test]
    fn continue_signal_returns_continue() {
        let err = EvalError::continue_signal();
        match to_loop_action(err) {
            LoopAction::Continue => {}
            other => panic!("expected Continue, got {other:?}"),
        }
    }

    #[test]
    fn continue_with_value_returns_continue_with() {
        let err = EvalError::continue_with(Value::int(42));
        match to_loop_action(err) {
            LoopAction::ContinueWith(v) => assert_eq!(v, Value::int(42)),
            other => panic!("expected ContinueWith, got {other:?}"),
        }
    }

    #[test]
    fn break_void_returns_break_void() {
        let err = EvalError::break_with(Value::Void);
        match to_loop_action(err) {
            LoopAction::Break(Value::Void) => {}
            other => panic!("expected Break(Void), got {other:?}"),
        }
    }

    #[test]
    fn break_with_value_returns_break_with_value() {
        let err = EvalError::break_with(Value::int(42));
        match to_loop_action(err) {
            LoopAction::Break(v) => assert_eq!(v, Value::int(42)),
            other => panic!("expected Break with value, got {other:?}"),
        }
    }

    #[test]
    fn regular_error_returns_error() {
        let err = EvalError::new("some error");
        match to_loop_action(err) {
            LoopAction::Error(e) => assert_eq!(e.message, "some error"),
            other => panic!("expected Error, got {other:?}"),
        }
    }
}

// Edge Cases

mod edge_cases {
    use super::*;

    #[test]
    fn deeply_nested_pattern() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        // ((((x))))
        let pattern =
            BindingPattern::Tuple(vec![BindingPattern::Tuple(vec![BindingPattern::Tuple(
                vec![BindingPattern::Tuple(vec![BindingPattern::Name(x)])],
            )])]);

        let value = Value::tuple(vec![Value::tuple(vec![Value::tuple(vec![Value::tuple(
            vec![Value::int(42)],
        )])])]);

        let mut env = Environment::new();
        bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();
        assert_eq!(env.lookup(x), Some(Value::int(42)));
    }

    #[test]
    fn mixed_tuple_list_pattern() {
        let interner = SharedInterner::default();
        let a = interner.intern("a");
        let b = interner.intern("b");
        let c = interner.intern("c");

        // (a, [b, c])
        let pattern = BindingPattern::Tuple(vec![
            BindingPattern::Name(a),
            BindingPattern::List {
                elements: vec![BindingPattern::Name(b), BindingPattern::Name(c)],
                rest: None,
            },
        ]);

        let value = Value::tuple(vec![
            Value::int(1),
            Value::list(vec![Value::int(2), Value::int(3)]),
        ]);

        let mut env = Environment::new();
        bind_pattern(&pattern, value, Mutability::Immutable, &mut env).unwrap();
        assert_eq!(env.lookup(a), Some(Value::int(1)));
        assert_eq!(env.lookup(b), Some(Value::int(2)));
        assert_eq!(env.lookup(c), Some(Value::int(3)));
    }
}
