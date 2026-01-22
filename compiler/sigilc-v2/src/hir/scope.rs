//! Scope management for name resolution and type checking.
//!
//! Scopes form a tree structure where each scope has a parent.
//! Variables are looked up by walking up the scope chain.

use crate::intern::{Name, TypeId};
use rustc_hash::FxHashMap;
use std::fmt;

/// Unique identifier for a scope.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ScopeId(u32);

impl ScopeId {
    /// The global/module scope.
    pub const GLOBAL: ScopeId = ScopeId(0);

    pub fn new(id: u32) -> Self {
        ScopeId(id)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::GLOBAL {
            write!(f, "ScopeId::GLOBAL")
        } else {
            write!(f, "ScopeId({})", self.0)
        }
    }
}

/// A local variable binding in a scope.
#[derive(Clone, Debug)]
pub struct LocalVar {
    /// Variable name.
    pub name: Name,
    /// Inferred or declared type.
    pub ty: TypeId,
    /// Whether the variable is mutable.
    pub mutable: bool,
    /// Scope where this variable was defined.
    pub scope: ScopeId,
}

/// A binding that can be looked up by name.
#[derive(Clone, Debug)]
pub enum Binding {
    /// Local variable.
    Local(LocalVar),
    /// Function parameter.
    Param {
        name: Name,
        ty: TypeId,
        index: usize,
    },
    /// Loop variable (for x in items).
    LoopVar {
        name: Name,
        ty: TypeId,
    },
    /// Pattern binding (let { x, y } = ...).
    Pattern {
        name: Name,
        ty: TypeId,
    },
}

impl Binding {
    pub fn name(&self) -> Name {
        match self {
            Binding::Local(v) => v.name,
            Binding::Param { name, .. } => *name,
            Binding::LoopVar { name, .. } => *name,
            Binding::Pattern { name, .. } => *name,
        }
    }

    pub fn ty(&self) -> TypeId {
        match self {
            Binding::Local(v) => v.ty,
            Binding::Param { ty, .. } => *ty,
            Binding::LoopVar { ty, .. } => *ty,
            Binding::Pattern { ty, .. } => *ty,
        }
    }

    pub fn is_mutable(&self) -> bool {
        match self {
            Binding::Local(v) => v.mutable,
            _ => false,
        }
    }
}

/// Internal scope data.
#[derive(Clone, Debug)]
struct ScopeData {
    /// Parent scope (None for global scope).
    parent: Option<ScopeId>,
    /// Local bindings in this scope.
    bindings: FxHashMap<Name, Binding>,
    /// Expected return type for this scope (if in a function).
    return_type: Option<TypeId>,
    /// Whether this is a loop scope (for break/continue).
    is_loop: bool,
}

/// Manager for all scopes in a module.
///
/// Scopes are stored in a flat vector for cache efficiency.
/// The scope tree is navigated via parent pointers.
pub struct Scopes {
    /// All scopes.
    scopes: Vec<ScopeData>,
    /// Current active scope.
    current: ScopeId,
}

impl Scopes {
    /// Create a new scope manager with just the global scope.
    pub fn new() -> Self {
        let global = ScopeData {
            parent: None,
            bindings: FxHashMap::default(),
            return_type: None,
            is_loop: false,
        };

        Scopes {
            scopes: vec![global],
            current: ScopeId::GLOBAL,
        }
    }

    /// Get the current scope ID.
    pub fn current(&self) -> ScopeId {
        self.current
    }

    /// Enter a new child scope.
    pub fn push(&mut self) -> ScopeId {
        let id = ScopeId::new(self.scopes.len() as u32);
        let scope = ScopeData {
            parent: Some(self.current),
            bindings: FxHashMap::default(),
            return_type: self.scopes[self.current.index()].return_type,
            is_loop: false,
        };
        self.scopes.push(scope);
        self.current = id;
        id
    }

    /// Enter a new function scope with a return type.
    pub fn push_function(&mut self, return_type: TypeId) -> ScopeId {
        let id = ScopeId::new(self.scopes.len() as u32);
        let scope = ScopeData {
            parent: Some(self.current),
            bindings: FxHashMap::default(),
            return_type: Some(return_type),
            is_loop: false,
        };
        self.scopes.push(scope);
        self.current = id;
        id
    }

    /// Enter a new loop scope.
    pub fn push_loop(&mut self) -> ScopeId {
        let id = ScopeId::new(self.scopes.len() as u32);
        let scope = ScopeData {
            parent: Some(self.current),
            bindings: FxHashMap::default(),
            return_type: self.scopes[self.current.index()].return_type,
            is_loop: true,
        };
        self.scopes.push(scope);
        self.current = id;
        id
    }

    /// Exit the current scope and return to parent.
    pub fn pop(&mut self) -> ScopeId {
        let old = self.current;
        if let Some(parent) = self.scopes[old.index()].parent {
            self.current = parent;
        }
        old
    }

