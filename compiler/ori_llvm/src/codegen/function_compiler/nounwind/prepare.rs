//! Preparation phase: lower functions through ARC pipeline without emitting LLVM IR.

use ori_arc::lower_function_can;
use ori_arc::verify::VerifyError;
use ori_ir::canon::CanonResult;
use ori_ir::{Function, Name, StringInterner};
use ori_types::{FunctionSig, Idx};
use rustc_hash::FxHashMap;
use tracing::debug;

use super::types::{PreparedFunction, PreparedLambda};
use crate::codegen::abi::FunctionAbi;
use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::FunctionId;

/// Rewrite Apply / Invoke call targets in every cached `ArcFunction` to use
/// the mangled mono name.
///
/// INVARIANT: AIMS interprocedural contract lookup (`sigs.get(callee_name)`)
/// must resolve to the actual analyzed function. Without this rewrite, monos
/// reference generic names (`@id`) at call sites, but `analyze_program`
/// produces contracts keyed on mangled mono names (`@id$m$4_Lint`), so
/// transitive `transfers_through_return` propagation through forwarder chains
/// silently fails.
///
/// Resolution strategy (mirrors codegen's `lookup_mono_dispatch`):
/// 1. If `mono_instance_id` is set, use `MonoInstanceId → Name` lookup.
/// 2. Otherwise, type-match the Apply's arg types against the candidate
///    monos of `func.callee` — same logic LLVM emission uses as the legacy
///    fallback. Required because typeck does not yet populate
///    `mono_dispatch_map_can` for generic-call sites in mono bodies.
///
/// Idempotent: rewrites only when a candidate mono matches the call site;
/// no-op for non-generic calls.
#[allow(
    clippy::implicit_hasher,
    reason = "FxHashMap is the workspace-wide hasher convention; consumers always pass FxHashMap from arc_cache"
)]
pub fn rewrite_apply_targets_for_monos(
    arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
    mono_functions: &[crate::monomorphize::MonoFunction],
    pool: &ori_types::Pool,
) {
    let maps = MonoTargetMaps::build(mono_functions, pool);
    for (_name, (arc_func, lambdas)) in arc_cache.iter_mut() {
        maps.rewrite_function(arc_func, lambdas, pool);
    }
}

/// The two mono-name lookup tables a call-target rewrite consumes: a
/// `MonoInstanceId → mangled Name` map and a `generic Name → [(resolved param
/// types, mangled Name)]` candidate list. SSOT for the mono-target rewrite — both
/// the `arc_cache` walk and the per-test-body rewrite build it once and call
/// `rewrite_function`.
pub struct MonoTargetMaps {
    mono_by_id: FxHashMap<ori_ir::canon::MonoInstanceId, Name>,
    mono_by_generic: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
}

impl MonoTargetMaps {
    pub fn build(
        mono_functions: &[crate::monomorphize::MonoFunction],
        pool: &ori_types::Pool,
    ) -> Self {
        let mut mono_by_id = FxHashMap::default();
        let mut mono_by_generic: FxHashMap<Name, Vec<(Vec<Idx>, Name)>> = FxHashMap::default();
        for mono_fn in mono_functions {
            for &id in &mono_fn.instance_ids {
                mono_by_id.insert(id, mono_fn.mangled_name);
            }
            let resolved_params: Vec<Idx> = mono_fn
                .sig
                .param_types
                .iter()
                .map(|&t| pool.resolve_fully(t))
                .collect();
            mono_by_generic
                .entry(mono_fn.original_name)
                .or_default()
                .push((resolved_params, mono_fn.mangled_name));
        }
        Self {
            mono_by_id,
            mono_by_generic,
        }
    }

    /// Rewrite the call targets of `func` and every lambda in `lambdas` to the
    /// mangled mono name. Idempotent; no-op for non-generic call sites.
    pub fn rewrite_function(
        &self,
        func: &mut ori_arc::ArcFunction,
        lambdas: &mut [ori_arc::ArcFunction],
        pool: &ori_types::Pool,
    ) {
        rewrite_func_call_targets(func, &self.mono_by_id, &self.mono_by_generic, pool);
        for lambda in lambdas {
            rewrite_func_call_targets(lambda, &self.mono_by_id, &self.mono_by_generic, pool);
        }
    }
}

