use super::*;

#[test]
fn mock_executor_eval() {
    let mut exec = MockPatternExecutor::new()
        .with_expr(ExprId::new(0), Value::int(42))
        .with_expr(ExprId::new(1), Value::string("hello"));

    assert_eq!(exec.eval(ExprId::new(0)).unwrap(), Value::int(42));
    assert_eq!(exec.eval(ExprId::new(1)).unwrap(), Value::string("hello"));
    assert!(exec.eval(ExprId::new(2)).is_err());
}

#[test]
fn mock_executor_variables() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");
    let y = interner.intern("y");
    let z = interner.intern("z");

    let exec = MockPatternExecutor::new()
        .with_var(x, Value::int(10))
        .with_var(y, Value::Bool(true));

    assert_eq!(exec.lookup_var(x), Some(Value::int(10)));
    assert_eq!(exec.lookup_var(y), Some(Value::Bool(true)));
    assert_eq!(exec.lookup_var(z), None);
}

#[test]
fn mock_executor_capabilities() {
    let interner = SharedInterner::default();
    let print = interner.intern("Print");
    let http = interner.intern("Http");

    let exec = MockPatternExecutor::new().with_capability(print, Value::Void);

    assert_eq!(exec.lookup_capability(print), Some(Value::Void));
    assert_eq!(exec.lookup_capability(http), None);
}

#[test]
fn mock_executor_call_results() {
    let mut exec = MockPatternExecutor::new().with_call_results(vec![Value::int(1), Value::int(2)]);

    assert_eq!(exec.call(&Value::Void, vec![]).unwrap(), Value::int(1));
    assert_eq!(exec.call(&Value::Void, vec![]).unwrap(), Value::int(2));
    // Cycles back
    assert_eq!(exec.call(&Value::Void, vec![]).unwrap(), Value::int(1));
}

#[test]
fn mock_executor_bind_var() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let mut exec = MockPatternExecutor::new();
    assert_eq!(exec.lookup_var(x), None);

    exec.bind_var(x, Value::int(42));
    assert_eq!(exec.lookup_var(x), Some(Value::int(42)));
}
