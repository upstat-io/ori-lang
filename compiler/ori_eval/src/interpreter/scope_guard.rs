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
mod tests {
    use super::*;
    use ori_ir::{ExprArena, SharedInterner};

    #[test]
    fn test_scoped_interpreter_drops_on_normal_exit() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        // Start with 1 scope
        assert_eq!(interp.env.depth(), 1);

        {
            let scoped = interp.scoped();
            assert_eq!(scoped.env.depth(), 2);
        }

        // Back to 1 scope after guard dropped
        assert_eq!(interp.env.depth(), 1);
    }

    #[test]
    fn test_scoped_interpreter_drops_on_panic() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        assert_eq!(interp.env.depth(), 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            let scoped = interp.scoped();
            assert_eq!(scoped.env.depth(), 2);
            panic!("test panic");
        }));

        assert!(result.is_err());
        // Scope should still be popped due to Drop
        assert_eq!(interp.env.depth(), 1);
    }

    #[test]
    fn test_scoped_interpreter_drops_on_nested_panic() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        assert_eq!(interp.env.depth(), 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            interp.with_env_scope(|scoped1| {
                assert_eq!(scoped1.env.depth(), 2);
                scoped1.with_env_scope(|scoped2| {
                    assert_eq!(scoped2.env.depth(), 3);
                    scoped2.with_env_scope(|scoped3| {
                        assert_eq!(scoped3.env.depth(), 4);
                        panic!("deep panic");
                    });
                });
            });
        }));

        assert!(result.is_err());
        // All 3 scopes should be popped due to Drop during unwinding
        assert_eq!(interp.env.depth(), 1);
    }

    #[test]
    fn test_with_env_scope_closure() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        let name = interner.intern("x");
        let result = interp.with_env_scope(|scoped| {
            scoped
                .env
                .define(name, Value::int(42), Mutability::Immutable);
            scoped.env.lookup(name)
        });

        assert_eq!(result, Some(Value::int(42)));
        // Variable should be gone after scope exit
        assert_eq!(interp.env.lookup(name), None);
    }

    #[test]
    fn test_with_env_scope_closure_panic() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        let name = interner.intern("x");
        assert_eq!(interp.env.depth(), 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            interp.with_env_scope(|scoped| {
                scoped
                    .env
                    .define(name, Value::int(42), Mutability::Immutable);
                assert_eq!(scoped.env.depth(), 2);
                panic!("closure panic");
            })
        }));

        assert!(result.is_err());
        // Scope should be popped even though closure panicked
        assert_eq!(interp.env.depth(), 1);
        // Variable should be gone
        assert_eq!(interp.env.lookup(name), None);
    }

    #[test]
    fn test_nested_scopes() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        assert_eq!(interp.env.depth(), 1);

        interp.with_env_scope(|scoped1| {
            assert_eq!(scoped1.env.depth(), 2);

            scoped1.with_env_scope(|scoped2| {
                assert_eq!(scoped2.env.depth(), 3);
            });

            assert_eq!(scoped1.env.depth(), 2);
        });

        assert_eq!(interp.env.depth(), 1);
    }

    #[test]
    fn test_with_binding() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        let name = interner.intern("x");

        let result = interp.with_binding(name, Value::int(100), Mutability::Immutable, |scoped| {
            scoped.env.lookup(name)
        });

        assert_eq!(result, Some(Value::int(100)));
        assert_eq!(interp.env.lookup(name), None);
    }

    #[test]
    fn test_with_bindings_multiple() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        let a = interner.intern("a");
        let b = interner.intern("b");
        let c = interner.intern("c");

        let bindings = vec![
            (a, Value::int(1), Mutability::Immutable),
            (b, Value::int(2), Mutability::Immutable),
            (c, Value::int(3), Mutability::Immutable),
        ];

        let result = interp.with_bindings(bindings, |scoped| {
            (
                scoped.env.lookup(a),
                scoped.env.lookup(b),
                scoped.env.lookup(c),
            )
        });

        assert_eq!(result.0, Some(Value::int(1)));
        assert_eq!(result.1, Some(Value::int(2)));
        assert_eq!(result.2, Some(Value::int(3)));

        // All should be gone after scope exit
        assert_eq!(interp.env.lookup(a), None);
        assert_eq!(interp.env.lookup(b), None);
        assert_eq!(interp.env.lookup(c), None);
    }

    #[test]
    fn test_scoped_deref_allows_method_calls() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        let name = interner.intern("test_var");

        // Create a scoped interpreter
        {
            let mut scoped = interp.scoped();

            // Can access env through Deref
            scoped
                .env
                .define(name, Value::int(42), Mutability::Immutable);

            // Can lookup through the scoped interpreter
            assert_eq!(scoped.env.lookup(name), Some(Value::int(42)));

            // Can access interner through Deref
            assert_eq!(scoped.interner.lookup(name), "test_var");
        }

        // Scope popped, variable gone
        assert_eq!(interp.env.lookup(name), None);
    }

    #[test]
    fn test_early_return_still_cleans_up() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let mut interp = Interpreter::new(&interner, &arena);

        fn helper(interp: &mut Interpreter) -> Option<i64> {
            let mut scoped = interp.scoped();
            let name = scoped.interner.intern("early");
            scoped
                .env
                .define(name, Value::int(999), Mutability::Immutable);

            // Early return - scope should still be cleaned up
            return Some(42);
        }

        assert_eq!(interp.env.depth(), 1);
        let result = helper(&mut interp);
        assert_eq!(result, Some(42));
        assert_eq!(interp.env.depth(), 1); // Scope cleaned up
    }
}
