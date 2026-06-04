//! Per-body state storage helpers on [`InferEngine`].
//!
//! Expression-type storage, pattern-resolution records, monomorphization
//! instance tracking, and `current_function` metadata. All these helpers
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

    /// Record a concrete instantiation of a generic function.
    pub fn record_mono_instance(&mut self, instance: crate::MonoInstance) {
        self.mono_instances.push(instance);
    }

    /// Record a concrete instantiation tied to a call-site `ExprId`.
    ///
    /// Pushes the instance into `mono_instances` and emits a parallel
    /// `(call_expr_id, MonoInstanceId)` entry into `mono_dispatch_pre_dedup`
    /// for later remap-and-publish in [`crate::TypedModule::mono_dispatch_map`].
    /// The [`MonoInstanceId`] is the pre-push length of `mono_instances`
    /// (this body's local index space). The body-pass absorbs both vectors
    /// together via
    /// [`crate::check::ModuleChecker::accumulate_mono_session`], which
    /// offsets the local index into module-wide position before
    /// [`crate::check::ModuleChecker::finish_with_pool`] remaps once more
    /// across dedup + sort.
    ///
    /// Used by the eager call-site path
    /// (`infer::expr::calls::monomorphization::maybe_record_mono_instance`).
    /// The deferred-resolution path keeps using `record_mono_instance`
    /// until
    /// §C.2 sub-step 1b-deferred extends `DeferredMonoCall` to carry
    /// `ExprId`.
    pub fn record_mono_with_dispatch(
        &mut self,
        call_expr_id: ExprId,
        instance: crate::MonoInstance,
    ) {
        // Saturating `Vec::len() → u32` matches the workspace pattern at
        // `pool/substitute/mod.rs:541` and `ModuleChecker::accumulate_mono_session`;
        // strict workspace clippy denies `cast_possible_truncation` and
        // `expect`/`unwrap`. Per-body mono counts cannot reach `u32::MAX`
        // in practice — 4 billion mono instances inside a single body is
        // structurally unreachable for any compilable module.
        let local_idx = u32::try_from(self.mono_instances.len()).unwrap_or(u32::MAX);
        self.mono_instances.push(instance);
        self.mono_dispatch_pre_dedup
            .push((call_expr_id, MonoInstanceId(local_idx)));
    }

    /// Take mono instances, leaving an empty vector.
    pub fn take_mono_instances(&mut self) -> Vec<crate::MonoInstance> {
        std::mem::take(&mut self.mono_instances)
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
    pub fn record_composed_burden(
        &mut self,
        idx: crate::Idx,
        spec: crate::registry::burden::UserBurdenSpec,
    ) {
        self.composed_burdens.push((idx, spec));
    }

    /// Take composed-burden entries, leaving an empty vector. Body-pass
    /// extractor consumed by the module checker; entries flush into
    /// `TypeRegistry::burden` via
    /// `crate::check::ModuleChecker::flush_composed_burdens` from
    /// `bodies::finalize_body_and_export`. Downstream codegen reads the
    /// registered spec via `TypeRegistry::burden(idx)` without
    /// re-deriving.
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

    /// Set the current function being type-checked.
    pub fn set_current_function(&mut self, name: Option<Name>) {
        self.current_function = name;
    }

    /// Get the current function being type-checked.
    pub fn current_function(&self) -> Option<Name> {
        self.current_function
    }
}
