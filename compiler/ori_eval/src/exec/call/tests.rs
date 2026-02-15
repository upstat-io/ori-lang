use super::*;
use ori_ir::{Name, SharedArena};
use rustc_hash::FxHashMap;

fn make_function_value(param_count: usize) -> FunctionValue {
    use ori_ir::ExprArena;
    let params: Vec<Name> = (0..param_count).map(|i| Name::from_raw(i as u32)).collect();
    let captures = FxHashMap::default();
    let arena = SharedArena::new(ExprArena::default());
    FunctionValue::new(params, captures, arena)
}

mod check_arg_count_tests {
    use super::*;

    #[test]
    fn correct_count_returns_ok() {
        let func = make_function_value(2);
        let args = vec![Value::int(1), Value::int(2)];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn too_few_args_returns_error() {
        let func = make_function_value(2);
        let args = vec![Value::int(1)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }

    #[test]
    fn too_many_args_returns_error() {
        let func = make_function_value(1);
        let args = vec![Value::int(1), Value::int(2)];
        let result = check_arg_count(&func, &args);
        assert!(result.is_err());
    }

    #[test]
    fn zero_params_zero_args_ok() {
        let func = make_function_value(0);
        let args: Vec<Value> = vec![];
        assert!(check_arg_count(&func, &args).is_ok());
    }
}

mod bind_captures_tests {
    use super::*;
    use ori_ir::ExprArena;
    use rustc_hash::FxHashMap;

    #[test]
    fn binds_captured_variables() {
        let params = vec![Name::from_raw(0)];
        let mut captures = FxHashMap::default();
        let capture_name = Name::from_raw(10);
        captures.insert(capture_name, Value::int(100));
        let arena = SharedArena::new(ExprArena::default());
        let func = FunctionValue::new(params, captures, arena);

        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);
        assert_eq!(env.lookup(capture_name), Some(Value::int(100)));
    }

    #[test]
    fn binds_multiple_captures() {
        let params = vec![];
        let mut captures = FxHashMap::default();
        let name1 = Name::from_raw(10);
        let name2 = Name::from_raw(11);
        captures.insert(name1, Value::int(1));
        captures.insert(name2, Value::int(2));
        let arena = SharedArena::new(ExprArena::default());
        let func = FunctionValue::new(params, captures, arena);

        let mut env = Environment::new();
        env.push_scope();
        bind_captures(&mut env, &func);
        assert_eq!(env.lookup(name1), Some(Value::int(1)));
        assert_eq!(env.lookup(name2), Some(Value::int(2)));
    }
}

mod eval_function_val_call_tests {
    use super::*;

    #[test]
    fn successful_call_returns_value() {
        fn add_one(args: &[Value]) -> Result<Value, EvalError> {
            if let Value::Int(n) = &args[0] {
                Ok(Value::int(n.raw() + 1))
            } else {
                Err(EvalError::new("expected int"))
            }
        }
        let args = vec![Value::int(41)];
        let result = eval_function_val_call(add_one, &args);
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn error_is_converted_to_eval_error() {
        fn fail(_args: &[Value]) -> Result<Value, EvalError> {
            Err(EvalError::new("intentional error"))
        }
        let args: Vec<Value> = vec![];
        let result = eval_function_val_call(fail, &args);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().into_eval_error().message,
            "intentional error"
        );
    }
}
