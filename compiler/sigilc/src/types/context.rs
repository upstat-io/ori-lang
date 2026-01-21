// Type context for the Sigil type checker
// Facade that delegates to focused registries for single-responsibility

use crate::ast::{TypeDef, TypeExpr};
use super::builtins::register_builtins;
use super::registries::{ConfigRegistry, FunctionRegistry, TypeRegistry};
use super::scope::{LocalBinding, ScopeGuard, ScopeManager};
use std::collections::HashMap;

// Re-export FunctionSig from registries for backwards compatibility
pub use super::registries::FunctionSig;

/// Type checking context (facade over focused registries)
pub struct TypeContext {
    /// Type definitions registry
    pub(crate) types: TypeRegistry,

    /// Function signatures registry
    pub(crate) functions: FunctionRegistry,

    /// Config variables registry
    pub(crate) configs: ConfigRegistry,

    /// Scope manager for local variables
    pub(crate) scopes: ScopeManager,
}

impl Default for TypeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = TypeContext {
            types: TypeRegistry::new(),
            functions: FunctionRegistry::new(),
            configs: ConfigRegistry::new(),
            scopes: ScopeManager::new(),
        };

        // Register builtin functions using declarative module
        register_builtins(&mut ctx.functions);
        ctx
    }

    // === Type Registry Facade ===

    pub fn define_type(&mut self, name: String, def: TypeDef) {
        self.types.define(name, def);
    }

    pub fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.types.lookup(name)
    }

    // === Function Registry Facade ===

    pub fn define_function(&mut self, name: String, sig: FunctionSig) {
        self.functions.define(name, sig);
    }

    pub fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.lookup(name)
    }

    // === Config Registry Facade ===

    pub fn define_config(&mut self, name: String, ty: TypeExpr) {
        self.configs.define(name, ty);
    }

    pub fn lookup_config(&self, name: &str) -> Option<&TypeExpr> {
        self.configs.lookup(name)
    }

    // === Scope Manager Facade ===

    /// Enter a new scope and return a RAII guard
    pub fn enter_scope(&mut self) -> ScopeGuard<'_> {
        self.scopes.enter_scope()
    }

    /// Enter a function scope with a specific return type
    pub fn enter_function_scope(&mut self, return_type: TypeExpr) -> ScopeGuard<'_> {
        self.scopes.enter_function_scope(return_type)
    }

    /// Set the current return type for self() calls
    pub fn set_current_return_type(&mut self, ty: TypeExpr) {
        self.scopes.set_return_type(ty);
    }

    #[allow(dead_code)]
    pub fn clear_current_return_type(&mut self) {
        self.scopes.clear_return_type();
    }

    /// Define a local variable with type and mutability
    pub fn define_local(&mut self, name: String, ty: TypeExpr, mutable: bool) {
        self.scopes.define_local(name, ty, mutable);
    }

    /// Define a local variable (immutable by default) - for backwards compatibility
    pub fn define_local_immutable(&mut self, name: String, ty: TypeExpr) {
        self.scopes.define_local_immutable(name, ty);
    }

    /// Lookup the type of a local variable
    pub fn lookup_local(&self, name: &str) -> Option<&TypeExpr> {
        self.scopes.lookup_local(name)
    }

    /// Lookup the full binding info for a local variable
    pub fn lookup_local_binding(&self, name: &str) -> Option<&LocalBinding> {
        self.scopes.lookup_local_binding(name)
    }

    /// Check if a local variable is mutable
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.scopes.is_mutable(name)
    }

    /// Get the current function's return type (for `self` calls in recurse)
    pub fn current_return_type(&self) -> Option<TypeExpr> {
        self.scopes.current_return_type()
    }

    /// Get a snapshot of current locals (for saving/restoring state)
    pub fn save_locals(&self) -> HashMap<String, LocalBinding> {
        self.scopes.save_locals()
    }

    /// Restore locals from a saved snapshot
    pub fn restore_locals(&mut self, locals: HashMap<String, LocalBinding>) {
        self.scopes.restore_locals(locals);
    }

    /// Create a child context that inherits all state (for block scopes)
    pub fn child(&self) -> Self {
        TypeContext {
            types: self.types.clone(),
            functions: self.functions.clone(),
            configs: self.configs.clone(),
            scopes: self.scopes.clone(),
        }
    }

    /// Create a child context with additional locals added via a closure
    pub fn child_with_locals<F>(&self, f: F) -> Self
    where
        F: FnOnce(&mut HashMap<String, LocalBinding>),
    {
        let mut child = self.child();
        f(&mut child.scopes.locals);
        child
    }
}
