//! Per-body state storage helpers on [`InferEngine`].
//!
//! Expression-type storage, pattern-resolution records, monomorphization
//! instance tracking, and deferred-caller metadata. All these helpers
//! either push into an internal `Vec` / `FxHashMap` or extract (via
//! `std::mem::take`) for the body-pass caller.

use rustc_hash::FxHashMap;

use ori_ir::{ExprId, Name};

use crate::{Idx, MonoInstanceId, PatternKey, PatternResolution};

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

    /// Record a concrete generic callable demand without an AST call-dispatch
    /// entry, such as a user-defined operator method.
    pub fn record_mono_instance(&mut self, instance: crate::MonoInstance) {
        self.mono_instances.push(instance);
    }

    /// Record a concrete instantiation tied to a call-site `ExprId`.
    ///
    /// Pushes the instance into `mono_instances` and emits a parallel
    /// `(call_expr_id, MonoInstanceId)` entry into `mono_dispatch_pre_dedup`
    /// for later remap-and-publish in [`crate::TypedModule::mono_dispatch_map`].
    /// The [`MonoInstanceId`] is the pre-push length of `mono_instances`
    /// (this body's local index space). Body finalization absorbs both vectors
    /// together, offsets the local index into module-wide position, then module
    /// finalization remaps once more across deduplication and sorting.
    ///
    /// Eager call sites publish dispatch immediately; deferred calls remain
    /// body-only demands until they acquire a call-site `ExprId`.
    pub(super) fn record_mono_with_dispatch(
        &mut self,
        call_expr_id: ExprId,
        instance: crate::MonoInstance,
    ) {
        let Ok(local_idx) = u32::try_from(self.mono_instances.len()) else {
            unreachable!("body mono-instance table exceeds MonoInstanceId capacity");
        };
        self.mono_instances.push(instance);
        self.mono_dispatch_pre_dedup
            .push((call_expr_id, MonoInstanceId::new(local_idx)));
    }

    /// Take mono instances, leaving an empty vector.
    pub fn take_mono_instances(&mut self) -> Vec<crate::MonoInstance> {
        std::mem::take(&mut self.mono_instances)
    }

    /// Record the exact ordered provider selection for one free call.
    pub fn record_capability_call(&mut self, key: ExprId, call: crate::CapabilityCallSite) {
        self.capability_call_sites.push((key, call));
    }

    /// Take capability call-site selections, leaving an empty vector.
    pub fn take_capability_calls(&mut self) -> Vec<(ExprId, crate::CapabilityCallSite)> {
        std::mem::take(&mut self.capability_call_sites)
    }

    // Assignment-Target Desugar Recording

    /// Record the type-directed desugar plan for an `AssignTarget` chain.
    ///
    /// `key` is the module-wide AST `ExprId` of the `AssignTarget` node;
    /// `level_types` are the resolved receiver-read types per chain level
    /// (length `steps + 1`). Canonical lowering consumes the plan to synthesize
    /// the pure-reassignment form.
    pub fn record_assign_desugar(&mut self, key: ExprId, level_types: Vec<Idx>) {
        self.assign_desugars
            .push((key, crate::AssignDesugar { level_types }));
    }

    /// Take assignment-target desugar plans, leaving an empty vector.
    pub fn take_assign_desugars(&mut self) -> Vec<(ExprId, crate::AssignDesugar)> {
        std::mem::take(&mut self.assign_desugars)
    }

    /// Record a resolved module-alias qualified call.
    ///
    /// `key` is the call's module-wide AST `ExprId`; `qualified` is the
    /// qualified imported-function `Name` (`"alias.func"`) the call rewrites to
    /// in `ori_canon`.
    pub fn record_module_alias_call(&mut self, key: ExprId, qualified: Name) {
        self.module_alias_calls.push((key, qualified));
    }

    /// Take module-alias qualified-call entries, leaving an empty vector.
    pub fn take_module_alias_calls(&mut self) -> Vec<(ExprId, Name)> {
        std::mem::take(&mut self.module_alias_calls)
    }

    /// Record an Iterable->Iterator routed method call.
    ///
    /// `key` is the exact source call `ExprId`; `route` owns every type
    /// needed to materialize the iterator path in `ori_canon`.
    pub fn record_iter_route(&mut self, key: ExprId, route: crate::IterMethodRoute) {
        self.iter_route_desugars.push((key, route));
    }

    /// Take Iterable->Iterator route entries, leaving an empty vector.
    pub fn take_iter_routes(&mut self) -> Vec<(ExprId, crate::IterMethodRoute)> {
        std::mem::take(&mut self.iter_route_desugars)
    }

    /// Take mono dispatch pre-dedup entries, leaving an empty vector.
    pub fn take_mono_dispatch_pre_dedup(&mut self) -> Vec<(ExprId, MonoInstanceId)> {
        std::mem::take(&mut self.mono_dispatch_pre_dedup)
    }

    /// Record a deferred mono call (generic calling generic).
    pub fn record_deferred_mono_call(&mut self, call: crate::DeferredMonoCall) {
        self.deferred_mono_calls.push(call);
    }

    /// Take deferred mono calls, leaving an empty vector.
    pub fn take_deferred_mono_calls(&mut self) -> Vec<crate::DeferredMonoCall> {
        std::mem::take(&mut self.deferred_mono_calls)
    }

    /// Record a composed `UserBurdenSpec` for a monomorphized generic-builtin
    /// `Idx`. Called once per first-instantiation by
    /// `infer::expr::calls::monomorphization::maybe_record_mono_instance`
    /// after substituting type args into the relevant `BURDEN_TABLE`
    /// template via `registry::burden_compose::compose_user_burden`.
    pub(super) fn record_composed_burden(
        &mut self,
        idx: crate::Idx,
        spec: crate::registry::burden::UserBurdenSpec,
    ) {
        self.composed_burdens.push((idx, spec));
    }

    /// Take composed-burden entries, leaving an empty vector. Body
    /// finalization registers each drained entry in the `TypeRegistry`, where
    /// codegen reads it via `TypeRegistry::burden(idx)`.
    ///
    /// Runs a final pool sweep (`compose_builtin_burdens_for_resolved_types`)
    /// before draining so that collection instances minted by literals
    /// (`["a", "b"]`, `{k: v}`, `Set` builders) — which never flow through a
    /// generic free-function monomorphization — also get their
    /// `UserBurdenSpec` composed. Without the sweep, a body that constructs a
    /// `[str]` but calls no generic function would leak the backing buffer in
    /// the standalone burden ledger (Spec: Annex E §AIMS, RL-2 — dec at
    /// last-use). The accumulator dedups against entries already pushed by the
    /// per-monomorphization site, so the sweep is idempotent in effect.
    pub fn take_composed_burdens(
        &mut self,
    ) -> Vec<(crate::Idx, crate::registry::burden::UserBurdenSpec)> {
        crate::infer::expr::compose_builtin_burdens_for_resolved_types(self);
        std::mem::take(&mut self.composed_burdens)
    }

    /// Set the exact body that owns deferred generic calls and its type-binder
    /// roots in declaration order.
    pub fn set_deferred_mono_caller(
        &mut self,
        caller: crate::DeferredMonoCaller,
        binder_roots: Vec<Idx>,
    ) {
        self.deferred_mono_caller = Some((caller, binder_roots));
    }

    /// Get the current deferred-call owner and its ordered type-binder roots.
    pub fn deferred_mono_caller(&self) -> Option<(crate::DeferredMonoCaller, &[Idx])> {
        self.deferred_mono_caller
            .as_ref()
            .map(|(caller, roots)| (*caller, roots.as_slice()))
    }
}
