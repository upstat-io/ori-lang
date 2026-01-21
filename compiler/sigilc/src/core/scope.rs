// Generic scope management for the Sigil compiler
//
// Provides a unified scope abstraction used across type checking and evaluation.
// Supports RAII-based scope guards for automatic cleanup.

use super::binding::Binding;
use std::collections::HashMap;
use std::fmt::Debug;

/// A scope that maps names to bindings with optional metadata.
///
/// This is a generic scope that can be used for:
/// - Type checking: `Scope<TypeExpr>` with return type as metadata
/// - Evaluation: `Scope<Value>` with optional function context
///
/// # Type Parameters
/// - `T`: The type of values stored in bindings
/// - `M`: Optional metadata type (defaults to unit)
#[derive(Clone)]
pub struct Scope<T, M = ()>
where
    T: Clone,
    M: Clone,
{
    /// Map from variable names to their bindings
    bindings: HashMap<String, Binding<T>>,
    /// Optional scope-level metadata (e.g., return type for functions)
    metadata: Option<M>,
}

impl<T: Clone, M: Clone> Default for Scope<T, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, M: Clone> Scope<T, M> {
    /// Create a new empty scope.
    pub fn new() -> Self {
        Scope {
            bindings: HashMap::new(),
            metadata: None,
        }
    }

    /// Create a new scope with the given metadata.
    pub fn with_metadata(metadata: M) -> Self {
        Scope {
            bindings: HashMap::new(),
            metadata: Some(metadata),
        }
    }

    /// Define a new binding in this scope.
    pub fn define(&mut self, name: String, value: T, mutable: bool) {
        self.bindings.insert(name, Binding::new(value, mutable));
    }

    /// Define a new immutable binding.
    pub fn define_immutable(&mut self, name: String, value: T) {
        self.define(name, value, false);
    }

    /// Define a new mutable binding.
    pub fn define_mutable(&mut self, name: String, value: T) {
        self.define(name, value, true);
    }

    /// Look up a binding by name.
    pub fn lookup(&self, name: &str) -> Option<&Binding<T>> {
        self.bindings.get(name)
    }

    /// Look up a binding mutably by name.
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Binding<T>> {
        self.bindings.get_mut(name)
    }

    /// Look up just the value by name.
    pub fn lookup_value(&self, name: &str) -> Option<&T> {
        self.bindings.get(name).map(|b| b.get())
    }

    /// Check if a name is defined in this scope.
    pub fn contains(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Check if a binding is mutable.
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.bindings.get(name).map(|b| b.is_mutable())
    }

    /// Get the scope metadata.
    pub fn metadata(&self) -> Option<&M> {
        self.metadata.as_ref()
    }

    /// Set the scope metadata.
    pub fn set_metadata(&mut self, metadata: M) {
        self.metadata = Some(metadata);
    }

    /// Clear the scope metadata.
    pub fn clear_metadata(&mut self) {
        self.metadata = None;
    }

    /// Get an iterator over all bindings.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Binding<T>)> {
        self.bindings.iter()
    }

    /// Get the number of bindings in this scope.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Check if this scope is empty.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Extract just the values (discarding mutability info).
    /// Useful for capturing closure environments.
    pub fn values(&self) -> HashMap<String, T> {
        self.bindings
            .iter()
            .map(|(k, b)| (k.clone(), b.get().clone()))
            .collect()
    }

    /// Create a scope from a map of values (all immutable).
    pub fn from_values(values: HashMap<String, T>) -> Self {
        let bindings = values
            .into_iter()
            .map(|(k, v)| (k, Binding::immutable(v)))
            .collect();
        Scope {
            bindings,
            metadata: None,
        }
    }
}

impl<T: Clone + Debug, M: Clone + Debug> Debug for Scope<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope")
            .field("bindings", &self.bindings)
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// RAII scope guard that restores the previous scope on drop.
///
/// This is used to implement nested scopes with automatic cleanup.
/// When the guard is dropped, the scope is restored to its previous state.
pub struct ScopeGuard<'a, T, M = ()>
where
    T: Clone,
    M: Clone,
{
    manager: &'a mut ScopeStack<T, M>,
}

impl<T: Clone, M: Clone> Drop for ScopeGuard<'_, T, M> {
    fn drop(&mut self) {
        self.manager.pop_scope();
    }
}

