//! Scope-management helpers on [`InferEngine`].
//!
//! Covers loop-break-type stack, capability sets (`uses` / `with...in`),
//! and the `with_provided_capability` scoped-frame helper.

use rustc_hash::FxHashSet;

use ori_ir::{Name, Span};

use crate::Idx;

use super::InferEngine;

/// Per-loop context tracked while checking a loop body.
///
/// Records the break-value-type unification target plus whether `break value`
/// and `continue value` are permitted in this loop form. Per spec
/// (Spec: Clause 14 / Clause 16):
/// - `loop { }` permits `break value`, forbids `continue value`.
/// - `for...yield` permits both.
/// - `while...do` / `for...do` forbid both (the loop has type `void`).
#[derive(Copy, Clone, Debug)]
pub struct LoopContext {
    /// Label declared on this loop, or `Name::EMPTY` when unlabeled.
    ///
    /// A labeled `break`/`continue` resolves its value-permission against the
    /// context whose `label` matches the target — not the innermost loop.
    pub label: Name,
    /// Unification target for `break value` (the loop's break type variable).
    pub break_ty: Idx,
    /// Whether `break value` is permitted in this loop form.
    pub break_value_allowed: bool,
    /// Whether `continue value` is permitted in this loop form.
    pub continue_value_allowed: bool,
    /// Which surface loop form this context describes (for diagnostics).
    pub form: LoopForm,
}

/// Carrier observed at one explicit `?` expression inside the active try
/// boundary. Result observations retain the propagated error type so all
/// operations in the boundary can be reconciled.
#[derive(Copy, Clone, Debug)]
pub(crate) enum TryPropagation {
    Option { span: Span },
    Result { error_ty: Idx, span: Span },
}

/// The surface loop form that introduced a [`LoopContext`].
///
/// Used only to name the offending construct in E0860 / E0861 diagnostics.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LoopForm {
    /// `loop { body }`.
    Loop,
    /// `while cond do body`.
    While,
    /// `for x in iter do body`.
    ForDo,
    /// `for x in iter yield body`.
    ForYield,
}

impl InferEngine<'_> {
    /// Enter a `try {}` propagation boundary.
    pub(crate) fn push_try_boundary(&mut self) {
        self.try_boundaries.push(Some(Vec::new()));
    }

    /// Leave a `try {}` propagation boundary and return only the explicit
    /// `?` operations inferred directly inside it.
    pub(crate) fn pop_try_boundary(&mut self) -> Vec<TryPropagation> {
        if let Some(Some(propagations)) = self.try_boundaries.pop() {
            propagations
        } else {
            debug_assert!(false, "try propagation boundary stack is unbalanced");
            Vec::new()
        }
    }

    /// Prevent `?` inside a nested function body from attaching to an
    /// enclosing try block. A nested try boundary outside this barrier
    /// remains independently active.
    pub(crate) fn push_try_boundary_barrier(&mut self) {
        self.try_boundaries.push(None);
    }

    /// Leave a nested-function propagation barrier.
    pub(crate) fn pop_try_boundary_barrier(&mut self) {
        let popped = self.try_boundaries.pop();
        debug_assert!(
            matches!(popped, Some(None)),
            "try propagation function barrier stack is unbalanced"
        );
    }

    /// Attach an explicit `?` operation to the innermost active try boundary.
    /// An absent boundary propagates to the current function instead.
    pub(crate) fn record_try_propagation(&mut self, propagation: TryPropagation) {
        if let Some(Some(propagations)) = self.try_boundaries.last_mut() {
            propagations.push(propagation);
        }
    }

    /// Push a loop context onto the stack.
    /// Called when entering any loop form.
    pub fn push_loop_context(&mut self, ctx: LoopContext) {
        self.loop_contexts.push(ctx);
    }

    /// Pop the innermost loop context.
    /// Called when exiting a loop form.
    pub fn pop_loop_context(&mut self) -> Option<LoopContext> {
        self.loop_contexts.pop()
    }

    /// Get the innermost loop's context (the enclosing loop), if any.
    pub fn current_loop_context(&self) -> Option<LoopContext> {
        self.loop_contexts.last().copied()
    }

    /// Resolve the loop context a `break`/`continue` targets.
    ///
    /// `label == Name::EMPTY` (unlabeled) → innermost loop. A non-empty label
    /// → the nearest enclosing loop whose `label` matches; `None` when no
    /// enclosing loop carries that label; the caller handles label-resolution
    /// errors.
    pub fn resolve_loop_context(&self, label: Name) -> Option<LoopContext> {
        if label == Name::EMPTY {
            return self.current_loop_context();
        }
        self.loop_contexts
            .iter()
            .rev()
            .find(|ctx| ctx.label == label)
            .copied()
    }

    /// Get the innermost loop's break type variable, if inside a loop.
    pub fn current_loop_break_type(&self) -> Option<Idx> {
        self.loop_contexts.last().map(|ctx| ctx.break_ty)
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

    /// Discard a capability when its `with` scope ends.
    pub fn remove_provided_capability(&mut self, cap: Name) {
        self.provided_capabilities.remove(&cap);
    }

    /// Execute a closure while a capability is provided.
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
