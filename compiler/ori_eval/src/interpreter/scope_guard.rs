//! RAII-style scope guards for Interpreter environment management.
//!
//! These methods provide safe scope management that guarantees cleanup
//! even on early returns or errors.

use ori_ir::Name;
use crate::{Value, EvalResult};
use super::Interpreter;

impl Interpreter<'_> {
    /// Execute evaluation within a new environment scope.
    ///
    /// The scope is automatically popped when the closure returns,
    /// even on error or early return.
    ///
    /// # Example
    ///
    /// ```ignore
    /// self.with_env_scope(|eval| {
    ///     eval.env.define(name, value, mutable);
    ///     eval.eval(body)
    /// })
    /// ```
    pub fn with_env_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.env.push_scope();
        let result = f(self);
        self.env.pop_scope();
        result
    }

    /// Execute with pre-defined bindings in a new scope.
    ///
    /// Each binding is a tuple of (name, value, mutable).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bindings = vec![(param_name, arg_value, false)];
    /// self.with_bindings(bindings, |eval| eval.eval(body))
    /// ```
    pub fn with_bindings<T, F, I>(&mut self, bindings: I, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
        I: IntoIterator<Item = (Name, Value, bool)>,
    {
        self.with_env_scope(|eval| {
            for (name, value, mutable) in bindings {
                eval.env.define(name, value, mutable);
            }
            f(eval)
        })
    }

    /// Execute with match bindings (immutable) in a new scope.
    ///
    /// This is a convenience method for match arms where all bindings
    /// are immutable.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bindings = extract_pattern_bindings(pattern, value)?;
    /// self.with_match_bindings(bindings, |eval| eval.eval(arm_body))
    /// ```
    pub fn with_match_bindings<T, F>(&mut self, bindings: Vec<(Name, Value)>, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.with_bindings(bindings.into_iter().map(|(n, v)| (n, v, false)), f)
    }

    /// Execute with a single binding in a new scope.
    ///
    /// This is a convenience method for simple cases like loop variables
    /// or single let bindings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // for x in items do body
    /// self.with_binding(x_name, item_value, false, |eval| eval.eval(body))
    /// ```
    pub fn with_binding<T, F>(&mut self, name: Name, value: Value, mutable: bool, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.with_env_scope(|eval| {
            eval.env.define(name, value, mutable);
            f(eval)
        })
    }

    /// Execute evaluation within a new scope, returning a Result.
    ///
    /// This variant is useful when the body returns `EvalResult` and you
    /// want to chain with `?` after the scope guard.
    pub fn with_env_scope_result<F>(&mut self, f: F) -> EvalResult
    where
        F: FnOnce(&mut Self) -> EvalResult,
    {
        self.env.push_scope();
        let result = f(self);
        self.env.pop_scope();
        result
    }
}
