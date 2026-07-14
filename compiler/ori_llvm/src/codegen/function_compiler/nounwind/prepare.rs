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
    mono_functions: &[ori_repr::monomorphize::MonoFunction],
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
        mono_functions: &[ori_repr::monomorphize::MonoFunction],
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
