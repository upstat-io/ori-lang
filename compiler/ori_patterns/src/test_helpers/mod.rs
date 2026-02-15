//! Test helpers for pattern unit testing.
//!
//! Provides `MockPatternExecutor` for testing patterns in isolation without
//! the full evaluator infrastructure.

#![allow(
    clippy::unwrap_used,
    reason = "test helper module — unwrap is idiomatic for test assertions"
)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "mock executor index cycling is bounded by call_results length"
)]

use rustc_hash::FxHashMap;

use ori_ir::{ExprArena, ExprId, Name, NamedExpr, SharedInterner};

use crate::{EvalContext, EvalError, EvalResult, PatternExecutor, Value};

/// Create an `EvalContext` for testing patterns.
///
/// This is a convenience wrapper around `EvalContext::new()` for test code.
/// Uses `SharedInterner` since that's what tests typically create.
pub fn make_ctx<'a>(
    interner: &'a SharedInterner,
    arena: &'a ExprArena,
    props: &'a [NamedExpr],
) -> EvalContext<'a> {
    // SharedInterner derefs to StringInterner
    EvalContext::new(interner, arena, props)
}

/// Mock executor for testing patterns in isolation.
///
/// Allows registration of:
/// - Values to return for specific `ExprId`s
/// - Variables accessible via `lookup_var`
/// - Capabilities accessible via `lookup_capability`
/// - Call results for specific function values
pub struct MockPatternExecutor {
    /// Values to return for `eval(ExprId)` calls.
    expr_values: FxHashMap<usize, Value>,
    /// Variables accessible via `lookup_var`.
    variables: FxHashMap<Name, Value>,
    /// Capabilities accessible via `lookup_capability`.
    capabilities: FxHashMap<Name, Value>,
    /// Function call results (function is matched by display string).
    call_results: Vec<Value>,
    /// Index for cycling through call results.
    call_index: usize,
}

impl MockPatternExecutor {
    /// Create a new mock executor.
    pub fn new() -> Self {
        MockPatternExecutor {
            expr_values: FxHashMap::default(),
            variables: FxHashMap::default(),
            capabilities: FxHashMap::default(),
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
    pub fn with_var(mut self, name: Name, value: Value) -> Self {
        self.variables.insert(name, value);
        self
    }

    /// Register a capability accessible via `lookup_capability`.
    pub fn with_capability(mut self, name: Name, value: Value) -> Self {
        self.capabilities.insert(name, value);
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
            .ok_or_else(|| {
                EvalError::new(format!("no mock value for ExprId({})", expr_id.index())).into()
            })
    }

    fn call(&mut self, _func: &Value, _args: Vec<Value>) -> EvalResult {
        if self.call_results.is_empty() {
            return Err(EvalError::new("no mock call results configured").into());
        }
        let result = self.call_results[self.call_index].clone();
        self.call_index = (self.call_index + 1) % self.call_results.len();
        Ok(result)
    }

    fn lookup_capability(&self, name: Name) -> Option<Value> {
        self.capabilities.get(&name).cloned()
    }

    fn call_method(&mut self, _receiver: Value, _method: Name, _args: Vec<Value>) -> EvalResult {
        Ok(Value::Void)
    }

    fn lookup_var(&self, name: Name) -> Option<Value> {
        self.variables.get(&name).cloned()
    }

    fn bind_var(&mut self, name: Name, value: Value) {
        self.variables.insert(name, value);
    }
}

#[cfg(test)]
mod tests;
