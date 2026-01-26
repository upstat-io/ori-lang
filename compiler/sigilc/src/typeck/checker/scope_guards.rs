//! RAII-style scope guards for TypeChecker context management.
//!
//! These helpers ensure context (capabilities, impl Self type) is properly
//! restored even on early returns, preventing bugs from forgotten restores.

use std::collections::HashSet;

use crate::ir::Name;
use crate::types::Type;

use super::TypeChecker;

/// Saved capability context for restoration.
pub struct SavedCapabilityContext {
    old_caps: HashSet<Name>,
    old_provided: HashSet<Name>,
}

impl<'a> TypeChecker<'a> {
    /// Enter a capability scope with the given capabilities.
    ///
    /// Returns the saved context that must be restored via `restore_capability_context`.
    /// Prefer using `with_capability_scope` for automatic restoration.
    pub(crate) fn enter_capability_scope(
        &mut self,
        new_caps: HashSet<Name>,
    ) -> SavedCapabilityContext {
        let old_caps = std::mem::replace(&mut self.scope.current_function_caps, new_caps);
        let old_provided = std::mem::take(&mut self.scope.provided_caps);
        SavedCapabilityContext {
            old_caps,
            old_provided,
        }
    }

    /// Restore a previously saved capability context.
    pub(crate) fn restore_capability_context(&mut self, saved: SavedCapabilityContext) {
        self.scope.current_function_caps = saved.old_caps;
        self.scope.provided_caps = saved.old_provided;
    }

    /// Execute a closure with a temporary capability scope.
    ///
    /// The capability context is automatically restored when the closure returns,
    /// ensuring proper cleanup even on early returns within the closure.
    pub(crate) fn with_capability_scope<R, F>(&mut self, new_caps: HashSet<Name>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let saved = self.enter_capability_scope(new_caps);
        let result = f(self);
        self.restore_capability_context(saved);
        result
    }

    /// Execute a closure with an empty capability scope (for tests).
    ///
    /// Tests don't declare capabilities, so they start with an empty context.
    pub(crate) fn with_empty_capability_scope<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.with_capability_scope(HashSet::new(), f)
    }
}

/// Saved impl context for restoration.
pub struct SavedImplContext {
    prev_self: Option<Type>,
}

impl<'a> TypeChecker<'a> {
    /// Enter an impl scope with the given Self type.
    ///
    /// Returns the saved context that must be restored via `restore_impl_context`.
    /// Prefer using `with_impl_scope` for automatic restoration.
    pub(crate) fn enter_impl_scope(&mut self, self_ty: Type) -> SavedImplContext {
        let prev_self = self.scope.current_impl_self.replace(self_ty);
        SavedImplContext { prev_self }
    }

    /// Restore a previously saved impl context.
    pub(crate) fn restore_impl_context(&mut self, saved: SavedImplContext) {
        self.scope.current_impl_self = saved.prev_self;
    }

    /// Execute a closure with a temporary impl scope.
    ///
    /// The impl context is automatically restored when the closure returns.
    pub(crate) fn with_impl_scope<R, F>(&mut self, self_ty: Type, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let saved = self.enter_impl_scope(self_ty);
        let result = f(self);
        self.restore_impl_context(saved);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ExprArena, StringInterner};
    use crate::typeck::checker::TypeCheckerBuilder;

    #[test]
    fn test_capability_scope_restores_on_return() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let mut checker = TypeCheckerBuilder::new(&arena, &interner).build();

        // Start with empty caps
        assert!(checker.scope.current_function_caps.is_empty());

        // Enter scope with some caps
        let cap_name = interner.intern("TestCap");
        let new_caps: HashSet<Name> = [cap_name].into_iter().collect();

        checker.with_capability_scope(new_caps, |c| {
            assert!(c.scope.current_function_caps.contains(&cap_name));
        });

        // After scope, caps should be restored to empty
        assert!(checker.scope.current_function_caps.is_empty());
    }

    #[test]
    fn test_capability_scope_preserves_previous_caps() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let mut checker = TypeCheckerBuilder::new(&arena, &interner).build();

        // Set up initial caps
        let outer_cap = interner.intern("OuterCap");
        checker.scope.current_function_caps.insert(outer_cap);

        // Enter nested scope
        let inner_cap = interner.intern("InnerCap");
        let new_caps: HashSet<Name> = [inner_cap].into_iter().collect();

        checker.with_capability_scope(new_caps, |c| {
            assert!(!c.scope.current_function_caps.contains(&outer_cap));
            assert!(c.scope.current_function_caps.contains(&inner_cap));
        });

        // After scope, original caps should be restored
        assert!(checker.scope.current_function_caps.contains(&outer_cap));
        assert!(!checker.scope.current_function_caps.contains(&inner_cap));
    }

    #[test]
    fn test_impl_scope_restores_on_return() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let mut checker = TypeCheckerBuilder::new(&arena, &interner).build();

        // Start with no impl self
        assert!(checker.scope.current_impl_self.is_none());

        // Enter impl scope
        checker.with_impl_scope(Type::Int, |c| {
            assert_eq!(c.scope.current_impl_self, Some(Type::Int));
        });

        // After scope, impl self should be restored to None
        assert!(checker.scope.current_impl_self.is_none());
    }

    #[test]
    fn test_nested_impl_scopes() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let mut checker = TypeCheckerBuilder::new(&arena, &interner).build();

        checker.with_impl_scope(Type::Int, |c| {
            assert_eq!(c.scope.current_impl_self, Some(Type::Int));

            c.with_impl_scope(Type::Float, |c2| {
                assert_eq!(c2.scope.current_impl_self, Some(Type::Float));
            });

            // Back to outer scope
            assert_eq!(c.scope.current_impl_self, Some(Type::Int));
        });

        assert!(checker.scope.current_impl_self.is_none());
    }
}
