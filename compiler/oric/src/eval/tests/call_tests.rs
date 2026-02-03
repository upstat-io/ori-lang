//! Tests for function and method call evaluation.
//!
//! Tests argument binding, parameter validation, and function call dispatch.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#![allow(clippy::cast_lossless)]

use crate::eval::exec::call::{
    bind_captures, bind_parameters, bind_self, check_arg_count, eval_function_val_call,
    extract_named_args,
};
use crate::eval::{Environment, FunctionValue, Value};
use crate::ir::{
    CallArg, CallArgRange, Expr, ExprArena, ExprId, ExprKind, Name, SharedArena, SharedInterner,
    Span,
};
use rustc_hash::FxHashMap;

/// Create a dummy arena for tests.
fn dummy_arena() -> SharedArena {
    SharedArena::new(ExprArena::new())
}

/// Create a test function with the given parameters.
fn test_func(params: Vec<Name>, body: ExprId) -> FunctionValue {
    FunctionValue::new(params, body, FxHashMap::default(), dummy_arena())
}

// Argument Count Validation Tests

mod arg_count {
    use super::*;

    #[test]
    fn correct_count_zero() {
        let func = test_func(vec![], ExprId::new(0));
        let args: Vec<Value> = vec![];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn correct_count_one() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let func = test_func(vec![x], ExprId::new(0));
        let args = vec![Value::int(1)];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn correct_count_many() {
        let interner = SharedInterner::default();
        let func = test_func(
            vec![
                interner.intern("a"),
                interner.intern("b"),
                interner.intern("c"),
            ],
            ExprId::new(0),
        );
        let args = vec![Value::int(1), Value::int(2), Value::int(3)];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn too_few_args() {
        let interner = SharedInterner::default();
        let func = test_func(
            vec![interner.intern("a"), interner.intern("b")],
            ExprId::new(0),
        );
        let args = vec![Value::int(1)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }

    #[test]
    fn too_many_args() {
        let interner = SharedInterner::default();
        let func = test_func(vec![interner.intern("x")], ExprId::new(0));
        let args = vec![Value::int(1), Value::int(2)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }

    #[test]
    fn zero_params_with_args() {
        let func = test_func(vec![], ExprId::new(0));
        let args = vec![Value::int(1)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }
}

// Parameter Binding Tests

mod parameter_binding {
    use super::*;

    #[test]
    fn single_param() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let func = test_func(vec![x], ExprId::new(0));
        let args = vec![Value::int(42)];

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        assert_eq!(env.lookup(x), Some(Value::int(42)));
    }

    #[test]
    fn multiple_params() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");
        let z = interner.intern("z");
        let func = test_func(vec![x, y, z], ExprId::new(0));
        let args = vec![Value::int(1), Value::int(2), Value::int(3)];

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        assert_eq!(env.lookup(x), Some(Value::int(1)));
        assert_eq!(env.lookup(y), Some(Value::int(2)));
        assert_eq!(env.lookup(z), Some(Value::int(3)));
    }

    #[test]
    fn different_value_types() {
        let interner = SharedInterner::default();
        let i = interner.intern("i");
        let s = interner.intern("s");
        let b = interner.intern("b");
        let func = test_func(vec![i, s, b], ExprId::new(0));
        let args = vec![Value::int(42), Value::string("hello"), Value::Bool(true)];

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        assert_eq!(env.lookup(i), Some(Value::int(42)));
        assert_eq!(env.lookup(s), Some(Value::string("hello")));
        assert_eq!(env.lookup(b), Some(Value::Bool(true)));
    }

    #[test]
    fn params_are_immutable() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let func = test_func(vec![x], ExprId::new(0));
        let args = vec![Value::int(42)];

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        // Parameters are bound as immutable
        assert!(env.assign(x, Value::int(100)).is_err());
    }
}

// Capture Binding Tests

mod capture_binding {
    use super::*;

    fn func_with_captures(params: Vec<Name>, captures: FxHashMap<Name, Value>) -> FunctionValue {
        FunctionValue::new(params, ExprId::new(0), captures, dummy_arena())
    }

    #[test]
    fn single_capture() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let mut captures = FxHashMap::default();
        captures.insert(x, Value::int(42));

        let func = func_with_captures(vec![], captures);
        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);

        assert_eq!(env.lookup(x), Some(Value::int(42)));
    }

    #[test]
    fn multiple_captures() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");
        let z = interner.intern("z");

        let mut captures = FxHashMap::default();
        captures.insert(x, Value::int(1));
        captures.insert(y, Value::string("hello"));
        captures.insert(z, Value::Bool(true));

