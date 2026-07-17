//! Function definition (second pass) -- generates LLVM IR for function bodies.
//!
//! Implements Phase 2 of the two-pass compilation: walk all functions again,
//! lower through the ARC pipeline (`CanExpr` -> ARC IR -> `ArcIrEmitter` -> LLVM IR).
//! Also handles monomorphized function declaration, lambda compilation,
//! and shared ARC processing helpers.

use ori_arc::lower_function_can;
use ori_arc::verify::VerifyError;
use ori_ir::canon::{CanId, CanonResult};
use ori_ir::{Name, Span};
use ori_types::Idx;
use rustc_hash::FxHashMap;
use tracing::{debug, trace};

use super::FunctionCompiler;
use crate::codegen::abi::{compute_function_abi_from_shape, CallConvSite, FunctionAbi};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::value_id::FunctionId;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Dump a realized `ArcFunction`'s ARC IR to stderr when `ORI_DUMP_AFTER_ARC`
    /// is set. Covers every JIT/test-compile emit path (immediate-emit impl
    /// methods + prepared ordinary functions/lambdas); the AOT-only `oric`
    /// finalize dump never reaches `compile_module_with_tests`.
    pub(in crate::codegen::function_compiler) fn dump_arc_if_requested(
        &self,
        arc_func: &ori_arc::ArcFunction,
        label: &str,
    ) {
        if std::env::var_os("ORI_DUMP_AFTER_ARC").is_some() {
            eprintln!(
                "=== ARC IR ({label}) ===\n{}",
                ori_arc::ir::format::format_function(arc_func, self.pool, self.interner)
            );
        }
    }

    // Monomorphized function support

    /// Declare monomorphized functions (phase 1).
    ///
    /// Each `MonoFunction` has a concrete (non-generic) `FunctionSig`, so the
    /// existing `declare_function` infrastructure works unchanged.
    pub fn declare_mono_functions(
        &mut self,
        mono_functions: &[ori_repr::monomorphize::MonoFunction],
    ) {
        for mono_fn in mono_functions {
            self.declare_function(mono_fn.mangled_name, &mono_fn.sig, Span::DUMMY);

            // Build mono dispatch index: original_name -> [(param_types, mangled_name)]
            // (legacy arg-type-matching fallback for `mono_instance_id: None`)
            self.codegen_ctx
                .mono_dispatch
                .entry(mono_fn.identity.original_name())
                .or_default()
                .push((mono_fn.sig.param_types.clone(), mono_fn.mangled_name));

            // Build abstract-index dispatch map: every contributing
            // `MonoInstanceId` points at this function's mangled name.
            // Multiple ids may map to the same mangled name when distinct
            // instances dedup to one specialization (sub-step 1e).
            for &instance_id in mono_fn.identity.instance_ids() {
                self.codegen_ctx
                    .mono_dispatch_by_id
                    .insert(instance_id, mono_fn.mangled_name);
            }
        }
    }

    // Phase 2: Define

    /// Define a single function body via the ARC codegen pipeline.
    ///
    /// Runs: lower -> borrow annotate -> ARC pipeline -> `ArcIrEmitter`.
    ///
    /// Returns `Err(VerifyError::UnresolvedTypeVar(_))` when the PC-2 contract
    /// check at `process_arc_function` / `declare_and_process_lambda` fires,
    /// short-circuiting downstream emission for this function.
    pub(super) fn define_function_body(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: CanId,
        canon: &CanonResult,
        is_fbip: bool,
    ) -> Result<(), VerifyError> {
        self.define_function_body_arc_with_subst(name, func_id, abi, body, canon, is_fbip, None)
    }

    /// ARC IR -> LLVM IR codegen (with RC lifecycle).
    ///
    /// Runs the full ARC pipeline: lower -> liveness -> RC insert -> detect/expand
    /// reuse -> RC eliminate -> `ArcIrEmitter`. The emitter handles block creation,
    /// parameter binding, and return emission internally.
    ///
    /// When `type_subst` is `Some`, expression types from the canonical IR are
    /// substituted before ARC lowering -- used for monomorphized generic functions.
    fn define_function_body_arc_with_subst(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: CanId,
        canon: &CanonResult,
        is_fbip: bool,
        type_subst: Option<&FxHashMap<Idx, Idx>>,
    ) -> Result<(), VerifyError> {
        let name_str = self.interner.lookup(name);
        debug!(name = name_str, tier = 2, "defining function body (ARC)");

        self.enter_debug_scope(func_id);
        self.builder.set_current_function(func_id);

        // Build parameter list for ARC IR lowering: (Name, Idx) pairs
        let params: Vec<(Name, Idx)> = abi.params.iter().map(|p| (p.name, p.ty)).collect();
        let return_type = abi.return_abi.ty;

        // Step 1: Lower canonical IR -> ARC IR
        let mut problems = Vec::new();
        let (arc_func, lambdas) = lower_function_can(
            name,
            &params,
            return_type,
            body,
            canon,
            self.interner,
            self.pool,
            &mut problems,
            is_fbip,
            type_subst,
        );

        for problem in &problems {
            debug!(?problem, "ARC lowering problem");
        }

        self.emit_arc_function(name, func_id, abi, arc_func, lambdas)
    }

    /// Shared post-lowering pipeline: apply borrows -> compile lambdas ->
    /// annotate arg ownership -> ARC pipeline -> emit LLVM IR.
    ///
    /// Called by `define_function_body_arc_with_subst` (after inline lowering)
    /// and `compile_tests` (for test wrappers). The caller is responsible for
    /// `enter_debug_scope` / `set_current_function`.
    pub(super) fn emit_arc_function(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) -> Result<(), VerifyError> {
        // All early-return paths must `exit_debug_scope()` — the enclosing
        // caller (`define_function_body_arc_with_subst`) entered the scope
        // and relies on this function to exit it.
        let result = self.emit_arc_function_inner(name, func_id, abi, arc_func, lambdas);
        self.exit_debug_scope();
        result
    }

    /// Inner helper for [`Self::emit_arc_function`] — omitted `exit_debug_scope`
    /// so the outer function can run it on both Ok and Err paths.
    fn emit_arc_function_inner(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        mut arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) -> Result<(), VerifyError> {
        // Why: ORI_DUMP_AFTER_ARC's AOT surface (oric finalize) never reaches
        // `compile_module_with_tests`, so the realized ArcFunction fed to LLVM
        // on the JIT/test path was undumpable. This fills the gap — decision-tree
        // / RC mis-lowering is now inspectable on the test path too.
        self.dump_arc_if_requested(&arc_func, "emit path");
        // Prepared lambda bodies on this path carry their own decision-tree / RC
        // lowering.
        for lambda in &lambdas {
            self.dump_arc_if_requested(lambda, "emit path, lambda");
        }
        // Compile lambda ArcFunctions (closures).
        // Each lambda is compiled as a separate LLVM function, registered in
        // self.codegen_ctx.functions so that emit_partial_apply can look it up by Name.
        //
        // declare_and_process_lambda renames each lambda to a globally unique
        // name. We collect the (old → new) mapping so we can update the
        // parent function's PartialApply references.
        let mut lambda_renames: Vec<(Name, Name)> = Vec::new();
        for mut lambda in lambdas {
            let original_name = lambda.name;
            self.compile_lambda_arc(&mut lambda)?;
            // After compile_lambda_arc, lambda.name is the globally unique name
            if lambda.name != original_name {
                lambda_renames.push((original_name, lambda.name));
            }
        }

        // Remap PartialApply callee references in the parent function to use
        // the globally unique lambda names assigned during compilation.
        if !lambda_renames.is_empty() {
            super::lambda_rewrite::remap_partial_apply_names(&mut arc_func, &lambda_renames);
        }

        // Lambda compilation changes builder.current_function to the last
        // lambda's FunctionId. Reset it to the parent so entry-block allocas
        // (sret temporaries, indirect param storage) land in the right function.
        self.builder.set_current_function(func_id);

        // Shared ARC processing: borrow annotations -> arg ownership -> pipeline
        self.process_arc_function(name, &mut arc_func)?;

        let name_str = self.interner.lookup(name);
        let is_nounwind = self.is_arc_function_nounwind(&arc_func);

        trace!(
            name = name_str,
            blocks = arc_func.blocks.len(),
            is_nounwind,
            "ARC pipeline complete"
        );

        // Emit LLVM IR from ARC IR
        let mut emitter = ArcIrEmitter::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            self.interner,
            self.pool,
            self.arc_classifier as &dyn ori_arc::ArcClassification,
            func_id,
            &self.codegen_ctx,
            self.debug_context,
        );
        emitter.set_verify_arc(self.verify_arc);
        emitter.set_func_contract(self.aims_contracts.get(&arc_func.name));
        emitter.emit_function(&arc_func, abi);

        // Post-emission CFG simplification: eliminate empty blocks and
        // redundant branches created by if/else and overflow check lowering.
        let fn_val = self.builder.get_function_value(func_id);
        let cfg_stats = crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);
        if cfg_stats.blocks_removed > 0 || cfg_stats.branches_simplified > 0 {
            debug!(
                name = name_str,
                blocks_removed = cfg_stats.blocks_removed,
                branches_simplified = cfg_stats.branches_simplified,
                "cfg_simplify"
            );
        }

        // Function-level LLVM IR verification.
        if self.verify_arc && !fn_val.verify(true) {
            tracing::error!(
                name = name_str,
                "LLVM IR verification failed after codegen (emit_arc_function)"
            );
            self.builder.record_codegen_error();
        }

        // Mark nounwind after emission so LLVM's PruneEH pass can
        // optimize callers (even those compiled before this function).
        if is_nounwind {
            self.codegen_ctx.nounwind_functions.insert(name);
            self.builder.add_nounwind_attribute(func_id);
            debug!(name = name_str, "marked nounwind");
        }

        Ok(())
    }

    /// Compile a lambda `ArcFunction` as a standalone LLVM function.
    ///
    /// The lambda takes `(captures..., user_params...)` as a flat parameter list.
    /// A wrapper function bridging `(env_ptr, user_params...)` -> flat call is
    /// generated later by `emit_partial_apply` in the ARC emitter.
    ///
    /// Registers the lambda in `self.codegen_ctx.functions` so the emitter can look it up.
    fn compile_lambda_arc(&mut self, lambda: &mut ori_arc::ArcFunction) -> Result<(), VerifyError> {
        // PC-2 + BoundVar invariant checks run inside
        // `declare_and_process_lambda` (the shared primary seam for both this
        // immediate-emit path and the two-pass `prepare_lambda` path) — no
        // duplicate check needed here.

        // Shared setup: declare, register, run ARC pipeline.
        // On PC-2 violation, return before ArcIrEmitter consumes the body.
        let (lambda_name, func_id, abi) = self.declare_and_process_lambda(lambda)?;

        let is_nounwind = self.is_arc_function_nounwind(lambda);

        // Emit LLVM IR from the lambda's ARC IR
        self.builder.set_current_function(func_id);
        let mut emitter = ArcIrEmitter::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            self.interner,
            self.pool,
            self.arc_classifier as &dyn ori_arc::ArcClassification,
            func_id,
            &self.codegen_ctx,
            self.debug_context,
        );
        emitter.set_verify_arc(self.verify_arc);
        emitter.set_func_contract(self.aims_contracts.get(&lambda.name));
        emitter.emit_function(lambda, &abi);

        // Post-emission CFG simplification
        let fn_val = self.builder.get_function_value(func_id);
        crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);

        // Function-level LLVM IR verification.
        if self.verify_arc && !fn_val.verify(true) {
            tracing::error!(
                name = %self.interner.lookup(lambda_name),
                "LLVM IR verification failed after codegen (compile_lambda_arc)"
            );
            self.builder.record_codegen_error();
        }

        if is_nounwind {
            self.codegen_ctx.nounwind_functions.insert(lambda_name);
            self.builder.add_nounwind_attribute(func_id);
        }

        Ok(())
    }

    /// Compute a `FunctionAbi` from an `ArcFunction`'s parameter and return types.
    ///
    /// Used for lambda functions where no `FunctionSig` exists.
    pub(super) fn compute_arc_function_abi(&self, func: &ori_arc::ArcFunction) -> FunctionAbi {
        let annotated = self.annotated_sigs.get(&func.name);
        debug_assert!(annotated.is_none_or(|signature| {
            signature.return_type == func.return_type
                && signature.params.len() == func.params.len()
                && signature
                    .params
                    .iter()
                    .zip(&func.params)
                    .all(|(fact, parameter)| fact.ty == parameter.ty)
        }));
        let params = func.params.iter().enumerate().map(|(index, parameter)| {
            (
                self.interner.intern(&format!("v{}", parameter.var.raw())),
                parameter.ty,
                annotated
                    .and_then(|signature| signature.params.get(index))
                    .map_or(ori_arc::Ownership::Owned, |fact| fact.ownership),
            )
        });
        compute_function_abi_from_shape(
            params,
            func.return_type,
            CallConvSite::OriFunction,
            self.type_info,
            annotated.map(|_| self.arc_classifier as &dyn ori_arc::ArcClassification),
            self.repr_plan(),
        )
    }

    // `process_arc_function`, `report_primary_seam_violation`,
    // `process_arc_function`, and `declare_and_process_lambda` live
    // in the sibling [`super::shared_seam`] module — the shared primary
    // seam between the immediate-emit path (this file) and the two-pass
    // prepare path (`nounwind/prepare.rs`). See `shared_seam.rs` for
    // the rationale.
}
