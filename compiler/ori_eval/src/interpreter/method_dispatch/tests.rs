use super::*;

mod expect_arg_count_tests {
    use super::*;

    #[test]
    fn correct_count_returns_ok() {
        let args = vec![Value::int(1)];
        assert!(Interpreter::expect_arg_count("test", 1, &args).is_ok());
    }

    #[test]
    fn wrong_count_returns_error() {
        let args = vec![Value::int(1), Value::int(2)];
        let result = Interpreter::expect_arg_count("test", 1, &args);
        let Err(err) = result else {
            panic!("expected error for wrong arg count");
        };
        assert!(err.message.contains("test"));
    }

    #[test]
    fn zero_expected_zero_given_ok() {
        let args: Vec<Value> = vec![];
        assert!(Interpreter::expect_arg_count("test", 0, &args).is_ok());
    }

    #[test]
    fn zero_expected_one_given_error() {
        let args = vec![Value::int(1)];
        assert!(Interpreter::expect_arg_count("test", 0, &args).is_err());
    }
}

// Note: More comprehensive method dispatch tests are in the integration
// tests (tests/spec/) since they require a full interpreter setup.
// These unit tests cover the simpler helper functions.