impl<'a, T: Clone, M: Clone> ScopeGuard<'a, T, M> {
    /// Define a binding in the current scope.
    pub fn define(&mut self, name: String, value: T, mutable: bool) {
        self.manager.define(name, value, mutable);
    }

    /// Define an immutable binding in the current scope.
    pub fn define_immutable(&mut self, name: String, value: T) {
        self.manager.define_immutable(name, value);
    }

    /// Define a mutable binding in the current scope.
    pub fn define_mutable(&mut self, name: String, value: T) {
        self.manager.define_mutable(name, value);
    }

    /// Look up a binding in the scope stack.
    pub fn lookup(&self, name: &str) -> Option<&Binding<T>> {
        self.manager.lookup(name)
    }

    /// Look up just the value.
    pub fn lookup_value(&self, name: &str) -> Option<&T> {
        self.manager.lookup_value(name)
    }

    /// Check if a binding is mutable.
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.manager.is_mutable(name)
    }

    /// Get the current scope's metadata.
    pub fn metadata(&self) -> Option<&M> {
        self.manager.current_metadata()
    }

    /// Set the current scope's metadata.
    pub fn set_metadata(&mut self, metadata: M) {
        self.manager.set_current_metadata(metadata);
    }
}

/// A stack of scopes for implementing lexical scoping.
///
/// This manages a stack of scopes where lookups traverse from innermost
/// to outermost scope. Each scope can have its own metadata.
pub struct ScopeStack<T, M = ()>
where
    T: Clone,
    M: Clone,
{
    scopes: Vec<Scope<T, M>>,
}

impl<T: Clone, M: Clone> Default for ScopeStack<T, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, M: Clone> ScopeStack<T, M> {
    /// Create a new scope stack with a single empty scope.
    pub fn new() -> Self {
        ScopeStack {
            scopes: vec![Scope::new()],
        }
    }

    /// Push a new empty scope onto the stack.
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Push a new scope with metadata.
    pub fn push_scope_with_metadata(&mut self, metadata: M) {
        self.scopes.push(Scope::with_metadata(metadata));
    }

    /// Pop the topmost scope. Panics if only one scope remains.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Enter a new scope and return a guard that pops it on drop.
    pub fn enter_scope(&mut self) -> ScopeGuard<'_, T, M> {
        self.push_scope();
        ScopeGuard { manager: self }
    }

    /// Enter a new scope with metadata and return a guard.
    pub fn enter_scope_with_metadata(&mut self, metadata: M) -> ScopeGuard<'_, T, M> {
        self.push_scope_with_metadata(metadata);
        ScopeGuard { manager: self }
    }

    /// Define a binding in the current (innermost) scope.
    pub fn define(&mut self, name: String, value: T, mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.define(name, value, mutable);
        }
    }

    /// Define an immutable binding in the current scope.
    pub fn define_immutable(&mut self, name: String, value: T) {
        self.define(name, value, false);
    }

    /// Define a mutable binding in the current scope.
    pub fn define_mutable(&mut self, name: String, value: T) {
        self.define(name, value, true);
    }

    /// Look up a binding, searching from innermost to outermost scope.
    pub fn lookup(&self, name: &str) -> Option<&Binding<T>> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.lookup(name) {
                return Some(binding);
            }
        }
        None
    }

    /// Look up a binding mutably, searching from innermost to outermost.
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Binding<T>> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains(name) {
                return scope.lookup_mut(name);
            }
        }
        None
    }

    /// Look up just the value.
    pub fn lookup_value(&self, name: &str) -> Option<&T> {
        self.lookup(name).map(|b| b.get())
    }

    /// Check if a name is defined in any scope.
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Check if a binding is mutable.
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.lookup(name).map(|b| b.is_mutable())
    }

    /// Get the current scope's metadata.
    pub fn current_metadata(&self) -> Option<&M> {
        self.scopes.last().and_then(|s| s.metadata())
    }

    /// Set the current scope's metadata.
    pub fn set_current_metadata(&mut self, metadata: M) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_metadata(metadata);
        }
    }

    /// Get all values from all scopes (for closure capture).
    /// Values from inner scopes shadow outer scopes.
    pub fn all_values(&self) -> HashMap<String, T> {
        let mut result = HashMap::new();
        for scope in &self.scopes {
            for (name, binding) in scope.iter() {
                result.insert(name.clone(), binding.get().clone());
            }
        }
        result
    }

    /// Get the current scope depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

