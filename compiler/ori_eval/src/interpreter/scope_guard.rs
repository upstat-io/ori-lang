//! RAII-style scope guards for Interpreter environment management.
//!
//! Provides true panic-safe scope management through the [`ScopedInterpreter`] guard.
//! The guard ensures `pop_scope()` is called when dropped, even during unwinding.
//!
//! # Design
//!
//! The guard holds `&mut Interpreter` and implements `Deref`/`DerefMut`, allowing
//! transparent access to all interpreter methods. This avoids the borrow conflict
//! that would occur if the guard only held `&mut Environment`.
//!
//! # Usage
//!
//! ```text
//! // Direct guard usage (most flexible)
//! {
//!     let mut scoped = interpreter.scoped();
//!     scoped.env.define(name, value, Mutability::Immutable);
//!     scoped.eval(body)?;
//! } // pop_scope called here, even on panic
//!
//! // Closure-based (convenience)
//! interpreter.with_env_scope(|scoped| {
//!     scoped.env.define(name, value, Mutability::Immutable);
//!     scoped.eval(body)
//! })
//! ```

use std::ops::{Deref, DerefMut};

use super::Interpreter;
use crate::{EvalResult, Mutability, Value};
use ori_ir::Name;

/// RAII guard that ensures environment scope cleanup on drop.
///
/// Access the interpreter through this guard - it implements `Deref` and `DerefMut`.
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
/// let mut scoped = interpreter.scoped();
/// scoped.env.define(name, value, Mutability::Immutable);
/// let result = scoped.eval(body)?;
/// // Scope automatically popped when `scoped` goes out of scope
/// ```
pub struct ScopedInterpreter<'guard, 'interp> {
    interpreter: &'guard mut Interpreter<'interp>,
}

impl Drop for ScopedInterpreter<'_, '_> {
    fn drop(&mut self) {
        self.interpreter.env.pop_scope();
    }
}

impl<'interp> Deref for ScopedInterpreter<'_, 'interp> {
    type Target = Interpreter<'interp>;

    fn deref(&self) -> &Self::Target {
        self.interpreter
    }
}

impl DerefMut for ScopedInterpreter<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.interpreter
    }
}

impl<'a> Interpreter<'a> {
    /// Create a scoped interpreter that automatically pops the environment scope on drop.
    ///
    /// This is the foundation for panic-safe scope management. The returned guard
    /// implements `Deref` and `DerefMut` to `Interpreter`, so you can use it
    /// exactly like the interpreter itself.
    ///
    /// # Panic Safety
    ///
    /// The scope will be popped even if code panics while the guard is held.
    ///
    /// # Example
    ///
    /// ```text
    /// {
    ///     let mut scoped = interpreter.scoped();
    ///     scoped.env.define(name, value, Mutability::Immutable);
    ///     scoped.eval(body)?;
    /// } // Scope popped here, even on panic
    /// ```
    pub fn scoped(&mut self) -> ScopedInterpreter<'_, 'a> {
        self.env.push_scope();
        ScopedInterpreter { interpreter: self }
    }

    /// Execute evaluation within a new environment scope.
    ///
    /// The scope is automatically popped when the closure returns,
    /// even on panic (true RAII guarantee).
    ///
    /// # Example
    ///
    /// ```text
    /// self.with_env_scope(|scoped| {
    ///     scoped.env.define(name, value, mutable);
    ///     scoped.eval(body)
    /// })
    /// ```
    pub fn with_env_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut ScopedInterpreter<'_, 'a>) -> T,
    {
        let mut scoped = self.scoped();
        f(&mut scoped)
    }

    /// Execute with pre-defined bindings in a new scope.
    ///
    /// Each binding is a tuple of (name, value, mutability).
    /// Panic-safe: scope cleanup guaranteed even on panic.
    ///
    /// # Example
    ///
    /// ```text
    /// let bindings = vec![(param_name, arg_value, Mutability::Immutable)];
    /// self.with_bindings(bindings, |scoped| scoped.eval(body))
    /// ```
    pub fn with_bindings<T, F, I>(&mut self, bindings: I, f: F) -> T
    where
        F: FnOnce(&mut ScopedInterpreter<'_, 'a>) -> T,
        I: IntoIterator<Item = (Name, Value, Mutability)>,
    {
        self.with_env_scope(|scoped| {
            for (name, value, mutability) in bindings {
                scoped.env.define(name, value, mutability);
            }
            f(scoped)
        })
    }

    /// Execute with match bindings (immutable) in a new scope.
    ///
    /// Convenience method for match arms where all bindings are immutable.
    /// Panic-safe: scope cleanup guaranteed even on panic.
    ///
    /// # Example
    ///
    /// ```text
    /// let bindings = extract_pattern_bindings(pattern, value)?;
    /// self.with_match_bindings(bindings, |scoped| scoped.eval(arm_body))
    /// ```
    pub fn with_match_bindings<T, F>(&mut self, bindings: Vec<(Name, Value)>, f: F) -> T
    where
        F: FnOnce(&mut ScopedInterpreter<'_, 'a>) -> T,
    {
        self.with_bindings(
            bindings
                .into_iter()
                .map(|(n, v)| (n, v, Mutability::Immutable)),
            f,
        )
    }

    /// Execute with a single binding in a new scope.
    ///
    /// Convenience method for simple cases like loop variables or single let bindings.
    /// Panic-safe: scope cleanup guaranteed even on panic.
    ///
    /// # Example
    ///
    /// ```text
    /// // for x in items do body
    /// self.with_binding(x_name, item_value, Mutability::Immutable, |scoped| scoped.eval(body))
    /// ```
    pub fn with_binding<T, F>(
        &mut self,
        name: Name,
        value: Value,
        mutability: Mutability,
        f: F,
    ) -> T
    where
        F: FnOnce(&mut ScopedInterpreter<'_, 'a>) -> T,
    {
        self.with_env_scope(|scoped| {
            scoped.env.define(name, value, mutability);
            f(scoped)
        })
    }

    /// Execute evaluation within a new scope, returning a Result.
    ///
    /// Convenience variant for when the body returns `EvalResult`.
    /// Panic-safe: scope cleanup guaranteed even on panic.
    pub fn with_env_scope_result<F>(&mut self, f: F) -> EvalResult
    where
        F: FnOnce(&mut ScopedInterpreter<'_, 'a>) -> EvalResult,
    {
        self.with_env_scope(f)
    }
}

#[cfg(test)]
#[expect(
    clippy::semicolon_if_nothing_returned,
    clippy::items_after_statements,
    clippy::unnecessary_wraps,
    reason = "test code: relaxed style for readability"
)]
mod tests;