    /// Define a binding in the current scope.
    pub fn define(&mut self, binding: Binding) {
        let name = binding.name();
        self.scopes[self.current.index()].bindings.insert(name, binding);
    }

    /// Define a local variable in the current scope.
    pub fn define_local(&mut self, name: Name, ty: TypeId, mutable: bool) {
        self.define(Binding::Local(LocalVar {
            name,
            ty,
            mutable,
            scope: self.current,
        }));
    }

    /// Define a function parameter in the current scope.
    pub fn define_param(&mut self, name: Name, ty: TypeId, index: usize) {
        self.define(Binding::Param { name, ty, index });
    }

    /// Define a loop variable in the current scope.
    pub fn define_loop_var(&mut self, name: Name, ty: TypeId) {
        self.define(Binding::LoopVar { name, ty });
    }

    /// Look up a binding by name, searching up the scope chain.
    pub fn lookup(&self, name: Name) -> Option<&Binding> {
        let mut scope_id = self.current;

        loop {
            let scope = &self.scopes[scope_id.index()];

            if let Some(binding) = scope.bindings.get(&name) {
                return Some(binding);
            }

            match scope.parent {
                Some(parent) => scope_id = parent,
                None => return None,
            }
        }
    }

    /// Look up a binding only in the current scope (no parent search).
    pub fn lookup_local(&self, name: Name) -> Option<&Binding> {
        self.scopes[self.current.index()].bindings.get(&name)
    }

    /// Get the expected return type for the current function scope.
    pub fn return_type(&self) -> Option<TypeId> {
        self.scopes[self.current.index()].return_type
    }

    /// Check if we're currently in a loop scope.
    pub fn in_loop(&self) -> bool {
        let mut scope_id = self.current;

        loop {
            let scope = &self.scopes[scope_id.index()];

            if scope.is_loop {
                return true;
            }

            match scope.parent {
                Some(parent) => scope_id = parent,
                None => return false,
            }
        }
    }

    /// Get the number of scopes.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Check if only the global scope exists.
    pub fn is_empty(&self) -> bool {
        self.scopes.len() <= 1
    }

    /// Reset to initial state (keeps global scope).
    pub fn reset(&mut self) {
        self.scopes.truncate(1);
        self.scopes[0].bindings.clear();
        self.current = ScopeId::GLOBAL;
    }
}

impl Default for Scopes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::StringInterner;

    #[test]
    fn test_scope_push_pop() {
        let mut scopes = Scopes::new();
        assert_eq!(scopes.current(), ScopeId::GLOBAL);

        let s1 = scopes.push();
        assert_eq!(scopes.current(), s1);

        let s2 = scopes.push();
        assert_eq!(scopes.current(), s2);

        scopes.pop();
        assert_eq!(scopes.current(), s1);

        scopes.pop();
        assert_eq!(scopes.current(), ScopeId::GLOBAL);
    }

    #[test]
    fn test_scope_lookup() {
        let interner = StringInterner::new();
        let mut scopes = Scopes::new();

        let x = interner.intern("x");
        let y = interner.intern("y");

        // Define x in global scope
        scopes.define_local(x, TypeId::INT, false);

        // Enter new scope and define y
        scopes.push();
        scopes.define_local(y, TypeId::STR, true);

        // Can find both x and y
        assert!(scopes.lookup(x).is_some());
        assert!(scopes.lookup(y).is_some());

        // Pop scope - y should be gone
        scopes.pop();
        assert!(scopes.lookup(x).is_some());
        assert!(scopes.lookup(y).is_none());
    }

    #[test]
    fn test_scope_shadowing() {
        let interner = StringInterner::new();
        let mut scopes = Scopes::new();

        let x = interner.intern("x");

        // Define x as int in global scope
        scopes.define_local(x, TypeId::INT, false);

        // Enter new scope and shadow x as str
        scopes.push();
        scopes.define_local(x, TypeId::STR, true);

        // Should find the shadowed version (str)
        let binding = scopes.lookup(x).unwrap();
        assert_eq!(binding.ty(), TypeId::STR);

        // Pop - should find original (int)
        scopes.pop();
        let binding = scopes.lookup(x).unwrap();
        assert_eq!(binding.ty(), TypeId::INT);
    }

    #[test]
    fn test_loop_scope() {
        let mut scopes = Scopes::new();

        assert!(!scopes.in_loop());

        scopes.push_loop();
        assert!(scopes.in_loop());

        // Nested non-loop scope still in loop
        scopes.push();
        assert!(scopes.in_loop());

        scopes.pop();
        scopes.pop();
        assert!(!scopes.in_loop());
    }

    #[test]
    fn test_function_return_type() {
        let mut scopes = Scopes::new();

        assert!(scopes.return_type().is_none());

        scopes.push_function(TypeId::INT);
        assert_eq!(scopes.return_type(), Some(TypeId::INT));

        // Nested scope inherits return type
        scopes.push();
        assert_eq!(scopes.return_type(), Some(TypeId::INT));
    }
}
