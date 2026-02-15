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
mod tests;
