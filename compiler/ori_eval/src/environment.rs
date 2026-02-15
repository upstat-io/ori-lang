//! Environment for variable scoping in the interpreter.
//!
//! Uses a scope stack (not cloning) for efficient scope management.

// Rc is the intentional implementation detail of LocalScope<T>
#![expect(
    clippy::disallowed_types,
    reason = "Rc is the implementation of LocalScope<T>"
)]

use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use ori_ir::Name;
use ori_patterns::Value;

/// Whether a variable binding can be reassigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutability {
    /// Binding can be reassigned (`let x = ...`).
    Mutable,
    /// Binding cannot be reassigned (`let $x = ...`).
    Immutable,
}

/// Error returned by `Scope::assign` when assignment fails.
///
/// Typed error replaces the previous `Result<(), String>`, letting callers
/// distinguish the failure mode and produce the correct diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignError {
    /// Variable exists but is immutable.
    Immutable,
    /// Variable not found in any scope.
    Undefined,
}

impl Mutability {
    /// Returns `true` if this is `Mutable`.
    #[inline]
    pub fn is_mutable(self) -> bool {
        matches!(self, Mutability::Mutable)
    }
}

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

/// A single scope containing variable bindings.
#[derive(Clone, Debug)]
pub struct Scope {
    /// Variable bindings in this scope (`FxHashMap` for faster hashing with `Name` keys).
    bindings: FxHashMap<Name, Binding>,
    /// Parent scope (for lexical scoping).
    parent: Option<LocalScope<Scope>>,
}

/// A variable binding.
#[derive(Clone, Debug)]
struct Binding {
    /// The value.
    value: Value,
    /// Whether this binding can be reassigned.
    mutability: Mutability,
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
    pub fn with_parent(parent: LocalScope<Scope>) -> Self {
        Scope {
            bindings: FxHashMap::default(),
            parent: Some(parent),
        }
    }

    /// Define a variable in this scope.
    #[inline]
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        self.bindings.insert(name, Binding { value, mutability });
    }

    /// Look up a variable by name.
    #[inline]
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
    #[inline]
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), AssignError> {
        if let Some(binding) = self.bindings.get_mut(&name) {
            if !binding.mutability.is_mutable() {
                return Err(AssignError::Immutable);
            }
            binding.value = value;
            return Ok(());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }
        Err(AssignError::Undefined)
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

    /// Get the current scope depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Push a new scope onto the stack.
    #[inline]
    pub fn push_scope(&mut self) {
        let parent = self.current_scope();
        let new_scope = LocalScope::new(Scope::with_parent(parent));
        self.scopes.push(new_scope);
    }

    /// Pop the current scope from the stack.
    #[inline]
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Get the current scope.
    /// Returns the last scope on the stack, or the global scope if empty (which shouldn't happen).
    #[inline]
    fn current_scope(&self) -> LocalScope<Scope> {
        self.scopes.last().unwrap_or(&self.global).clone()
    }

    /// Define a variable in the current scope.
    #[inline]
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        self.scopes
            .last()
            .unwrap_or(&self.global)
            .borrow_mut()
            .define(name, value, mutability);
    }

    /// Look up a variable by name.
    ///
    /// Optimized to avoid cloning the current scope by accessing the last scope directly.
    #[inline]
    pub fn lookup(&self, name: Name) -> Option<Value> {
        self.scopes
            .last()
            .unwrap_or(&self.global)
            .borrow()
            .lookup(name)
    }

    /// Assign to a variable.
    #[inline]
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), AssignError> {
        self.scopes
            .last()
            .unwrap_or(&self.global)
            .borrow_mut()
            .assign(name, value)
    }

    /// Define a global variable (immutable).
    pub fn define_global(&mut self, name: Name, value: Value) {
        self.global
            .borrow_mut()
            .define(name, value, Mutability::Immutable);
    }

    /// Create a child environment for function calls.
    ///
    /// This creates a new environment that shares the global scope
    /// but has its own local scope stack.
    #[must_use]
    pub fn child(&self) -> Self {
        // Clone global once and reuse to avoid redundant Rc::clone
        let global = self.global.clone();
        Environment {
            scopes: vec![global.clone()],
            global,
        }
    }

    /// Capture the current scope for closures.
    ///
    /// Returns a map of all visible bindings that can be used
    /// when the closure is called later.
    pub fn capture(&self) -> FxHashMap<Name, Value> {
        fn collect(scope: &Scope, captures: &mut FxHashMap<Name, Value>) {
            for (name, binding) in &scope.bindings {
                captures
                    .entry(*name)
                    .or_insert_with(|| binding.value.clone());
            }
            if let Some(parent) = &scope.parent {
                collect(&parent.borrow(), captures);
            }
        }
        let mut captures = FxHashMap::default();
        collect(&self.current_scope().borrow(), &mut captures);
        captures
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
