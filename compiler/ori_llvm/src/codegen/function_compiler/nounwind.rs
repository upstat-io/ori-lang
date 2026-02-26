//! Two-pass nounwind analysis: prepare → analyze → emit.
//!
//! Sound nounwind analysis requires seeing all functions before emitting any
//! LLVM IR. The pipeline:
//! 1. **Prepare**: Lower all functions through the ARC pipeline, buffering
//!    the results as [`PreparedFunction`] / [`PreparedLambda`].
//! 2. **Analyze**: Fixed-point iteration over all prepared functions to build
//!    the complete nounwind set ([`FunctionCompiler::compute_nounwind_set`]).
//! 3. **Emit**: Emit LLVM IR using the complete nounwind set, ensuring
//!    callers of nounwind callees use `call` instead of `invoke`.
//!
//! # Known limitation
//!
//! Impl methods are compiled via the old immediate-emit path
//! ([`FunctionCompiler::emit_arc_function`]) **before** the two-pass analysis
//! runs. This means impl methods calling monomorphized generic functions will
//! use `invoke` instead of `call`, even if the callee is trivially nounwind.
//! This is safe (using `invoke` is always correct) but generates unnecessary
//! overhead. A future refactor could fold impl methods into the two-pass batch.

use ori_arc::lower_function_can;
use ori_ir::canon::CanonResult;
use ori_ir::{Function, Name};
use ori_types::{FunctionSig, Idx};
use rustc_hash::FxHashMap;
use tracing::{debug, trace};

use super::FunctionCompiler;
use crate::codegen::abi::FunctionAbi;
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::value_id::FunctionId;

// ---------------------------------------------------------------------------
// Prepared function types
// ---------------------------------------------------------------------------

/// A function fully processed through the ARC pipeline, ready for nounwind
/// analysis and LLVM emission.
///
/// Created by [`FunctionCompiler::prepare_all_cached`] or
/// [`FunctionCompiler::prepare_mono_cached`]. Enables two-pass compilation:
/// 1. Lower all functions to ARC IR (populate this buffer)
/// 2. Analyze nounwind on the complete set ([`FunctionCompiler::compute_nounwind_set`])
/// 3. Emit LLVM IR using the complete nounwind set ([`FunctionCompiler::emit_prepared_functions`])
///
/// This ensures monomorphized callee nounwind status is available when
/// analyzing callers, preventing unnecessary `invoke` + landing pad overhead.
pub struct PreparedFunction {
    pub(super) name: Name,
    pub(super) func_id: FunctionId,
    pub(super) abi: FunctionAbi,
    pub(super) arc_func: ori_arc::ArcFunction,
    pub(super) lambdas: Vec<PreparedLambda>,
}

/// A lambda processed through the ARC pipeline, ready for LLVM emission.
///
/// The lambda's LLVM function is already declared and registered in
/// `CodegenContext::functions` during preparation — only the body emission
/// is deferred to [`FunctionCompiler::emit_prepared_functions`].
pub(super) struct PreparedLambda {
    pub(super) name: Name,
    pub(super) func_id: FunctionId,
    pub(super) abi: FunctionAbi,
    pub(super) arc_func: ori_arc::ArcFunction,
}

