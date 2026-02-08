//! Scope management for V2 codegen.
//!
//! `Scope` uses `im::HashMap` for O(1) structural-sharing clone, making
//! child scope creation essentially free. Each binding tracks whether it's
//! immutable (SSA register) or mutable (stack alloca).
//!
//! This replaces the existing `Locals` / `LocalStorage` in `builder.rs`
//! with ID-based types instead of raw inkwell values.

use im::HashMap;
use ori_ir::Name;

use super::value_id::{LLVMTypeId, ValueId};

// ---------------------------------------------------------------------------
// ScopeBinding
// ---------------------------------------------------------------------------

/// How a variable is stored in LLVM IR.
///
/// Immutable bindings use SSA values directly (no memory traffic).
/// Mutable bindings use stack allocations with explicit load/store.
#[derive(Clone, Copy, Debug)]
pub enum ScopeBinding {
    /// SSA value in a virtual register — cannot be reassigned.
    Immutable(ValueId),
    /// Stack-allocated via `alloca` — supports reassignment via load/store.
    Mutable {
        /// Pointer to the alloca'd stack slot.
        ptr: ValueId,
        /// Type of the stored value (needed for `load`).
        ty: LLVMTypeId,
    },
}

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// A lexical scope with variable bindings.
///
/// Uses `im::HashMap` for persistent structural sharing: calling `child()`
/// clones in O(1), and mutations in the child are isolated from the parent.
/// This is critical for codegen where each `if`/`match`/`for` block creates
/// a nested scope that inherits all parent bindings.
#[derive(Clone)]
pub struct Scope {
    bindings: HashMap<Name, ScopeBinding>,
}

impl Scope {
    /// Create an empty scope.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Create a child scope that inherits all parent bindings.
    ///
    /// O(1) via `im::HashMap` structural sharing — no allocation or copy.
    #[must_use]
    pub fn child(&self) -> Self {
        self.clone()
    }

    /// Bind an immutable variable (SSA value).
    pub fn bind_immutable(&mut self, name: Name, val: ValueId) {
        self.bindings.insert(name, ScopeBinding::Immutable(val));
    }

    /// Bind a mutable variable (stack-allocated pointer + type).
    pub fn bind_mutable(&mut self, name: Name, ptr: ValueId, ty: LLVMTypeId) {
        self.bindings
            .insert(name, ScopeBinding::Mutable { ptr, ty });
    }

    /// Look up a binding by name.
    pub fn lookup(&self, name: Name) -> Option<ScopeBinding> {
        self.bindings.get(&name).copied()
    }

    /// Check if a name is bound in this scope (or any parent).
    pub fn contains(&self, name: Name) -> bool {
        self.bindings.contains_key(&name)
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn name(n: u32) -> Name {
        Name::from_raw(n)
    }

    #[test]
    fn empty_scope_lookup_returns_none() {
        let scope = Scope::new();
        assert!(scope.lookup(name(1)).is_none());
        assert!(!scope.contains(name(1)));
    }

    #[test]
    fn bind_immutable_and_lookup() {
        let mut scope = Scope::new();
        let val = ValueId::NONE; // Sentinel — just testing the binding.
        scope.bind_immutable(name(10), val);

        let binding = scope.lookup(name(10));
        assert!(binding.is_some());
        match binding.unwrap() {
            ScopeBinding::Immutable(v) => assert_eq!(v, val),
            ScopeBinding::Mutable { .. } => panic!("expected immutable"),
        }
        assert!(scope.contains(name(10)));
    }

    #[test]
    fn bind_mutable_and_lookup() {
        let mut scope = Scope::new();
        let ptr = ValueId::NONE;
        let ty = LLVMTypeId::NONE;
        scope.bind_mutable(name(20), ptr, ty);

        match scope.lookup(name(20)).unwrap() {
            ScopeBinding::Mutable { ptr: p, ty: t } => {
                assert_eq!(p, ptr);
                assert_eq!(t, ty);
            }
            ScopeBinding::Immutable(_) => panic!("expected mutable"),
        }
    }

    #[test]
    fn child_scope_inherits_parent_bindings() {
        let mut parent = Scope::new();
        parent.bind_immutable(name(1), ValueId::NONE);

        let child = parent.child();
        assert!(child.contains(name(1)));
        assert!(child.lookup(name(1)).is_some());
    }

    #[test]
    fn child_scope_modifications_dont_affect_parent() {
        let mut parent = Scope::new();
        parent.bind_immutable(name(1), ValueId::NONE);

        let mut child = parent.child();
        child.bind_immutable(name(2), ValueId::NONE);

        // Child sees both.
        assert!(child.contains(name(1)));
        assert!(child.contains(name(2)));

        // Parent only sees the original.
        assert!(parent.contains(name(1)));
        assert!(!parent.contains(name(2)));
    }

    #[test]
    fn variable_shadowing_in_child_scope() {
        let mut parent = Scope::new();
        // Use distinct ValueIds to tell apart parent vs child binding.
        let parent_val = ValueId::NONE;
        parent.bind_immutable(name(1), parent_val);

        let mut child = parent.child();
        let ptr = ValueId::NONE;
        let ty = LLVMTypeId::NONE;
        // Shadow the immutable with a mutable in the child.
        child.bind_mutable(name(1), ptr, ty);

        // Child sees the mutable binding.
        match child.lookup(name(1)).unwrap() {
            ScopeBinding::Mutable { .. } => {} // expected
            ScopeBinding::Immutable(_) => panic!("expected child's mutable binding"),
        }

        // Parent still sees the immutable binding.
        match parent.lookup(name(1)).unwrap() {
            ScopeBinding::Immutable(_) => {} // expected
            ScopeBinding::Mutable { .. } => panic!("expected parent's immutable binding"),
        }
    }

    #[test]
    fn deeply_nested_scopes() {
        let mut s0 = Scope::new();
        s0.bind_immutable(name(1), ValueId::NONE);

        let mut s1 = s0.child();
        s1.bind_immutable(name(2), ValueId::NONE);

        let mut s2 = s1.child();
        s2.bind_immutable(name(3), ValueId::NONE);

        // Innermost scope sees all three.
        assert!(s2.contains(name(1)));
        assert!(s2.contains(name(2)));
        assert!(s2.contains(name(3)));

        // Middle scope sees first two.
        assert!(s1.contains(name(1)));
        assert!(s1.contains(name(2)));
        assert!(!s1.contains(name(3)));

        // Outermost scope sees only the first.
        assert!(s0.contains(name(1)));
        assert!(!s0.contains(name(2)));
        assert!(!s0.contains(name(3)));
    }
}
