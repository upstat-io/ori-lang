//! Preparation phase: lower functions through ARC pipeline without emitting LLVM IR.

use ori_arc::lower_function_can;
use ori_ir::canon::CanonResult;
use ori_ir::{Function, Name};
use ori_types::{FunctionSig, Idx};
use rustc_hash::FxHashMap;
use tracing::debug;

use super::types::{PreparedFunction, PreparedLambda};
use crate::codegen::abi::FunctionAbi;
use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::FunctionId;

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

        for (func, sig) in module_functions.iter().zip(function_sigs.iter()) {
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

            prepared.push(self.prepare_arc_function(func.name, func_id, &abi, arc_func, lambdas));
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
                let body = canon.root_for(mono_fn.original_name).unwrap_or(canon.root);
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

            prepared.push(self.prepare_arc_function(
                mono_fn.mangled_name,
                func_id,
                &abi,
                arc_func,
                lambdas,
            ));
        }

        prepared
    }

    /// Process an ARC function through the pipeline without emitting LLVM IR.
    ///
    /// Uses [`Self::process_arc_function`] for shared ARC processing and
    /// [`Self::prepare_lambda`] for lambda preparation. Returns a
    /// [`PreparedFunction`] ready for nounwind analysis and LLVM emission.
    fn prepare_arc_function(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        mut arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) -> PreparedFunction {
        debug!(
            name = %self.interner.lookup(name),
            "preparing function (ARC pipeline, no emit)"
        );

        // Resolve BoundVar types in polymorphic lambdas before preparation.
        let mut lambdas = lambdas;
        crate::codegen::function_compiler::define_phase::resolve_all_lambda_bound_vars(
            &mut arc_func,
            &mut lambdas,
            self.pool,
            self.interner,
        );

        // Prepare lambdas: declare + ARC pipeline (no LLVM emission).
        // declare_and_process_lambda renames each lambda to a globally unique
        // name. We collect the (old → new) mapping so we can update the
        // parent function's PartialApply references.
        let mut lambda_renames: Vec<(Name, Name)> = Vec::new();
        let prepared_lambdas: Vec<PreparedLambda> = lambdas
            .into_iter()
            .map(|lambda| {
                let original_name = lambda.name;
                let prepared = self.prepare_lambda(lambda);
                if prepared.name != original_name {
                    lambda_renames.push((original_name, prepared.name));
                }
                prepared
            })
            .collect();

        // Remap PartialApply callee references in the parent function to use
        // the globally unique lambda names assigned during preparation.
        if !lambda_renames.is_empty() {
            crate::codegen::function_compiler::purity_analysis::remap_partial_apply_names(
                &mut arc_func,
                &lambda_renames,
            );
        }

        // Shared ARC processing: borrow annotations → arg ownership → pipeline
        self.process_arc_function(name, &mut arc_func);

        tracing::trace!(
            name = %self.interner.lookup(name),
            blocks = arc_func.blocks.len(),
            "ARC pipeline complete (prepared, not emitted)"
        );

        PreparedFunction {
            name,
            func_id,
            abi: abi.clone(),
            arc_func,
            lambdas: prepared_lambdas,
        }
    }

    /// Prepare a lambda through the ARC pipeline without emitting LLVM IR.
    ///
    /// Uses [`Self::declare_and_process_lambda`] for shared setup + ARC
    /// processing. The actual LLVM body emission is deferred to
    /// [`Self::emit_prepared_functions`].
    fn prepare_lambda(&mut self, mut lambda: ori_arc::ArcFunction) -> PreparedLambda {
        let (name, func_id, abi) = self.declare_and_process_lambda(&mut lambda);
        PreparedLambda {
            name,
            func_id,
            abi,
            arc_func: lambda,
        }
    }
}