fn resolve_mono_target(
    callee: Name,
    args: &[ori_arc::ArcVarId],
    mono_instance_id: Option<ori_ir::canon::MonoInstanceId>,
    func: &ori_arc::ArcFunction,
    mono_by_id: &FxHashMap<ori_ir::canon::MonoInstanceId, Name>,
    mono_by_generic: &FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    pool: &ori_types::Pool,
) -> Option<Name> {
    if let Some(id) = mono_instance_id {
        if let Some(&mangled) = mono_by_id.get(&id) {
            return Some(mangled);
        }
    }
    let candidates = mono_by_generic.get(&callee)?;
    let arg_types: Vec<Idx> = args
        .iter()
        .map(|a| pool.resolve_fully(func.var_type(*a)))
        .collect();
    candidates
        .iter()
        .find(|(params, _)| {
            params.len() == arg_types.len()
                && params
                    .iter()
                    .zip(&arg_types)
                    .all(|(p, a)| pool.structural_eq(*p, *a))
        })
        .map(|(_, mangled)| *mangled)
}

fn rewrite_func_call_targets(
    func: &mut ori_arc::ArcFunction,
    mono_by_id: &FxHashMap<ori_ir::canon::MonoInstanceId, Name>,
    mono_by_generic: &FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    pool: &ori_types::Pool,
) {
    use ori_arc::ir::{ArcInstr, ArcTerminator};
    // Snapshot args+mono_id for each Apply/Invoke so the rewrite step does
    // not borrow `func` mutably while reading var_type.
    let updates: Vec<(usize, Option<usize>, Name)> = {
        let mut out = Vec::new();
        for (b_idx, block) in func.blocks.iter().enumerate() {
            for (i_idx, instr) in block.body.iter().enumerate() {
                if let ArcInstr::Apply {
                    func: callee,
                    args,
                    mono_instance_id,
                    ..
                } = instr
                {
                    if let Some(mangled) = resolve_mono_target(
                        *callee,
                        args,
                        *mono_instance_id,
                        func,
                        mono_by_id,
                        mono_by_generic,
                        pool,
                    ) {
                        if mangled != *callee {
                            out.push((b_idx, Some(i_idx), mangled));
                        }
                    }
                }
            }
            if let ArcTerminator::Invoke {
                func: callee,
                args,
                mono_instance_id,
                ..
            } = &block.terminator
            {
                if let Some(mangled) = resolve_mono_target(
                    *callee,
                    args,
                    *mono_instance_id,
                    func,
                    mono_by_id,
                    mono_by_generic,
                    pool,
                ) {
                    if mangled != *callee {
                        out.push((b_idx, None, mangled));
                    }
                }
            }
        }
        out
    };
    for (b_idx, i_idx, mangled) in updates {
        match i_idx {
            Some(i) => {
                if let ArcInstr::Apply { func: callee, .. } = &mut func.blocks[b_idx].body[i] {
                    *callee = mangled;
                }
            }
            None => {
                if let ArcTerminator::Invoke { func: callee, .. } =
                    &mut func.blocks[b_idx].terminator
                {
                    *callee = mangled;
                }
            }
        }
    }
}