        let func = func_with_captures(vec![], captures);
        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);

        assert_eq!(env.lookup(x), Some(Value::int(1)));
        assert_eq!(env.lookup(y), Some(Value::string("hello")));
        assert_eq!(env.lookup(z), Some(Value::Bool(true)));
    }

    #[test]
    fn no_captures() {
        let func = func_with_captures(vec![], FxHashMap::default());
        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);
        // Should not fail with empty captures
    }

    #[test]
    fn captures_are_immutable() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let mut captures = FxHashMap::default();
        captures.insert(x, Value::int(42));

        let func = func_with_captures(vec![], captures);
        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);

        // Captures are bound as immutable
        assert!(env.assign(x, Value::int(100)).is_err());
    }

    #[test]
    fn complex_captured_values() {
        let interner = SharedInterner::default();
        let list_name = interner.intern("list");
        let tuple_name = interner.intern("tuple");

        let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
        let tuple = Value::tuple(vec![Value::string("a"), Value::Bool(false)]);

        let mut captures = FxHashMap::default();
        captures.insert(list_name, list.clone());
        captures.insert(tuple_name, tuple.clone());

        let func = func_with_captures(vec![], captures);
        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);

        assert_eq!(env.lookup(list_name), Some(list));
        assert_eq!(env.lookup(tuple_name), Some(tuple));
    }
}

// Self Binding Tests

mod self_binding {
    use super::*;

    #[test]
    fn binds_self_name() {
        let interner = SharedInterner::default();
        let name = interner.intern("test");
        let func = test_func(vec![name], ExprId::new(0));
        let func_val = Value::Function(func);

        let mut env = Environment::new();
        env.push_scope();
        bind_self(&mut env, func_val.clone(), &interner);

        let self_name = interner.intern("self");
        assert!(env.lookup(self_name).is_some());
    }

    #[test]
    fn self_is_the_function() {
        let interner = SharedInterner::default();
        let name = interner.intern("test");
        let func = test_func(vec![name], ExprId::new(0));
        let func_val = Value::Function(func);

        let mut env = Environment::new();
        env.push_scope();
        bind_self(&mut env, func_val.clone(), &interner);

        let self_name = interner.intern("self");
        let bound = env.lookup(self_name).unwrap();

        // Should be a function
        assert!(matches!(bound, Value::Function(_)));
    }
}

// Function Val Call Tests

mod function_val_call {
    use super::*;

    #[test]
    fn success() {
        fn add_one(args: &[Value]) -> Result<Value, String> {
            if let Value::Int(n) = &args[0] {
                Ok(Value::int(n.raw() + 1))
            } else {
                Err("expected int".to_string())
            }
        }

        let result = eval_function_val_call(add_one, &[Value::int(5)]);
        assert_eq!(result.unwrap(), Value::int(6));
    }

    #[test]
    fn error_propagation() {
        fn always_error(_args: &[Value]) -> Result<Value, String> {
            Err("always fails".to_string())
        }

        let result = eval_function_val_call(always_error, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("always fails"));
    }

    #[test]
    fn multiple_args() {
        fn sum(args: &[Value]) -> Result<Value, String> {
            let mut total = 0;
            for arg in args {
                if let Value::Int(n) = arg {
                    total += n.raw();
                } else {
                    return Err("expected int".to_string());
                }
            }
            Ok(Value::int(total))
        }

        let result = eval_function_val_call(sum, &[Value::int(1), Value::int(2), Value::int(3)]);
        assert_eq!(result.unwrap(), Value::int(6));
    }

    #[test]
    fn no_args() {
        #[expect(
            clippy::unnecessary_wraps,
            reason = "function signature required by eval_function_val_call"
        )]
        fn constant(_args: &[Value]) -> Result<Value, String> {
            Ok(Value::int(42))
        }

        let result = eval_function_val_call(constant, &[]);
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn returns_different_types() {
        #[expect(
            clippy::unnecessary_wraps,
            reason = "function signature required by eval_function_val_call"
        )]
        fn to_string(args: &[Value]) -> Result<Value, String> {
            Ok(Value::string(format!("{}", args[0])))
        }

        let result = eval_function_val_call(to_string, &[Value::int(42)]);
        assert!(matches!(result.unwrap(), Value::Str(_)));
    }

    #[test]
    fn error_message_preserved() {
        fn custom_error(_args: &[Value]) -> Result<Value, String> {
            Err("custom error message".to_string())
        }

        let result = eval_function_val_call(custom_error, &[]);
        assert_eq!(result.unwrap_err().message, "custom error message");
    }
}

// Edge Cases

mod edge_cases {
    use super::*;

