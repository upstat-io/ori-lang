//! Per-body state storage helpers on [`InferEngine`].
//!
//! Expression-type storage, pattern-resolution records, monomorphization
//! instance tracking, and `current_function` metadata. All these helpers
//! either push into an internal `Vec` / `FxHashMap` or extract (via
//! `std::mem::take`) for the body-pass caller.

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::{Idx, PatternKey, PatternResolution};

use super::{ExprIndex, InferEngine};

impl InferEngine<'_> {
    // Expression Type Storage

    /// Store the inferred type for an expression.
    pub fn store_type(&mut self, expr: ExprIndex, ty: Idx) {
        self.expr_types.insert(expr, ty);
    }

    /// Get the inferred type for an expression.
    pub fn get_type(&self, expr: ExprIndex) -> Option<Idx> {
        self.expr_types.get(&expr).copied()
    }

    /// Get all expression types.
    pub fn expr_types(&self) -> &FxHashMap<ExprIndex, Idx> {
        &self.expr_types
    }

    /// Take expression types, leaving an empty map.
    pub fn take_expr_types(&mut self) -> FxHashMap<ExprIndex, Idx> {
        std::mem::take(&mut self.expr_types)
    }

    // Pattern Resolution

    /// Record that a `Binding` pattern was resolved to a unit variant.
    pub fn record_pattern_resolution(&mut self, key: PatternKey, res: PatternResolution) {
        self.pattern_resolutions.push((key, res));
    }

    /// Take pattern resolutions, leaving an empty vector.
    pub fn take_pattern_resolutions(&mut self) -> Vec<(PatternKey, PatternResolution)> {
        std::mem::take(&mut self.pattern_resolutions)
    }

    // Monomorphization Recording

    /// Record a concrete instantiation of a generic function.
    pub fn record_mono_instance(&mut self, instance: crate::MonoInstance) {
        self.mono_instances.push(instance);
    }

    /// Take mono instances, leaving an empty vector.
    pub fn take_mono_instances(&mut self) -> Vec<crate::MonoInstance> {
        std::mem::take(&mut self.mono_instances)
    }

    /// Record a deferred mono call (generic calling generic).
    pub fn record_deferred_mono_call(&mut self, call: crate::DeferredMonoCall) {
        self.deferred_mono_calls.push(call);
    }

    /// Take deferred mono calls, leaving an empty vector.
    pub fn take_deferred_mono_calls(&mut self) -> Vec<crate::DeferredMonoCall> {
        std::mem::take(&mut self.deferred_mono_calls)
    }

    /// Set the current function being type-checked.
    pub fn set_current_function(&mut self, name: Option<Name>) {
        self.current_function = name;
    }

    /// Get the current function being type-checked.
    pub fn current_function(&self) -> Option<Name> {
        self.current_function
    }
}
