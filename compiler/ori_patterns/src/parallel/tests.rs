use super::*;

mod execute_task_tests {
    use super::*;

    #[test]
    fn function_val_success_wraps_in_ok() {
        let task = Value::FunctionVal(|_| Ok(Value::Int(42.into())), "test_fn");
        let result = execute_task(task);
        assert!(matches!(result, Value::Ok(_)));
        if let Value::Ok(inner) = result {
            assert!(matches!(*inner, Value::Int(n) if n.raw() == 42));
        }
    }

    #[test]
    fn function_val_error_wraps_in_err() {
        let task = Value::FunctionVal(|_| Err(EvalError::new("test error")), "test_fn");
        let result = execute_task(task);
        assert!(matches!(result, Value::Err(_)));
    }

    #[test]
    fn ok_passthrough() {
        let task = Value::ok(Value::Int(42.into()));
        let result = execute_task(task.clone());
        assert!(matches!(result, Value::Ok(_)));
    }

    #[test]
    fn err_passthrough() {
        let task = Value::err(Value::string("error"));
        let result = execute_task(task.clone());
        assert!(matches!(result, Value::Err(_)));
    }

    #[test]
    fn plain_value_wraps_in_ok() {
        let task = Value::Int(100.into());
        let result = execute_task(task);
        assert!(matches!(result, Value::Ok(_)));
        if let Value::Ok(inner) = result {
            assert!(matches!(*inner, Value::Int(n) if n.raw() == 100));
        }
    }
}

mod wrap_in_result_tests {
    use super::*;

    #[test]
    fn ok_passthrough() {
        let value = Value::ok(Value::Int(42.into()));
        let result = wrap_in_result(value);
        assert!(matches!(result, Value::Ok(_)));
    }

    #[test]
    fn err_passthrough() {
        let value = Value::err(Value::string("error"));
        let result = wrap_in_result(value);
        assert!(matches!(result, Value::Err(_)));
    }

    #[test]
    fn error_converts_to_err() {
        let value = Value::error("some error");
        let result = wrap_in_result(value);
        assert!(matches!(result, Value::Err(_)));
    }

    #[test]
    fn plain_value_wraps_in_ok() {
        let value = Value::Bool(true);
        let result = wrap_in_result(value);
        assert!(matches!(result, Value::Ok(_)));
        if let Value::Ok(inner) = result {
            assert!(matches!(*inner, Value::Bool(true)));
        }
    }
}
