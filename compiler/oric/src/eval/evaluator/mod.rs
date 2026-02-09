//! Salsa-integrated evaluator for Ori.
//!
//! `Evaluator` wraps `ori_eval::Interpreter` and adds:
//! - Salsa-tracked module loading via `db.load_file()`
//! - Prelude auto-loading
//! - Import resolution
//!
//! For standalone/WASM usage without Salsa, use `ori_eval::Interpreter` directly.
//!
//! # Panic-Safe Scope Management
//!
//! The [`ScopedEvaluator`] guard provides true RAII-style scope management.
//! When the guard is dropped (including during panic unwinding), the environment
//! scope is automatically popped. See [`Evaluator::scoped`] for usage.

mod builder;
mod module_loading;

pub use builder::EvaluatorBuilder;

use std::ops::{Deref, DerefMut};

use crate::db::Db;
use crate::ir::{ExprArena, Name, StringInterner};
use ori_eval::{
    Environment, EvalResult, Interpreter, SharedMutableRegistry, UserMethodRegistry, Value,
};

/// RAII guard that ensures environment scope cleanup on drop.
///
/// Access the evaluator through this guard - it implements `Deref` and `DerefMut`.
/// When the guard is dropped (including on panic), `pop_scope()` is called automatically.
///
/// # Panic Safety
///
/// This guard provides true panic safety. If code panics while the guard is held,
/// the `Drop` implementation will still run during stack unwinding, ensuring the
/// environment scope is properly cleaned up.
///
/// # Example
///
/// ```text
/// {
///     let mut scoped = evaluator.scoped();
///     scoped.env_mut().define(name, value, Mutability::Immutable);
///     scoped.eval(body)?;
/// } // Scope automatically popped when `scoped` goes out of scope
/// ```
pub struct ScopedEvaluator<'guard, 'eval> {
    evaluator: &'guard mut Evaluator<'eval>,
}

impl Drop for ScopedEvaluator<'_, '_> {
    fn drop(&mut self) {
        self.evaluator.interpreter.env.pop_scope();
    }
}

impl<'eval> Deref for ScopedEvaluator<'_, 'eval> {
    type Target = Evaluator<'eval>;

    fn deref(&self) -> &Self::Target {
        self.evaluator
    }
}

impl DerefMut for ScopedEvaluator<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.evaluator
    }
}

/// Salsa-integrated evaluator for Ori expressions.
///
/// Wraps `ori_eval::Interpreter` and adds module loading capabilities
/// through the Salsa database.
pub struct Evaluator<'a> {
    /// The underlying portable interpreter.
    pub(crate) interpreter: Interpreter<'a>,
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

    /// Evaluate a canonical expression.
    ///
    /// Dispatches via the canonical IR path (`eval_can`). Requires that
    /// the interpreter was built with a `SharedCanonResult`.
    pub fn eval_can(&mut self, can_id: ori_ir::canon::CanId) -> EvalResult {
        self.interpreter.eval_can(can_id)
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
    pub fn eval_call_value(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        self.interpreter.eval_call_value(func, args)
    }

    /// Get the user method registry for registering impl block methods.
    pub fn user_method_registry(&self) -> &SharedMutableRegistry<UserMethodRegistry> {
        &self.interpreter.user_method_registry
    }

    /// Enable performance counters for `--profile` mode.
    ///
    /// Must be called before evaluation begins. When enabled, expression,
    /// function call, method call, and pattern match counts are tracked.
    pub fn enable_counters(&mut self) {
        self.interpreter.enable_counters();
    }

    /// Get the counter report string, if counters are enabled.
    pub fn counters_report(&self) -> Option<String> {
        self.interpreter.counters_report()
    }

    /// Create a scoped evaluator that automatically pops the environment scope on drop.
    ///
    /// This is the foundation for panic-safe scope management. The returned guard
    /// implements `Deref` and `DerefMut` to `Evaluator`, so you can use it
    /// exactly like the evaluator itself.
    ///
    /// # Panic Safety
    ///
    /// The scope will be popped even if code panics while the guard is held.
    ///
    /// # Example
    ///
    /// ```text
    /// {
    ///     let mut scoped = evaluator.scoped();
    ///     scoped.env_mut().define(name, value, Mutability::Immutable);
    ///     scoped.eval(body)?;
    /// } // Scope popped here, even on panic
    /// ```
    pub fn scoped(&mut self) -> ScopedEvaluator<'_, 'a> {
        self.interpreter.env.push_scope();
        ScopedEvaluator { evaluator: self }
    }

    /// Execute evaluation within a new environment scope.
    ///
    /// The scope is automatically popped when the closure returns,
    /// even on panic (true RAII guarantee).
    ///
    /// # Example
    ///
    /// ```text
    /// evaluator.with_env_scope(|scoped| {
    ///     scoped.env_mut().define(name, value, Mutability::Immutable);
    ///     scoped.eval(body)
    /// })
    /// ```
    pub fn with_env_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut ScopedEvaluator<'_, 'a>) -> T,
    {
        let mut scoped = self.scoped();
        f(&mut scoped)
    }

    /// Execute evaluation within a new scope, returning a Result.
    ///
    /// Convenience variant for when the body returns `EvalResult`.
    /// Panic-safe: scope cleanup guaranteed even on panic.
    pub fn with_env_scope_result<F>(&mut self, f: F) -> EvalResult
    where
        F: FnOnce(&mut ScopedEvaluator<'_, 'a>) -> EvalResult,
    {
        self.with_env_scope(f)
    }
}

#[cfg(test)]
mod tests;