    #[test]
    #[expect(clippy::cast_possible_wrap, reason = "i < 100 so cast is safe")]
    fn many_parameters() {
        let interner = SharedInterner::default();
        let params: Vec<_> = (0..100)
            .map(|i| interner.intern(&format!("p{i}")))
            .collect();
        let func = test_func(params.clone(), ExprId::new(0));
        let args: Vec<_> = (0..100).map(Value::int).collect();

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        // Verify all parameters are bound
        for (i, param) in params.iter().enumerate() {
            assert_eq!(env.lookup(*param), Some(Value::int(i as i64)));
        }
    }

    #[test]
    fn empty_function() {
        let func = test_func(vec![], ExprId::new(0));
        let args: Vec<Value> = vec![];

        assert!(check_arg_count(&func, &args).is_ok());

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);
        // Should not fail with empty params/args
    }

    #[test]
    fn complex_value_args() {
        let interner = SharedInterner::default();
        let l = interner.intern("l");
        let t = interner.intern("t");
        let func = test_func(vec![l, t], ExprId::new(0));

        let list = Value::list(vec![Value::int(1), Value::int(2)]);
        let tuple = Value::tuple(vec![Value::string("a"), Value::Bool(true)]);
        let args = vec![list.clone(), tuple.clone()];

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        assert_eq!(env.lookup(l), Some(list));
        assert_eq!(env.lookup(t), Some(tuple));
    }
}

// Extract Named Args Tests

mod extract_named_args_tests {
    use super::*;

    #[test]
    fn single_arg() {
        let mut arena = ExprArena::new();

        // Create a simple expression for the argument value
        let expr = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::default()));

        // Create call args with the expression
        let call_args = vec![CallArg {
            name: None,
            value: expr,
            span: Span::default(),
        }];
        let range = arena.alloc_call_args(call_args);

        // Mock evaluation function that returns the int value
        let eval_fn = |expr_id: ExprId| -> Result<Value, crate::eval::EvalError> {
            // In real usage, this would evaluate the expression
            // For testing, we just return a value based on the expression ID
            Ok(Value::int(expr_id.raw() as i64))
        };

        let result = extract_named_args(range, &arena, eval_fn).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn multiple_args() {
        let mut arena = ExprArena::new();

        // Create expressions for each argument
        let expr1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::default()));
        let expr2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::default()));
        let expr3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::default()));

        let call_args = vec![
            CallArg {
                name: None,
                value: expr1,
                span: Span::default(),
            },
            CallArg {
                name: None,
                value: expr2,
                span: Span::default(),
            },
            CallArg {
                name: None,
                value: expr3,
                span: Span::default(),
            },
        ];
        let range = arena.alloc_call_args(call_args);

        let mut counter = 0;
        let eval_fn = |_: ExprId| -> Result<Value, crate::eval::EvalError> {
            counter += 1;
            Ok(Value::int(counter))
        };

        let result = extract_named_args(range, &arena, eval_fn).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], Value::int(1));
        assert_eq!(result[1], Value::int(2));
        assert_eq!(result[2], Value::int(3));
    }

    #[test]
    fn empty_args() {
        let arena = ExprArena::new();
        let range = CallArgRange::EMPTY;

        let eval_fn = |_: ExprId| -> Result<Value, crate::eval::EvalError> {
            panic!("should not be called for empty args")
        };

        let result = extract_named_args(range, &arena, eval_fn).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn error_propagation() {
        let mut arena = ExprArena::new();
        let expr = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::default()));

        let call_args = vec![CallArg {
            name: None,
            value: expr,
            span: Span::default(),
        }];
        let range = arena.alloc_call_args(call_args);

        let eval_fn = |_: ExprId| -> Result<Value, crate::eval::EvalError> {
            Err(crate::eval::EvalError::new("evaluation failed"))
        };

        let result = extract_named_args(range, &arena, eval_fn);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("evaluation failed"));
    }

    #[test]
    fn evaluation_order() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut arena = ExprArena::new();
        let expr1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::default()));
        let expr2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::default()));

        let call_args = vec![
            CallArg {
                name: None,
                value: expr1,
                span: Span::default(),
            },
            CallArg {
                name: None,
                value: expr2,
                span: Span::default(),
            },
        ];
        let range = arena.alloc_call_args(call_args);

        // Track evaluation order
        let order = Rc::new(RefCell::new(Vec::new()));
        let order_clone = Rc::clone(&order);

        let eval_fn = move |expr_id: ExprId| -> Result<Value, crate::eval::EvalError> {
            order_clone.borrow_mut().push(expr_id);
            Ok(Value::int(expr_id.raw() as i64))
        };

        let _ = extract_named_args(range, &arena, eval_fn).unwrap();

        // Verify args are evaluated in order
        let evaluated = order.borrow();
        assert_eq!(evaluated.len(), 2);
        assert_eq!(evaluated[0], expr1);
        assert_eq!(evaluated[1], expr2);
    }
}
