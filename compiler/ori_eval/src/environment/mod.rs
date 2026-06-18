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

pub use ori_ir::Mutability;
use ori_ir::Name;
use ori_patterns::Value;

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

/// Environment for the interpreter — flat local scopes + a shared global scope.
///
/// Local scopes are a FLAT representation: `bindings` is one contiguous stack of
/// `(Name, Binding)` entries (innermost-last) and `scope_starts[i]` marks where local
/// scope `i` begins. `push_scope` is an O(1) `scope_starts.push` with NO per-scope
/// `Rc<RefCell<Scope>>` + `FxHashMap` allocation (the prior chain allocated both per
/// call); `pop_scope` truncates. The GLOBAL scope stays a shared `Rc<RefCell<Scope>>`
/// (a `HashMap` for fast top-level-fn lookup, shared across `child()` envs so recursion
/// and top-level functions resolve). Lookup scans the local bindings backward
/// (innermost shadows outer) then falls to the global scope.
pub struct Environment {
    /// Flat local-scope bindings, innermost-last. Shadowing resolves via backward scan.
    bindings: Vec<(Name, Binding)>,
    /// `scope_starts[i]` = index into `bindings` where local scope `i` begins.
    scope_starts: Vec<usize>,
    /// Shared global scope (recursion + top-level fns; shared across `child()` envs).
    global: LocalScope<Scope>,
}

impl Environment {
    /// Create a new environment with an empty local stack + a fresh global scope.
    pub fn new() -> Self {
        Environment {
            bindings: Vec::new(),
            scope_starts: Vec::new(),
            global: LocalScope::new(Scope::new()),
        }
    }

    /// Get the current scope depth (global scope is depth 1; each local scope adds 1).
    pub fn depth(&self) -> usize {
        self.scope_starts.len().saturating_add(1)
    }

    /// Push a new local scope — O(1), no allocation (just a `scope_starts` marker).
    #[inline]
    pub fn push_scope(&mut self) {
        self.scope_starts.push(self.bindings.len());
    }

    /// Pop the current local scope — truncate its bindings off the flat stack.
    #[inline]
    pub fn pop_scope(&mut self) {
        if let Some(start) = self.scope_starts.pop() {
            self.bindings.truncate(start);
        }
    }

    /// Define a variable in the current scope (a pushed local scope, else the global).
    #[inline]
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        if self.scope_starts.is_empty() {
            // No local scope pushed → a top-level define lands in the global scope.
            self.global.borrow_mut().define(name, value, mutability);
        } else {
            self.bindings.push((name, Binding { value, mutability }));
        }
    }

    /// Look up a variable: innermost local binding first, then the global scope.
    #[inline]
    pub fn lookup(&self, name: Name) -> Option<Value> {
        for (n, binding) in self.bindings.iter().rev() {
            if *n == name {
                return Some(binding.value.clone());
            }
        }
        self.global.borrow().lookup(name)
    }

    /// Assign to a variable: innermost local binding first, then the global scope.
    #[inline]
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), AssignError> {
        for (n, binding) in self.bindings.iter_mut().rev() {
            if *n == name {
                if !binding.mutability.is_mutable() {
                    return Err(AssignError::Immutable);
                }
                binding.value = value;
                return Ok(());
            }
        }
        self.global.borrow_mut().assign(name, value)
    }

    /// Define a global variable (immutable).
    pub fn define_global(&mut self, name: Name, value: Value) {
        self.global
            .borrow_mut()
            .define(name, value, Mutability::Immutable);
    }

    /// Create a child environment for function calls — shares the global scope
    /// (recursion + top-level fns) but starts with an empty local stack.
    #[must_use]
    pub fn child(&self) -> Self {
        Environment {
            bindings: Vec::new(),
            scope_starts: Vec::new(),
            global: self.global.clone(),
        }
    }

    /// Capture the LOCAL bindings for closures (global scope excluded).
    ///
    /// Globals are reached at call time via `child()` scope-sharing (`prepare_call_env`
    /// builds the call env from `self.env.child()`, which shares the global `Rc`), so
    /// top-level functions resolve there — NOT via the capture map. Innermost local
    /// binding wins on shadowing.
    pub fn capture(&self) -> FxHashMap<Name, Value> {
        let mut captures = FxHashMap::default();
        // Innermost-first (backward) so a shadowing inner binding wins via or_insert.
        for (name, binding) in self.bindings.iter().rev() {
            captures
                .entry(*name)
                .or_insert_with(|| binding.value.clone());
        }
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
