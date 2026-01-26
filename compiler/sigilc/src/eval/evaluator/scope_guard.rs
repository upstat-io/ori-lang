//! RAII-style scope guards for Evaluator environment management.
//!
//! These methods provide safe scope management that guarantees cleanup
//! even on early returns or errors.

use crate::ir::Name;
use super::Evaluator;
use super::super::value::Value;
use sigil_patterns::EvalResult;

impl Evaluator<'_> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::EvaluatorBuilder;
    use crate::ir::{ExprArena, StringInterner};

    #[test]
    fn test_with_env_scope_basic() {
        let interner = StringInterner::new();
        let arena = ExprArena::new();
        let mut eval = EvaluatorBuilder::new(&interner, &arena).build();

        let x_name = interner.intern("x");
        eval.env.define(x_name, Value::Int(1), false);

        let result = eval.with_env_scope(|e| {
            e.env.define(x_name, Value::Int(2), false);
            // Use match to safely extract value without unwrap
            match e.env.lookup(x_name) {
                Some(v) => v,
                None => Value::Void, // Unreachable in this test
            }
        });

        assert_eq!(result, Value::Int(2));
        // After scope exits, x should be back to 1
        assert_eq!(eval.env.lookup(x_name), Some(Value::Int(1)));
    }

    #[test]
    fn test_with_binding() {
        let interner = StringInterner::new();
        let arena = ExprArena::new();
        let mut eval = EvaluatorBuilder::new(&interner, &arena).build();

        let x_name = interner.intern("x");

        let result = eval.with_binding(x_name, Value::Int(42), false, |e| {
            match e.env.lookup(x_name) {
                Some(v) => v,
                None => Value::Void,
            }
        });

        assert_eq!(result, Value::Int(42));
        // After scope exits, x should not exist
        assert!(eval.env.lookup(x_name).is_none());
    }

    #[test]
    fn test_with_match_bindings() {
        let interner = StringInterner::new();
        let arena = ExprArena::new();
        let mut eval = EvaluatorBuilder::new(&interner, &arena).build();

        let a_name = interner.intern("a");
        let b_name = interner.intern("b");

        let bindings = vec![
            (a_name, Value::Int(1)),
            (b_name, Value::Int(2)),
        ];

        let result = eval.with_match_bindings(bindings, |e| {
            let a = match e.env.lookup(a_name) {
                Some(Value::Int(n)) => n,
                _ => 0,
            };
            let b = match e.env.lookup(b_name) {
                Some(Value::Int(n)) => n,
                _ => 0,
            };
            Value::Int(a + b)
        });

        assert_eq!(result, Value::Int(3));
        // After scope exits, bindings should not exist
        assert!(eval.env.lookup(a_name).is_none());
        assert!(eval.env.lookup(b_name).is_none());
    }
}