/// Lower monomorphized functions to ARC IR and populate `arc_cache` before
/// AIMS interprocedural analysis runs.
///
/// INVARIANT: every reachable mono lands in `arc_cache` before
/// `run_interprocedural_analyses` (PL-5: no-stale-summary). Lowering monos
/// only inside `prepare_mono_cached` (the prior shape) ran them AFTER AIMS
/// had already analyzed an `arc_cache` missing every mono, yielding
/// `CONSERVATIVE` contracts for every generic call site.
///
/// Idempotent: skips entries already present in `arc_cache`.
pub(crate) fn pre_lower_monos_to_arc_cache(
    mono_functions: &[crate::monomorphize::MonoFunction],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &ori_types::Pool,
    arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
) {
    for mono_fn in mono_functions {
        if arc_cache.contains_key(&mono_fn.mangled_name) {
            continue;
        }
        let body = mono_fn.body_root(canon);
        let params: Vec<(Name, Idx)> = mono_fn
            .sig
            .param_names
            .iter()
            .copied()
            .zip(mono_fn.sig.param_types.iter().copied())
            .collect();
        let mut problems = Vec::new();
        let result = lower_function_can(
            mono_fn.mangled_name,
            &params,
            mono_fn.sig.return_type,
            body,
            canon,
            interner,
            pool,
            &mut problems,
            mono_fn.sig.is_fbip,
            Some(&mono_fn.body_type_map),
        );
        for problem in &problems {
            debug!(?problem, "ARC lowering problem (mono pre-pass)");
        }
        arc_cache.insert(mono_fn.mangled_name, result);
    }
}

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Lower all non-generic functions through the ARC pipeline without
    /// emitting LLVM IR.
    ///
    /// Buffers results for two-pass nounwind analysis. Functions are removed
    /// from `arc_cache` (zero-copy move). Functions not in the cache fall back
    /// to inline lowering.
    pub fn prepare_all_cached(
        &mut self,
        module_functions: &[Function],
        function_sigs: &[FunctionSig],
        canon: &CanonResult,
        arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
    ) -> Vec<PreparedFunction> {
        let mut prepared = Vec::new();

        // Build name→sig lookup to avoid positional misalignment between
        // module_functions (source order, multi-clause duplicates) and
        // function_sigs (sorted by Name, deduped).
        let sig_map: rustc_hash::FxHashMap<Name, &FunctionSig> =
            function_sigs.iter().map(|s| (s.name, s)).collect();

        let mut seen = rustc_hash::FxHashSet::default();
        for func in module_functions {
            if !seen.insert(func.name) {
                continue;
            }
            let Some(sig) = sig_map.get(&func.name) else {
                continue;
            };
            if sig.is_generic() {
                continue;
            }

            let Some(&(func_id, ref abi)) = self.codegen_ctx.functions.get(&func.name) else {
                tracing::warn!(
                    name = %self.interner.lookup(func.name),
                    "function not declared — skipping preparation"
                );
                self.builder.record_codegen_error();
                continue;
            };
            let abi = abi.clone();

            let (arc_func, lambdas) = if let Some(cached) = arc_cache.remove(&func.name) {
                cached
            } else {
                // Fallback: lower inline from canonical IR
                let body = canon.root_for(func.name).unwrap_or(canon.root);
                let params: Vec<(Name, Idx)> = abi.params.iter().map(|p| (p.name, p.ty)).collect();
                let mut problems = Vec::new();
                let result = lower_function_can(
                    func.name,
                    &params,
                    abi.return_abi.ty,
                    body,
                    canon,
                    self.interner,
                    self.pool,
                    &mut problems,
                    sig.is_fbip,
                    None,
                );
                for problem in &problems {
                    debug!(?problem, "ARC lowering problem (fallback)");
                }
                result
            };

            match self.prepare_arc_function(func.name, func_id, &abi, arc_func, lambdas) {
                Ok(pf) => prepared.push(pf),
                Err(err) => {
                    // PC-2 contract violation — error already recorded via
                    // `record_codegen_error` inside the hook. Skip this
                    // function's emission; continue the batch.
                    debug!(
                        name = %self.interner.lookup(func.name),
                        ?err,
                        "PC-2 contract violation — skipping function"
                    );
                }
            }
        }

        prepared
    }

    /// Lower all monomorphized functions through the ARC pipeline without
    /// emitting LLVM IR.
    ///
    /// Buffers results for two-pass nounwind analysis. Falls back to inline
    /// lowering with type substitution for functions not found in the cache.
    pub fn prepare_mono_cached(
        &mut self,
        mono_functions: &[crate::monomorphize::MonoFunction],
        canon: &CanonResult,
        arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
    ) -> Vec<PreparedFunction> {
        let mut prepared = Vec::new();

        for mono_fn in mono_functions {
            let Some(&(func_id, ref abi)) = self.codegen_ctx.functions.get(&mono_fn.mangled_name)
            else {
                tracing::warn!(
                    name = %self.interner.lookup(mono_fn.mangled_name),
                    "mono function not declared — skipping preparation"
                );
                self.builder.record_codegen_error();
                continue;
            };
            let abi = abi.clone();

            let (arc_func, lambdas) = if let Some(cached) = arc_cache.remove(&mono_fn.mangled_name)
            {
                cached
            } else {
                // Fallback: lower inline with type substitution
                let body = mono_fn.body_root(canon);
                let params: Vec<(Name, Idx)> = abi.params.iter().map(|p| (p.name, p.ty)).collect();
                let mut problems = Vec::new();
                let result = lower_function_can(
                    mono_fn.mangled_name,
                    &params,
                    abi.return_abi.ty,
                    body,
                    canon,
                    self.interner,
                    self.pool,
                    &mut problems,
                    mono_fn.sig.is_fbip,
                    Some(&mono_fn.body_type_map),
                );
                for problem in &problems {
                    debug!(?problem, "ARC lowering problem (mono fallback)");
                }
                result
            };

            // INVARIANT: every successfully-lowered mono lands in arc_cache before
            // run_interprocedural_analyses (PL-5: no-stale-summary).
            arc_cache.insert(mono_fn.mangled_name, (arc_func.clone(), lambdas.clone()));

            match self.prepare_arc_function(mono_fn.mangled_name, func_id, &abi, arc_func, lambdas)
            {
                Ok(pf) => prepared.push(pf),
                Err(err) => {
                    debug!(
                        name = %self.interner.lookup(mono_fn.mangled_name),
                        ?err,
                        "PC-2 contract violation — skipping mono function"
                    );
                }
            }
        }

        prepared
    }

    /// Process an ARC function through the pipeline without emitting LLVM IR.
    ///
    /// Uses [`Self::process_arc_function`] for shared ARC processing and
    /// [`Self::prepare_lambda`] for lambda preparation. Returns a
    /// [`PreparedFunction`] ready for nounwind analysis and LLVM emission.
    ///
    /// Returns `Err(VerifyError::UnresolvedTypeVar(_))` if the PC-2 contract
    /// check fires on the parent function or on ANY lambda. Filter-out of a
    /// single failing lambda is NOT sound — the parent's `PartialApply` would
    /// then reference a never-emitted lambda name. Parent emission is skipped
    /// on any lambda failure.
    fn prepare_arc_function(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        mut arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) -> Result<PreparedFunction, VerifyError> {
        debug!(
            name = %self.interner.lookup(name),
            "preparing function (ARC pipeline, no emit)"
        );

        // Resolve BoundVar types in polymorphic lambdas before preparation.
        let mut lambdas = lambdas;
        crate::codegen::function_compiler::lambda_mono::resolve_all_lambda_bound_vars(
            &mut arc_func,
            &mut lambdas,
            self.pool,
            self.interner,
            self.arc_classifier as &dyn ori_arc::ArcClassification,
        );

        // Prepare lambdas: declare + ARC pipeline (no LLVM emission).
        // declare_and_process_lambda renames each lambda to a globally unique
        // name. We collect the (old → new) mapping so we can update the
        // parent function's PartialApply references.
        //
        // A failing lambda fails the WHOLE parent: filter-out would leave a
        // dangling PartialApply callee (plan Hook 2 cascade note).
        let mut lambda_renames: Vec<(Name, Name)> = Vec::new();
        let mut prepared_lambdas: Vec<PreparedLambda> = Vec::with_capacity(lambdas.len());
        for lambda in lambdas {
            let original_name = lambda.name;
            let prepared = self.prepare_lambda(lambda)?;
            if prepared.name != original_name {
                lambda_renames.push((original_name, prepared.name));
            }
            prepared_lambdas.push(prepared);
        }

        // Remap PartialApply callee references in the parent function to use
        // the globally unique lambda names assigned during preparation.
        if !lambda_renames.is_empty() {
            crate::codegen::function_compiler::purity_analysis::remap_partial_apply_names(
                &mut arc_func,
                &lambda_renames,
            );
        }

        // Shared ARC processing: borrow annotations → arg ownership → pipeline
        self.process_arc_function(name, &mut arc_func)?;

        tracing::trace!(
            name = %self.interner.lookup(name),
            blocks = arc_func.blocks.len(),
            "ARC pipeline complete (prepared, not emitted)"
        );

        Ok(PreparedFunction {
            name,
            func_id,
            abi: abi.clone(),
            arc_func,
            lambdas: prepared_lambdas,
        })
    }

    /// Prepare a lambda through the ARC pipeline without emitting LLVM IR.
    ///
    /// Uses [`Self::declare_and_process_lambda`] for shared setup + ARC
    /// processing. The actual LLVM body emission is deferred to
    /// [`Self::emit_prepared_functions`].
    fn prepare_lambda(
        &mut self,
        mut lambda: ori_arc::ArcFunction,
    ) -> Result<PreparedLambda, VerifyError> {
        let (name, func_id, abi) = self.declare_and_process_lambda(&mut lambda)?;
        Ok(PreparedLambda {
            name,
            func_id,
            abi,
            arc_func: lambda,
        })
    }
}
