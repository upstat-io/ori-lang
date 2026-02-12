//! Tests for function and method call evaluation.
//!
//! Tests argument binding, parameter validation, and function call dispatch.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use rustc_hash::FxHashMap;

use crate::eval::exec::call::{bind_captures, check_arg_count, eval_function_val_call};
use crate::eval::{Environment, EvalError, FunctionValue, Value};
use crate::ir::{ExprArena, Name, SharedArena, SharedInterner};

/// Create a dummy arena for tests.
fn dummy_arena() -> SharedArena {
    SharedArena::new(ExprArena::new())
}

/// Create a test function with the given parameters.
fn test_func(params: Vec<Name>) -> FunctionValue {
    FunctionValue::new(params, FxHashMap::default(), dummy_arena())
}

// Argument Count Validation Tests

mod arg_count {
    use super::*;

    #[test]
    fn correct_count_zero() {
        let func = test_func(vec![]);
        let args: Vec<Value> = vec![];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn correct_count_one() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let func = test_func(vec![x]);
        let args = vec![Value::int(1)];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn correct_count_many() {
        let interner = SharedInterner::default();
        let func = test_func(vec![
            interner.intern("a"),
            interner.intern("b"),
            interner.intern("c"),
        ]);
        let args = vec![Value::int(1), Value::int(2), Value::int(3)];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn too_few_args() {
        let interner = SharedInterner::default();
        let func = test_func(vec![interner.intern("a"), interner.intern("b")]);
        let args = vec![Value::int(1)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }

    #[test]
    fn too_many_args() {
        let interner = SharedInterner::default();
        let func = test_func(vec![interner.intern("x")]);
        let args = vec![Value::int(1), Value::int(2)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }

    #[test]
    fn zero_params_with_args() {
        let func = test_func(vec![]);
        let args = vec![Value::int(1)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }
}

// Capture Binding Tests

mod capture_binding {
    use super::*;

    fn func_with_captures(params: Vec<Name>, captures: FxHashMap<Name, Value>) -> FunctionValue {
        FunctionValue::new(params, captures, dummy_arena())
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

// Function Val Call Tests

mod function_val_call {
    use super::*;

    #[test]
    fn success() {
        fn add_one(args: &[Value]) -> Result<Value, EvalError> {
            if let Value::Int(n) = &args[0] {
                Ok(Value::int(n.raw() + 1))
            } else {
                Err(EvalError::new("expected int"))
            }
        }

        let result = eval_function_val_call(add_one, &[Value::int(5)]);
        assert_eq!(result.unwrap(), Value::int(6));
    }

    #[test]
    fn error_propagation() {
        fn always_error(_args: &[Value]) -> Result<Value, EvalError> {
            Err(EvalError::new("always fails"))
        }

        let result = eval_function_val_call(always_error, &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .into_eval_error()
            .message
            .contains("always fails"));
    }

    #[test]
    fn multiple_args() {
        fn sum(args: &[Value]) -> Result<Value, EvalError> {
            let mut total = 0;
            for arg in args {
                if let Value::Int(n) = arg {
                    total += n.raw();
                } else {
                    return Err(EvalError::new("expected int"));
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
        fn constant(_args: &[Value]) -> Result<Value, EvalError> {
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
        fn to_string(args: &[Value]) -> Result<Value, EvalError> {
            Ok(Value::string(format!("{}", args[0])))
        }

        let result = eval_function_val_call(to_string, &[Value::int(42)]);
        assert!(matches!(result.unwrap(), Value::Str(_)));
    }

    #[test]
    fn error_message_preserved() {
        fn custom_error(_args: &[Value]) -> Result<Value, EvalError> {
            Err(EvalError::new("custom error message"))
        }

        let result = eval_function_val_call(custom_error, &[]);
        assert_eq!(
            result.unwrap_err().into_eval_error().message,
            "custom error message"
        );
    }
}
