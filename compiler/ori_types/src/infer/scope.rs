//! Scope-management helpers on [`InferEngine`].
//!
//! Covers loop-break-type stack, capability sets (`uses` / `with...in`),
//! and the `with_provided_capability` scoped-frame helper.

use rustc_hash::FxHashSet;

use ori_ir::Name;

use crate::Idx;

use super::InferEngine;

impl InferEngine<'_> {
    /// Push a loop break type variable onto the stack.
    /// Called when entering a `loop()` expression.
    pub fn push_loop_break_type(&mut self, ty: Idx) {
        self.loop_break_types.push(ty);
    }

    /// Pop the loop break type variable.
    /// Called when exiting a `loop()` expression.
    pub fn pop_loop_break_type(&mut self) -> Option<Idx> {
        self.loop_break_types.pop()
    }

    /// Get the current loop's break type variable (innermost loop).
    pub fn current_loop_break_type(&self) -> Option<Idx> {
        self.loop_break_types.last().copied()
    }

    // Capability Management

    /// Set capabilities for the current function scope.
    ///
    /// `current` contains capabilities declared via `uses` on the function.
    /// `provided` contains capabilities introduced via `with...in`.
    pub fn set_capabilities(&mut self, current: FxHashSet<Name>, provided: FxHashSet<Name>) {
        self.current_capabilities = current;
        self.provided_capabilities = provided;
    }

    /// Check if a capability is available (declared or provided).
    pub fn has_capability(&self, cap: Name) -> bool {
        self.current_capabilities.contains(&cap) || self.provided_capabilities.contains(&cap)
    }

    /// Get all available capabilities (declared + provided).
    pub fn available_capabilities(&self) -> Vec<Name> {
        self.current_capabilities
            .union(&self.provided_capabilities)
            .copied()
            .collect()
    }

    /// Add a provided capability (for `with...in` scoping).
    pub fn add_provided_capability(&mut self, cap: Name) {
        self.provided_capabilities.insert(cap);
    }

    /// Remove a provided capability.
    pub fn remove_provided_capability(&mut self, cap: Name) {
        self.provided_capabilities.remove(&cap);
    }

    /// Execute a closure with a temporarily provided capability.
    ///
    /// The capability is added before executing `f` and removed after.
    /// This implements the scoped semantics of `with...in`.
    pub fn with_provided_capability<T, F>(&mut self, cap: Name, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let was_present = self.provided_capabilities.insert(cap);
        let result = f(self);
        if !was_present {
            self.provided_capabilities.remove(&cap);
        }
        result
    }
}