// ---------------------------------------------------------------------------
// Prepare phase
// ---------------------------------------------------------------------------

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

        // Prepare lambdas: declare + ARC pipeline (no LLVM emission)
        let prepared_lambdas: Vec<PreparedLambda> = lambdas
            .into_iter()
            .map(|lambda| self.prepare_lambda(lambda))
            .collect();

        // Shared ARC processing: borrow annotations → arg ownership → pipeline
        self.process_arc_function(name, &mut arc_func);

        trace!(
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

    // -----------------------------------------------------------------------
    // Analyze phase
    // -----------------------------------------------------------------------

    /// Build the complete nounwind function set from all prepared functions.
    ///
    /// Uses fixed-point iteration: each pass analyzes all functions and their
    /// lambdas, adding newly-proven-nounwind functions to the set. Iteration
    /// continues until no new functions are added. This correctly handles
    /// call chains like A→B→C where C must be proven nounwind before B,
    /// and B before A.
    ///
    /// Must be called after all `prepare_*` methods and before
    /// [`Self::emit_prepared_functions`].
    pub fn compute_nounwind_set(&mut self, prepared: &[PreparedFunction]) {
        debug!(
            functions = prepared.len(),
            "computing complete nounwind set (fixed-point)"
        );

        let mut pass = 0u32;
        loop {
            let mut changed = false;

            for func in prepared {
                // Check lambdas first (they may be callees of the parent)
                for lambda in &func.lambdas {
                    if !self.codegen_ctx.nounwind_functions.contains(&lambda.name)
                        && self.is_arc_function_nounwind(&lambda.arc_func)
                    {
                        self.codegen_ctx.nounwind_functions.insert(lambda.name);
                        changed = true;
                    }
                }

                // Check parent function
                if !self.codegen_ctx.nounwind_functions.contains(&func.name)
                    && self.is_arc_function_nounwind(&func.arc_func)
                {
                    self.codegen_ctx.nounwind_functions.insert(func.name);
                    changed = true;
                }
            }

            pass = pass.saturating_add(1);
            if !changed {
                break;
            }
        }

        // Propagate nounwind from mangled monomorphized names to their original
        // generic names. ARC IR `Invoke` terminators use the original name
        // (e.g., `"identity"`), while `nounwind_functions` contains mangled names
        // (e.g., `"identity$m$int"`). If ALL specializations of a generic are
        // nounwind, the original name is also safe to call without landing pads.
        let mut mono_propagated = 0u32;
        for (original_name, specializations) in &self.codegen_ctx.mono_dispatch {
            if self.codegen_ctx.nounwind_functions.contains(original_name) {
                continue; // Already marked (e.g., non-generic with same name)
            }
            let all_nounwind = specializations
                .iter()
                .all(|(_, mangled)| self.codegen_ctx.nounwind_functions.contains(mangled));
            if all_nounwind && !specializations.is_empty() {
                self.codegen_ctx.nounwind_functions.insert(*original_name);
                mono_propagated = mono_propagated.saturating_add(1);
            }
        }

        debug!(
            passes = pass,
            nounwind_count = self.codegen_ctx.nounwind_functions.len(),
            mono_propagated,
            "nounwind analysis complete"
        );
    }

    // -----------------------------------------------------------------------
    // Emit phase
    // -----------------------------------------------------------------------

    /// Emit LLVM IR for all prepared functions using the complete nounwind set.
    ///
    /// Must be called after [`Self::compute_nounwind_set`]. For each function,
    /// emits lambdas first (so `emit_partial_apply` can reference them), then
    /// the parent function, and applies nounwind attributes from the
    /// pre-computed set.
    pub fn emit_prepared_functions(&mut self, prepared: Vec<PreparedFunction>) {
        debug!(
            count = prepared.len(),
            "emitting prepared functions to LLVM IR"
        );

        for func in prepared {
            self.enter_debug_scope(func.func_id);
            self.builder.set_current_function(func.func_id);

            // Emit lambdas first
            for lambda in &func.lambdas {
                self.emit_prepared_lambda(lambda);
            }

            // Emit parent function
            let mut emitter = ArcIrEmitter::new(
                self.builder,
                self.type_info,
                self.type_resolver,
                self.interner,
                self.pool,
                self.arc_classifier as &dyn ori_arc::ArcClassification,
                func.func_id,
                &self.codegen_ctx,
            );
            emitter.emit_function(&func.arc_func, &func.abi);

            if self.codegen_ctx.nounwind_functions.contains(&func.name) {
                self.builder.add_nounwind_attribute(func.func_id);
                debug!(
                    name = %self.interner.lookup(func.name),
                    "marked nounwind"
                );
            }

            self.exit_debug_scope();
        }
    }

    /// Emit LLVM IR for a prepared lambda using the pre-computed nounwind set.
    fn emit_prepared_lambda(&mut self, lambda: &PreparedLambda) {
        self.builder.set_current_function(lambda.func_id);
        let mut emitter = ArcIrEmitter::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            self.interner,
            self.pool,
            self.arc_classifier as &dyn ori_arc::ArcClassification,
            lambda.func_id,
            &self.codegen_ctx,
        );
        emitter.emit_function(&lambda.arc_func, &lambda.abi);

        if self.codegen_ctx.nounwind_functions.contains(&lambda.name) {
            self.builder.add_nounwind_attribute(lambda.func_id);
        }
    }
}
