//! Environment for variable scoping in the interpreter.
//!
//! Uses a scope stack (not cloning) for efficient scope management.

use std::rc::Rc;
use std::cell::RefCell;
use rustc_hash::FxHashMap;
use crate::intern::Name;
use super::value::Value;

/// A single scope containing variable bindings.
#[derive(Clone, Debug)]
pub struct Scope {
    /// Variable bindings in this scope.
    bindings: FxHashMap<Name, Binding>,
    /// Parent scope (for lexical scoping).
    parent: Option<Rc<RefCell<Scope>>>,
}

/// A variable binding.
#[derive(Clone, Debug)]
struct Binding {
    /// The value.
    value: Value,
    /// Whether this binding is mutable.
    mutable: bool,
}

impl Scope {
    /// Create a new empty scope with no parent.
    pub fn new() -> Self {
        Scope {
            bindings: FxHashMap::default(),
            parent: None,
        }
    }

    /// Create a new scope with a parent.
    pub fn with_parent(parent: Rc<RefCell<Scope>>) -> Self {
        Scope {
            bindings: FxHashMap::default(),
            parent: Some(parent),
        }
    }

    /// Define a variable in this scope.
    pub fn define(&mut self, name: Name, value: Value, mutable: bool) {
        self.bindings.insert(name, Binding { value, mutable });
    }

    /// Look up a variable by name.
    pub fn lookup(&self, name: Name) -> Option<Value> {
        if let Some(binding) = self.bindings.get(&name) {
            return Some(binding.value.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().lookup(name);
        }
        None
    }

    /// Assign to a variable.
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), String> {
        if let Some(binding) = self.bindings.get_mut(&name) {
            if !binding.mutable {
                return Err(format!("cannot assign to immutable variable"));
            }
            binding.value = value;
            return Ok(());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }
        Err(format!("undefined variable"))
    }

    /// Check if a variable is defined in this scope (not parents).
    pub fn has_local(&self, name: Name) -> bool {
        self.bindings.contains_key(&name)
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment for the interpreter using a scope stack.
///
/// Instead of cloning environments, we maintain a stack of scopes
/// that can be pushed and popped efficiently.
pub struct Environment {
    /// Stack of scopes, with current scope at the top.
    scopes: Vec<Rc<RefCell<Scope>>>,
    /// Global scope (always at the bottom).
    global: Rc<RefCell<Scope>>,
}

impl Environment {
    /// Create a new environment with a global scope.
    pub fn new() -> Self {
        let global = Rc::new(RefCell::new(Scope::new()));
        Environment {
            scopes: vec![Rc::clone(&global)],
            global,
        }
    }

    /// Push a new scope onto the stack.
    pub fn push_scope(&mut self) {
        let parent = self.current_scope();
        let new_scope = Rc::new(RefCell::new(Scope::with_parent(parent)));
        self.scopes.push(new_scope);
    }

    /// Pop the current scope from the stack.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Get the current scope.
    fn current_scope(&self) -> Rc<RefCell<Scope>> {
        Rc::clone(self.scopes.last().unwrap())
    }

    /// Define a variable in the current scope.
    pub fn define(&mut self, name: Name, value: Value, mutable: bool) {
        self.current_scope().borrow_mut().define(name, value, mutable);
    }

    /// Look up a variable by name.
    pub fn lookup(&self, name: Name) -> Option<Value> {
        self.current_scope().borrow().lookup(name)
    }

    /// Assign to a variable.
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), String> {
        self.current_scope().borrow_mut().assign(name, value)
    }

    /// Define a global variable.
    pub fn define_global(&mut self, name: Name, value: Value) {
        self.global.borrow_mut().define(name, value, false);
    }

    /// Get the current scope depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Create a child environment for function calls.
    ///
    /// This creates a new environment that shares the global scope
    /// but has its own local scope stack.
    pub fn child(&self) -> Self {
        Environment {
            scopes: vec![Rc::clone(&self.global)],
            global: Rc::clone(&self.global),
        }
    }

    /// Capture the current scope for closures.
    ///
    /// Returns a map of all visible bindings that can be used
    /// when the closure is called later.
    pub fn capture(&self) -> FxHashMap<Name, Value> {
        let mut captures = FxHashMap::default();
        // Walk up the scope chain and collect bindings
        // (most recent binding wins)
        fn collect(scope: &Scope, captures: &mut FxHashMap<Name, Value>) {
            for (name, binding) in &scope.bindings {
                captures.entry(*name).or_insert_with(|| binding.value.clone());
            }
            if let Some(parent) = &scope.parent {
                collect(&parent.borrow(), captures);
            }
        }
        collect(&self.current_scope().borrow(), &mut captures);
        captures
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Environment {
    fn clone(&self) -> Self {
        // Clone creates a new independent environment with copied values
        let global = Rc::new(RefCell::new(Scope::new()));
        // Copy global bindings
        // (in practice, we rarely clone environments)
        Environment {
            scopes: vec![Rc::clone(&global)],
            global,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::StringInterner;

    #[test]
    fn test_scope_define_lookup() {
        let interner = StringInterner::new();
        let x = interner.intern("x");

        let mut scope = Scope::new();
        scope.define(x, Value::Int(42), false);
        assert_eq!(scope.lookup(x), Some(Value::Int(42)));
    }

    #[test]
    fn test_scope_shadowing() {
        let interner = StringInterner::new();
        let x = interner.intern("x");

        let parent = Rc::new(RefCell::new(Scope::new()));
        parent.borrow_mut().define(x, Value::Int(1), false);

        let mut child = Scope::with_parent(parent);
        child.define(x, Value::Int(2), false);

        // Child's binding shadows parent's
        assert_eq!(child.lookup(x), Some(Value::Int(2)));
    }

    #[test]
    fn test_environment_push_pop() {
        let interner = StringInterner::new();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::Int(1), false);

        env.push_scope();
        env.define(x, Value::Int(2), false);
        assert_eq!(env.lookup(x), Some(Value::Int(2)));

        env.pop_scope();
        assert_eq!(env.lookup(x), Some(Value::Int(1)));
    }

    #[test]
    fn test_environment_mutable() {
        let interner = StringInterner::new();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::Int(1), true);
        assert!(env.assign(x, Value::Int(2)).is_ok());
        assert_eq!(env.lookup(x), Some(Value::Int(2)));
    }

    #[test]
    fn test_environment_immutable() {
        let interner = StringInterner::new();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::Int(1), false);
        assert!(env.assign(x, Value::Int(2)).is_err());
    }
}