impl<T: Clone + Debug, M: Clone + Debug> Debug for ScopeStack<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopeStack")
            .field("depth", &self.scopes.len())
            .field("scopes", &self.scopes)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_define_and_lookup() {
        let mut scope: Scope<i32> = Scope::new();
        scope.define("x".to_string(), 42, false);

        assert!(scope.contains("x"));
        assert_eq!(scope.lookup_value("x"), Some(&42));
        assert_eq!(scope.is_mutable("x"), Some(false));
    }

    #[test]
    fn test_scope_with_metadata() {
        let mut scope: Scope<i32, String> = Scope::with_metadata("test".to_string());
        assert_eq!(scope.metadata(), Some(&"test".to_string()));

        scope.clear_metadata();
        assert_eq!(scope.metadata(), None);
    }

    #[test]
    fn test_scope_values() {
        let mut scope: Scope<i32> = Scope::new();
        scope.define("a".to_string(), 1, false);
        scope.define("b".to_string(), 2, true);

        let values = scope.values();
        assert_eq!(values.get("a"), Some(&1));
        assert_eq!(values.get("b"), Some(&2));
    }

    #[test]
    fn test_scope_stack_basic() {
        let mut stack: ScopeStack<i32> = ScopeStack::new();
        stack.define_immutable("x".to_string(), 42);

        assert!(stack.contains("x"));
        assert_eq!(stack.lookup_value("x"), Some(&42));
    }

    #[test]
    fn test_scope_stack_nested() {
        let mut stack: ScopeStack<i32> = ScopeStack::new();
        stack.define_immutable("outer".to_string(), 1);

        stack.push_scope();
        stack.define_immutable("inner".to_string(), 2);

        // Both should be visible
        assert_eq!(stack.lookup_value("outer"), Some(&1));
        assert_eq!(stack.lookup_value("inner"), Some(&2));

        stack.pop_scope();

        // Only outer should be visible now
        assert_eq!(stack.lookup_value("outer"), Some(&1));
        assert_eq!(stack.lookup_value("inner"), None);
    }

    #[test]
    fn test_scope_stack_shadowing() {
        let mut stack: ScopeStack<i32> = ScopeStack::new();
        stack.define_immutable("x".to_string(), 1);

        stack.push_scope();
        stack.define_immutable("x".to_string(), 2);

        // Inner shadows outer
        assert_eq!(stack.lookup_value("x"), Some(&2));

        stack.pop_scope();

        // Outer is visible again
        assert_eq!(stack.lookup_value("x"), Some(&1));
    }

    #[test]
    fn test_scope_guard_raii() {
        let mut stack: ScopeStack<i32> = ScopeStack::new();
        stack.define_immutable("outer".to_string(), 1);

        {
            let mut guard = stack.enter_scope();
            guard.define_immutable("inner".to_string(), 2);

            assert_eq!(guard.lookup_value("outer"), Some(&1));
            assert_eq!(guard.lookup_value("inner"), Some(&2));
        }

        // Guard dropped, inner scope popped
        assert_eq!(stack.lookup_value("outer"), Some(&1));
        assert_eq!(stack.lookup_value("inner"), None);
    }

    #[test]
    fn test_scope_stack_with_metadata() {
        let mut stack: ScopeStack<i32, String> = ScopeStack::new();

        {
            let mut guard = stack.enter_scope_with_metadata("func".to_string());
            assert_eq!(guard.metadata(), Some(&"func".to_string()));
        }

        // Metadata gone with scope
        assert_eq!(stack.current_metadata(), None);
    }

    #[test]
    fn test_scope_stack_all_values() {
        let mut stack: ScopeStack<i32> = ScopeStack::new();
        stack.define_immutable("a".to_string(), 1);
        stack.push_scope();
        stack.define_immutable("b".to_string(), 2);
        stack.define_immutable("a".to_string(), 10); // Shadow

        let values = stack.all_values();
        assert_eq!(values.get("a"), Some(&10)); // Shadowed value
        assert_eq!(values.get("b"), Some(&2));
    }

    #[test]
    fn test_scope_stack_mutable_lookup() {
        let mut stack: ScopeStack<i32> = ScopeStack::new();
        stack.define_mutable("x".to_string(), 42);

        if let Some(binding) = stack.lookup_mut("x") {
            assert!(binding.set(100).is_ok());
        }

        assert_eq!(stack.lookup_value("x"), Some(&100));
    }
}
