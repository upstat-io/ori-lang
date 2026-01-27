//! Environment for variable scoping in the interpreter.
//!
//! Uses a scope stack (not cloning) for efficient scope management.

// Rc is the intentional implementation detail of LocalScope<T>
#![expect(clippy::disallowed_types, reason = "Rc is the implementation of LocalScope<T>")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use ori_ir::Name;
use ori_patterns::Value;

// LocalScope<T> - Newtype wrapper for single-threaded scopes

/// A single-threaded scope wrapper for reference-counted interior mutability.
///
/// This type wraps `Rc<RefCell<T>>` and enforces that all scope allocations
/// go through the `LocalScope::new()` factory method.
///
/// # Why This Exists
/// - Prevents accidental `Rc::new()` / `RefCell::new()` calls in user code
/// - Enforces that all scope allocations go through factory methods
/// - Makes it clear that these scopes are single-threaded (not `Arc`)
///
/// # Thread Safety
/// `LocalScope<T>` is NOT thread-safe. It uses `Rc` internally, which is
/// faster than `Arc` but cannot be shared across threads. This is intentional
/// for the interpreter's scope management, which runs single-threaded.
///
/// # Zero-Cost Abstraction
/// The `#[repr(transparent)]` attribute ensures this has the same memory layout
/// as `Rc<RefCell<T>>`, so there's no overhead from the wrapper.
#[repr(transparent)]
pub struct LocalScope<T>(Rc<RefCell<T>>);

impl<T> LocalScope<T> {
    /// Create a new `LocalScope` wrapping the given value.
    ///
    /// This is the public factory method for creating scopes.
    #[inline]
    pub fn new(value: T) -> Self {
        LocalScope(Rc::new(RefCell::new(value)))
    }

    /// Borrow the inner value immutably.
    #[inline]
    pub fn borrow(&self) -> std::cell::Ref<'_, T> {
        self.0.borrow()
    }

    /// Borrow the inner value mutably.
    #[inline]
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, T> {
        self.0.borrow_mut()
    }
}

impl<T> Clone for LocalScope<T> {
    #[inline]
    fn clone(&self) -> Self {
        LocalScope(Rc::clone(&self.0))
    }
}

impl<T: fmt::Debug> fmt::Debug for LocalScope<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LocalScope").field(&self.0).finish()
    }
}

impl<T: Default> Default for LocalScope<T> {
    fn default() -> Self {
        LocalScope::new(T::default())
    }
}

impl<T> Deref for LocalScope<T> {
    type Target = RefCell<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Scope

/// A single scope containing variable bindings.
#[derive(Clone, Debug)]
pub struct Scope {
    /// Variable bindings in this scope.
    bindings: HashMap<Name, Binding>,
    /// Parent scope (for lexical scoping).
    parent: Option<LocalScope<Scope>>,
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
            bindings: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new scope with a parent.
    pub fn with_parent(parent: LocalScope<Scope>) -> Self {
        Scope {
            bindings: HashMap::new(),
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
                return Err("cannot assign to immutable variable".to_string());
            }
            binding.value = value;
            return Ok(());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }
        Err("undefined variable".to_string())
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
    scopes: Vec<LocalScope<Scope>>,
    /// Global scope (always at the bottom).
    global: LocalScope<Scope>,
}

impl Environment {
    /// Create a new environment with a global scope.
    pub fn new() -> Self {
        let global = LocalScope::new(Scope::new());
        Environment {
            scopes: vec![global.clone()],
            global,
        }
    }

    /// Push a new scope onto the stack.
    pub fn push_scope(&mut self) {
        let parent = self.current_scope();
        let new_scope = LocalScope::new(Scope::with_parent(parent));
        self.scopes.push(new_scope);
    }

    /// Pop the current scope from the stack.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Get the current scope.
    /// Returns the last scope on the stack, or the global scope if empty (which shouldn't happen).
    fn current_scope(&self) -> LocalScope<Scope> {
        self.scopes.last().unwrap_or(&self.global).clone()
    }

    /// Define a variable in the current scope.
    pub fn define(&mut self, name: Name, value: Value, mutable: bool) {
        self.current_scope()
            .borrow_mut()
            .define(name, value, mutable);
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
    #[must_use]
    pub fn child(&self) -> Self {
        Environment {
            scopes: vec![self.global.clone()],
            global: self.global.clone(),
        }
    }

    /// Capture the current scope for closures.
    ///
    /// Returns a map of all visible bindings that can be used
    /// when the closure is called later.
    pub fn capture(&self) -> HashMap<Name, Value> {
        fn collect(scope: &Scope, captures: &mut HashMap<Name, Value>) {
            for (name, binding) in &scope.bindings {
                captures
                    .entry(*name)
                    .or_insert_with(|| binding.value.clone());
            }
            if let Some(parent) = &scope.parent {
                collect(&parent.borrow(), captures);
            }
        }
        let mut captures = HashMap::new();
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
        let global = LocalScope::new(Scope::new());
        // Copy global bindings
        // (in practice, we rarely clone environments)
        Environment {
            scopes: vec![global.clone()],
            global,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

    #[test]
    fn test_scope_define_lookup() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut scope = Scope::new();
        scope.define(x, Value::int(42), false);
        assert_eq!(scope.lookup(x), Some(Value::int(42)));
    }

    #[test]
    fn test_scope_shadowing() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let parent = LocalScope::new(Scope::new());
        parent.borrow_mut().define(x, Value::int(1), false);

        let mut child = Scope::with_parent(parent);
        child.define(x, Value::int(2), false);

        // Child's binding shadows parent's
        assert_eq!(child.lookup(x), Some(Value::int(2)));
    }

    #[test]
    fn test_environment_push_pop() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);

        env.push_scope();
        env.define(x, Value::int(2), false);
        assert_eq!(env.lookup(x), Some(Value::int(2)));

        env.pop_scope();
        assert_eq!(env.lookup(x), Some(Value::int(1)));
    }

    #[test]
    fn test_environment_mutable() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), true);
        assert!(env.assign(x, Value::int(2)).is_ok());
        assert_eq!(env.lookup(x), Some(Value::int(2)));
    }

    #[test]
    fn test_environment_immutable() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);
        assert!(env.assign(x, Value::int(2)).is_err());
    }

    #[test]
    fn test_environment_capture() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let mut env = Environment::new();
        env.define(x, Value::int(1), false);
        env.push_scope();
        env.define(y, Value::int(2), false);

        let captures = env.capture();
        assert_eq!(captures.get(&x), Some(&Value::int(1)));
        assert_eq!(captures.get(&y), Some(&Value::int(2)));
    }

    #[test]
    fn test_local_scope_new() {
        let scope = LocalScope::new(42);
        assert_eq!(*scope.borrow(), 42);
    }

    #[test]
    fn test_local_scope_borrow_mut() {
        let scope = LocalScope::new(vec![1, 2, 3]);
        scope.borrow_mut().push(4);
        assert_eq!(*scope.borrow(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_local_scope_clone() {
        let scope1 = LocalScope::new(42);
        let scope2 = scope1.clone();

        // Both point to the same allocation
        scope1.borrow_mut().clone_from(&100);
        assert_eq!(*scope2.borrow(), 100);
    }

    #[test]
    fn test_local_scope_default() {
        let scope: LocalScope<i32> = LocalScope::default();
        assert_eq!(*scope.borrow(), 0);
    }

    #[test]
    fn test_local_scope_deref() {
        let scope = LocalScope::new(42);
        // Deref returns &RefCell<T>
        let borrowed = scope.deref().borrow();
        assert_eq!(*borrowed, 42);
    }
}
