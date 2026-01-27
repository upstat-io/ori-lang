//! Salsa-integrated evaluator for Ori.
//!
//! `Evaluator` wraps `ori_eval::Interpreter` and adds:
//! - Salsa-tracked module loading via `db.load_file()`
//! - Prelude auto-loading
//! - Import resolution
//!
//! For standalone/WASM usage without Salsa, use `ori_eval::Interpreter` directly.

mod builder;
mod module_loading;

pub use builder::EvaluatorBuilder;

use crate::db::Db;
use crate::ir::{ExprArena, Name, StringInterner};
use ori_eval::{
    Environment, EvalResult, Interpreter, SharedMutableRegistry, UserMethodRegistry, Value,
};

/// Salsa-integrated evaluator for Ori expressions.
///
/// Wraps `ori_eval::Interpreter` and adds module loading capabilities
/// through the Salsa database.
pub struct Evaluator<'a> {
    /// The underlying portable interpreter.
    interpreter: Interpreter<'a>,
    /// Database reference for Salsa-tracked file loading.
    db: &'a dyn Db,
    /// Whether the prelude has been auto-loaded.
    prelude_loaded: bool,
}

impl<'a> Evaluator<'a> {
    /// Create a new evaluator with default registries.
    ///
    /// The database is required for Salsa-tracked import resolution.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena, db: &'a dyn Db) -> Self {
        EvaluatorBuilder::new(interner, arena, db).build()
    }

    /// Create an evaluator builder for more configuration options.
    pub fn builder(
        interner: &'a StringInterner,
        arena: &'a ExprArena,
        db: &'a dyn Db,
    ) -> EvaluatorBuilder<'a> {
        EvaluatorBuilder::new(interner, arena, db)
    }

    /// Evaluate an expression.
    pub fn eval(&mut self, expr_id: crate::ir::ExprId) -> EvalResult {
        self.interpreter.eval(expr_id)
    }

    /// Get a reference to the environment.
    pub fn env(&self) -> &Environment {
        self.interpreter.env()
    }

    /// Get a mutable reference to the environment.
    pub fn env_mut(&mut self) -> &mut Environment {
        self.interpreter.env_mut()
    }

    /// Get the database reference.
    pub fn db(&self) -> &dyn Db {
        self.db
    }

    /// Get the string interner.
    pub fn interner(&self) -> &StringInterner {
        self.interpreter.interner
    }

    /// Get the expression arena.
    pub fn arena(&self) -> &ExprArena {
        self.interpreter.arena
    }

    /// Register the prelude functions.
    pub fn register_prelude(&mut self) {
        self.interpreter.register_prelude();
    }

    /// Evaluate a method call.
    pub fn eval_method_call(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        self.interpreter.eval_method_call(receiver, method, args)
    }

    /// Call a function value with the given arguments.
    pub fn eval_call_value(&mut self, func: Value, args: &[Value]) -> EvalResult {
        self.interpreter.eval_call_value(func, args)
    }

    /// Get the user method registry for registering impl block methods.
    pub fn user_method_registry(&self) -> &SharedMutableRegistry<UserMethodRegistry> {
        &self.interpreter.user_method_registry
    }

    /// Execute evaluation within a new environment scope.
    pub fn with_env_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.interpreter.env.push_scope();
        let result = f(self);
        self.interpreter.env.pop_scope();
        result
    }

    /// Execute evaluation within a new scope, returning a Result.
    pub fn with_env_scope_result<F>(&mut self, f: F) -> EvalResult
    where
        F: FnOnce(&mut Self) -> EvalResult,
    {
        self.interpreter.env.push_scope();
        let result = f(self);
        self.interpreter.env.pop_scope();
        result
    }
}

#[cfg(test)]
mod tests;
