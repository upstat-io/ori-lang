// Scope management for Sigil type checker
// Provides RAII-based scope guards for automatic cleanup

use crate::ast::TypeExpr;
use crate::core::Binding;
use std::collections::HashMap;

/// Local variable binding with type and mutability info.
/// This is a type alias for `Binding<TypeExpr>` from the core module.
pub type LocalBinding = Binding<TypeExpr>;

/// Extension methods for LocalBinding to provide backward compatibility
pub trait LocalBindingExt {
    /// Get the type of this binding.
    fn ty(&self) -> &TypeExpr;
    /// Check if this binding is mutable.
    fn mutable(&self) -> bool;
}

impl LocalBindingExt for LocalBinding {
    fn ty(&self) -> &TypeExpr {
        self.get()
    }
    fn mutable(&self) -> bool {
        self.is_mutable()
    }
}

/// RAII scope guard for automatic cleanup
/// When the guard is dropped, the scope is automatically restored
pub struct ScopeGuard<'a> {
    manager: &'a mut ScopeManager,
    saved_locals: HashMap<String, LocalBinding>,
    saved_return_type: Option<TypeExpr>,
}

impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        // Automatic restore on scope exit
        self.manager.locals = std::mem::take(&mut self.saved_locals);
        self.manager.current_return_type = self.saved_return_type.take();
    }
}

impl<'a> ScopeGuard<'a> {
    /// Define a local variable in the current scope
    pub fn define_local(&mut self, name: String, ty: TypeExpr, mutable: bool) {
        self.manager.define_local(name, ty, mutable);
    }

    /// Lookup the type of a local variable
    pub fn lookup_local(&self, name: &str) -> Option<&TypeExpr> {
        self.manager.lookup_local(name)
    }

    /// Lookup the full binding info for a local variable
    pub fn lookup_local_binding(&self, name: &str) -> Option<&LocalBinding> {
        self.manager.lookup_local_binding(name)
    }

    /// Check if a local variable is mutable
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.manager.is_mutable(name)
    }

    /// Get the current return type
    pub fn current_return_type(&self) -> Option<TypeExpr> {
        self.manager.current_return_type()
    }
}

/// Manages local variable scopes with RAII semantics
#[derive(Clone, Default)]
pub struct ScopeManager {
    /// Local variable bindings (in current scope) with type and mutability
    pub(crate) locals: HashMap<String, LocalBinding>,

    /// Current function's return type (for `self` calls in recurse)
    pub(crate) current_return_type: Option<TypeExpr>,
}

impl ScopeManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter a new scope and return a guard that restores state on drop
    pub fn enter_scope(&mut self) -> ScopeGuard<'_> {
        ScopeGuard {
            saved_locals: self.locals.clone(),
            saved_return_type: self.current_return_type.clone(),
            manager: self,
        }
    }

    /// Enter a scope with a specific return type
    pub fn enter_function_scope(&mut self, return_type: TypeExpr) -> ScopeGuard<'_> {
        let guard = self.enter_scope();
        guard.manager.set_return_type(return_type);
        guard
    }

    /// Define a local variable with type and mutability
    pub fn define_local(&mut self, name: String, ty: TypeExpr, mutable: bool) {
        self.locals.insert(name, Binding::new(ty, mutable));
    }

    /// Define a local variable (immutable by default)
    pub fn define_local_immutable(&mut self, name: String, ty: TypeExpr) {
        self.define_local(name, ty, false);
    }

    /// Lookup the type of a local variable
    pub fn lookup_local(&self, name: &str) -> Option<&TypeExpr> {
        self.locals.get(name).map(|b| b.get())
    }

    /// Lookup the full binding info for a local variable
    pub fn lookup_local_binding(&self, name: &str) -> Option<&LocalBinding> {
        self.locals.get(name)
    }

    /// Check if a local variable is mutable
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.locals.get(name).map(|b| b.is_mutable())
    }

    /// Set the current function's return type
    pub fn set_return_type(&mut self, ty: TypeExpr) {
        self.current_return_type = Some(ty);
    }

    /// Clear the current function's return type
    pub fn clear_return_type(&mut self) {
        self.current_return_type = None;
    }

    /// Get the current function's return type
    pub fn current_return_type(&self) -> Option<TypeExpr> {
        self.current_return_type.clone()
    }

    /// Get a snapshot of current locals (for manual save/restore when needed)
    pub fn save_locals(&self) -> HashMap<String, LocalBinding> {
        self.locals.clone()
    }

    /// Restore locals from a saved snapshot
    pub fn restore_locals(&mut self, locals: HashMap<String, LocalBinding>) {
        self.locals = locals;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_manager_basic() {
        let mut manager = ScopeManager::new();

        manager.define_local("x".to_string(), TypeExpr::Named("int".to_string()), false);
        assert!(manager.lookup_local("x").is_some());
        assert_eq!(manager.is_mutable("x"), Some(false));
    }

    #[test]
    fn test_scope_manager_mutable() {
        let mut manager = ScopeManager::new();

        manager.define_local("x".to_string(), TypeExpr::Named("int".to_string()), true);
        assert_eq!(manager.is_mutable("x"), Some(true));
    }

    #[test]
    fn test_scope_guard_raii() {
        let mut manager = ScopeManager::new();
        manager.define_local("outer".to_string(), TypeExpr::Named("int".to_string()), false);

        {
            let mut guard = manager.enter_scope();
            guard.define_local("inner".to_string(), TypeExpr::Named("str".to_string()), false);
            assert!(guard.lookup_local("outer").is_some());
            assert!(guard.lookup_local("inner").is_some());
        }
        // After guard drops, inner should be gone
        assert!(manager.lookup_local("outer").is_some());
        assert!(manager.lookup_local("inner").is_none());
    }

    #[test]
    fn test_scope_guard_return_type() {
        let mut manager = ScopeManager::new();
        manager.set_return_type(TypeExpr::Named("int".to_string()));

        {
            let guard = manager.enter_function_scope(TypeExpr::Named("str".to_string()));
            assert_eq!(
                guard.current_return_type(),
                Some(TypeExpr::Named("str".to_string()))
            );
        }
        // After guard drops, original return type should be restored
        assert_eq!(
            manager.current_return_type(),
            Some(TypeExpr::Named("int".to_string()))
        );
    }

    #[test]
    fn test_nested_scopes() {
        let mut manager = ScopeManager::new();
        manager.define_local("a".to_string(), TypeExpr::Named("int".to_string()), false);

        {
            let mut guard1 = manager.enter_scope();
            guard1.define_local("b".to_string(), TypeExpr::Named("int".to_string()), false);
            assert!(guard1.lookup_local("a").is_some());
            assert!(guard1.lookup_local("b").is_some());

            // Nested scope - need to work with manager through the guard
            // For nested scopes, we create another scope on the manager
        }

        // After all guards drop
        assert!(manager.lookup_local("a").is_some());
        assert!(manager.lookup_local("b").is_none());
    }
}
