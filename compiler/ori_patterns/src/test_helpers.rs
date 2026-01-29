//! Test helpers for pattern unit testing.
//!
//! Provides `MockPatternExecutor` for testing patterns in isolation without
//! the full evaluator infrastructure.

#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

use std::collections::HashMap;

use ori_ir::ExprId;

use crate::{EvalError, EvalResult, PatternExecutor, Value};

/// Mock executor for testing patterns in isolation.
///
/// Allows registration of:
/// - Values to return for specific `ExprId`s
/// - Variables accessible via `lookup_var`
/// - Capabilities accessible via `lookup_capability`
/// - Call results for specific function values
pub struct MockPatternExecutor {
    /// Values to return for `eval(ExprId)` calls.
    expr_values: HashMap<usize, Value>,
    /// Variables accessible via `lookup_var`.
    variables: HashMap<String, Value>,
    /// Capabilities accessible via `lookup_capability`.
    capabilities: HashMap<String, Value>,
    /// Function call results (function is matched by display string).
    call_results: Vec<Value>,
    /// Index for cycling through call results.
    call_index: usize,
}

impl MockPatternExecutor {
    /// Create a new mock executor.
    pub fn new() -> Self {
        MockPatternExecutor {
            expr_values: HashMap::new(),
            variables: HashMap::new(),
            capabilities: HashMap::new(),
            call_results: Vec::new(),
            call_index: 0,
        }
    }

    /// Register a value to return when `eval(expr_id)` is called.
    pub fn with_expr(mut self, expr_id: ExprId, value: Value) -> Self {
        self.expr_values.insert(expr_id.index(), value);
        self
    }

    /// Register a variable accessible via `lookup_var`.
    pub fn with_var(mut self, name: &str, value: Value) -> Self {
        self.variables.insert(name.to_string(), value);
        self
    }

    /// Register a capability accessible via `lookup_capability`.
    pub fn with_capability(mut self, name: &str, value: Value) -> Self {
        self.capabilities.insert(name.to_string(), value);
        self
    }

    /// Register a sequence of values to return from `call()` invocations.
    ///
    /// Values are returned in order; cycles back to start when exhausted.
    pub fn with_call_results(mut self, results: Vec<Value>) -> Self {
        self.call_results = results;
        self
    }
}

impl Default for MockPatternExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternExecutor for MockPatternExecutor {
    fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        self.expr_values
            .get(&expr_id.index())
            .cloned()
            .ok_or_else(|| EvalError::new(format!("no mock value for ExprId({})", expr_id.index())))
    }

    fn call(&mut self, _func: &Value, _args: Vec<Value>) -> EvalResult {
        if self.call_results.is_empty() {
            return Err(EvalError::new("no mock call results configured"));
        }
        let result = self.call_results[self.call_index].clone();
        self.call_index = (self.call_index + 1) % self.call_results.len();
        Ok(result)
    }

    fn lookup_capability(&self, name: &str) -> Option<Value> {
        self.capabilities.get(name).cloned()
    }

    fn call_method(&mut self, _receiver: Value, _method: &str, _args: Vec<Value>) -> EvalResult {
        Ok(Value::Void)
    }

    fn lookup_var(&self, name: &str) -> Option<Value> {
        self.variables.get(name).cloned()
    }

    fn bind_var(&mut self, name: &str, value: Value) {
        self.variables.insert(name.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
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
        let exec = MockPatternExecutor::new()
            .with_var("x", Value::int(10))
            .with_var("y", Value::Bool(true));

        assert_eq!(exec.lookup_var("x"), Some(Value::int(10)));
        assert_eq!(exec.lookup_var("y"), Some(Value::Bool(true)));
        assert_eq!(exec.lookup_var("z"), None);
    }

    #[test]
    fn mock_executor_capabilities() {
        let exec = MockPatternExecutor::new().with_capability("Print", Value::Void);

        assert_eq!(exec.lookup_capability("Print"), Some(Value::Void));
        assert_eq!(exec.lookup_capability("Http"), None);
    }

    #[test]
    fn mock_executor_call_results() {
        let mut exec =
            MockPatternExecutor::new().with_call_results(vec![Value::int(1), Value::int(2)]);

        assert_eq!(exec.call(&Value::Void, vec![]).unwrap(), Value::int(1));
        assert_eq!(exec.call(&Value::Void, vec![]).unwrap(), Value::int(2));
        // Cycles back
        assert_eq!(exec.call(&Value::Void, vec![]).unwrap(), Value::int(1));
    }

    #[test]
    fn mock_executor_bind_var() {
        let mut exec = MockPatternExecutor::new();
        assert_eq!(exec.lookup_var("x"), None);

        exec.bind_var("x", Value::int(42));
        assert_eq!(exec.lookup_var("x"), Some(Value::int(42)));
    }
}
