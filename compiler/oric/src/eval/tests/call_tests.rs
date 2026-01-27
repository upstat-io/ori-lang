//! Tests for function and method call evaluation.
//!
//! Tests argument binding, parameter validation, and function call dispatch.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::exec::call::{
    bind_parameters, bind_self, check_arg_count, eval_function_val_call,
};
use crate::eval::{Environment, FunctionValue, Value};
use crate::ir::{ExprArena, ExprId, Name, SharedArena, SharedInterner};
use std::collections::HashMap;

/// Create a dummy arena for tests.
fn dummy_arena() -> SharedArena {
    SharedArena::new(ExprArena::new())
}

/// Create a test function with the given parameters.
fn test_func(params: Vec<Name>, body: ExprId) -> FunctionValue {
    FunctionValue::new(params, body, HashMap::new(), dummy_arena())
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
