//! Preparation of validated artifact functions without emitting LLVM IR.

use ori_arc::verify::VerifyError;
use ori_ir::{Function, Name};
use ori_types::FunctionSig;
use tracing::debug;

use super::types::{PreparedFunction, PreparedLambda};
use crate::codegen::abi::FunctionAbi;
use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::FunctionId;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Prepare every non-generic source function from its exact artifact family.
    ///
    /// Buffers results for two-pass nounwind analysis. Missing bodies fail
    /// closed through the artifact projection seam; canonical IR is not a
    /// physical-backend fallback.
    pub fn prepare_all_from_artifact(
        &mut self,
        module_functions: &[Function],
        function_sigs: &[FunctionSig],
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

            let Some((arc_func, lambdas)) = self.clone_bound_family(func.name, &abi) else {
                continue;
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

    /// Prepare every monomorphized function from its exact artifact family.
    ///
    /// There is no canonical-body or type-substitution fallback after the
    /// executable artifact has closed.
    pub fn prepare_mono_from_artifact(
        &mut self,
        mono_functions: &[ori_repr::monomorphize::MonoFunction],
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

            let Some((arc_func, lambdas)) = self.clone_bound_family(mono_fn.mangled_name, &abi)
            else {
                continue;
            };

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

    /// Prepare the exact artifact parents returned by
    /// [`Self::declare_artifact_remainder`].
    ///
    /// The declaration pass owns inventory selection; preparation consumes the
    /// frozen list verbatim so the two physical phases cannot disagree about
    /// which compiler-generated bodies exist.
    pub fn prepare_artifact_remainder_from_artifact(
        &mut self,
        functions: &[ori_repr::executable::FunctionId],
    ) -> Vec<PreparedFunction> {
        let mut prepared = Vec::with_capacity(functions.len());
        for &function in functions {
            let Some(program) = self.executable_program else {
                self.builder.record_codegen_error_with_msg(
                    "LLVM artifact-family preparation requires a closed executable program",
                );
                break;
            };
            let name = program.function(function).name;
            if program.function_family_lambdas(function).is_none() {
                self.builder.record_codegen_error_with_msg(format!(
                    "closed executable artifact remainder {} is a nested lambda, not a family parent",
                    self.interner.lookup(name)
                ));
                continue;
            }
            let Some(&(func_id, ref abi)) = self.codegen_ctx.functions.get(&name) else {
                self.builder.record_codegen_error_with_msg(format!(
                    "closed executable artifact remainder {} was not declared before preparation",
                    self.interner.lookup(name)
                ));
                continue;
            };
            let abi = abi.clone();
            let Some((arc_func, lambdas)) = self.clone_bound_family(name, &abi) else {
                continue;
            };
            match self.prepare_arc_function(name, func_id, &abi, arc_func, lambdas) {
                Ok(function) => prepared.push(function),
                Err(error) => {
                    debug!(
                        name = %self.interner.lookup(name),
                        ?error,
                        "artifact remainder violated the physical projection contract"
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

        let lambdas = lambdas;

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
            crate::codegen::function_compiler::lambda_rewrite::remap_partial_apply_names(
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
