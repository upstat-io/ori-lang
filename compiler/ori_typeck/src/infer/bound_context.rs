//! Stack-based scope tracking for free variable collection.
//!
//! This module provides `BoundContext`, an efficient alternative to cloning
//! `HashSet<Name>` at every scope boundary. Instead of cloning, it maintains
//! a stack of scopes that can be pushed/popped in O(1) time.

use ori_ir::Name;
use std::collections::HashSet;

/// Stack-based bound variable tracking for free variable collection.
///
/// Instead of cloning the entire bound set at each scope boundary,
/// this maintains a base set plus a stack of scope-local bindings.
/// Lookup checks the stack in reverse order, then the base set.
///
/// # Performance
///
/// - `push_scope()`: O(1)
/// - `pop_scope()`: O(1)
/// - `add_binding()`: O(1) amortized
/// - `contains()`: O(scopes × `bindings_per_scope`) worst case, typically O(1)-O(n) where n is small
///
/// This is much cheaper than cloning an entire `HashSet` at every scope boundary.
pub struct BoundContext<'a> {
    /// The base set of bound variables (from outer context).
    base: &'a HashSet<Name>,
    /// Stack of scope-local bindings. Each Vec represents one scope.
    scopes: Vec<Vec<Name>>,
}

impl<'a> BoundContext<'a> {
    /// Create a new bound context with the given base set.
    pub fn new(base: &'a HashSet<Name>) -> Self {
        Self {
            base,
            scopes: Vec::new(),
        }
    }

    /// Push a new empty scope onto the stack.
    #[inline]
    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Pop the current scope from the stack.
    ///
    /// # Panics
    ///
    /// Panics if there are no scopes to pop (invariant violation).
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn pop_scope(&mut self) {
        self.scopes
            .pop()
            .expect("BoundContext::pop_scope called with no scopes");
    }

    /// Add a binding to the current scope.
    ///
    /// # Panics
    ///
    /// Panics if there is no current scope (must call `push_scope` first).
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn add_binding(&mut self, name: Name) {
        self.scopes
            .last_mut()
            .expect("BoundContext::add_binding called with no scope")
            .push(name);
    }

    /// Add multiple bindings to the current scope.
    ///
    /// # Panics
    ///
    /// Panics if there is no current scope (must call `push_scope` first).
    #[allow(clippy::expect_used)]
    pub fn add_bindings(&mut self, names: impl IntoIterator<Item = Name>) {
        let scope = self
            .scopes
            .last_mut()
            .expect("BoundContext::add_bindings called with no scope");
        scope.extend(names);
    }

    /// Check if a name is bound (in any scope or the base set).
    pub fn contains(&self, name: &Name) -> bool {
        // Check scopes in reverse order (innermost first)
        for scope in self.scopes.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        // Check base set
        self.base.contains(name)
    }

    /// Execute a closure with a new scope, automatically popping when done.
    ///
    /// This is the preferred way to manage scopes as it ensures proper cleanup.
    ///
    /// # Example
    ///
    /// ```ignore
    /// bound.with_scope(|b| {
    ///     b.add_binding(name);
    ///     collect_free_vars_inner(checker, body, b, free);
    /// });
    /// ```
    pub fn with_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.push_scope();
        let result = f(self);
        self.pop_scope();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::StringInterner;

    fn intern(interner: &mut StringInterner, s: &str) -> Name {
        interner.intern(s)
    }

    #[test]
    fn test_base_contains() {
        let mut interner = StringInterner::new();
        let x = intern(&mut interner, "x");
        let y = intern(&mut interner, "y");
        let z = intern(&mut interner, "z");

        let mut base = HashSet::new();
        base.insert(x);
        base.insert(y);

        let ctx = BoundContext::new(&base);

        assert!(ctx.contains(&x));
        assert!(ctx.contains(&y));
        assert!(!ctx.contains(&z));
    }

    #[test]
    fn test_scope_bindings() {
        let mut interner = StringInterner::new();
        let x = intern(&mut interner, "x");
        let y = intern(&mut interner, "y");

        let base = HashSet::new();
        let mut ctx = BoundContext::new(&base);

        ctx.push_scope();
        assert!(!ctx.contains(&x));

        ctx.add_binding(x);
        assert!(ctx.contains(&x));
        assert!(!ctx.contains(&y));

        ctx.pop_scope();
        assert!(!ctx.contains(&x));
    }

    #[test]
    fn test_nested_scopes() {
        let mut interner = StringInterner::new();
        let a = intern(&mut interner, "a");
        let b = intern(&mut interner, "b");
        let c = intern(&mut interner, "c");

        let base = HashSet::new();
        let mut ctx = BoundContext::new(&base);

        ctx.push_scope();
        ctx.add_binding(a);
        assert!(ctx.contains(&a));
        assert!(!ctx.contains(&b));

        ctx.push_scope();
        ctx.add_binding(b);
        assert!(ctx.contains(&a)); // Still visible from outer scope
        assert!(ctx.contains(&b));
        assert!(!ctx.contains(&c));

        ctx.pop_scope();
        assert!(ctx.contains(&a));
        assert!(!ctx.contains(&b)); // No longer in scope

        ctx.pop_scope();
        assert!(!ctx.contains(&a));
    }

    #[test]
    fn test_with_scope_raii() {
        let mut interner = StringInterner::new();
        let x = intern(&mut interner, "x");

        let base = HashSet::new();
        let mut ctx = BoundContext::new(&base);

        ctx.push_scope();

        let result = ctx.with_scope(|inner| {
            inner.add_binding(x);
            assert!(inner.contains(&x));
            42
        });

        assert_eq!(result, 42);
        assert!(!ctx.contains(&x)); // Scope was popped

        ctx.pop_scope();
    }

    #[test]
    fn test_add_bindings() {
        let mut interner = StringInterner::new();
        let a = intern(&mut interner, "a");
        let b = intern(&mut interner, "b");
        let c = intern(&mut interner, "c");

        let base = HashSet::new();
        let mut ctx = BoundContext::new(&base);

        ctx.push_scope();
        ctx.add_bindings([a, b, c]);

        assert!(ctx.contains(&a));
        assert!(ctx.contains(&b));
        assert!(ctx.contains(&c));

        ctx.pop_scope();
    }
}
