//! Pattern-evaluation executor abstraction: [`EvalAction`] and [`PatternExecutor`].

use ori_ir::{ExprId, Name};

use crate::errors::EvalResult;
use crate::value::Value;

/// Actions that patterns can request the evaluator to perform.
#[derive(Debug)]
pub enum EvalAction {
    /// Evaluate an expression.
    Eval(ExprId),
    /// Call a function value with arguments.
    Call(Value, Vec<Value>),
}

/// Executor trait for pattern evaluation.
///
/// This trait abstracts over the evaluator, allowing patterns to request
/// expression evaluation and function calls without directly accessing the evaluator.
pub trait PatternExecutor {
    /// Evaluate an expression by its ID.
    fn eval(&mut self, expr_id: ExprId) -> EvalResult;

    /// Call a function value with the given arguments.
    fn call(&mut self, func: &Value, args: Vec<Value>) -> EvalResult;

    /// Look up a capability from the environment.
    ///
    /// Used by patterns that need to access capabilities like Print.
    fn lookup_capability(&self, name: Name) -> Option<Value>;

    /// Call a method on a value.
    ///
    /// Used by patterns to invoke capability methods.
    fn call_method(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult;

    /// Look up a variable in the current scope.
    ///
    /// Returns `None` if the variable is not defined.
    fn lookup_var(&self, name: Name) -> Option<Value>;

    /// Bind a variable in the current scope.
    ///
    /// Used by patterns to introduce scoped bindings during evaluation.
    fn bind_var(&mut self, name: Name, value: Value);
}
