//! Function definition (second pass) -- generates LLVM IR for function bodies.
//!
//! Implements Phase 2 of the two-pass compilation: walk all functions again,
//! lower through the ARC pipeline (`CanExpr` -> ARC IR -> `ArcIrEmitter` -> LLVM IR).
//! Also handles monomorphized function declaration, lambda compilation,
//! and shared ARC processing helpers.

use ori_arc::lower_function_can;
use ori_ir::canon::{CanId, CanonResult};
use ori_ir::{Name, Span};
use ori_types::Idx;
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use super::FunctionCompiler;
use crate::codegen::abi::{
    compute_param_passing, compute_return_passing, CallConv, FunctionAbi, ParamAbi, ReturnAbi,
};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::value_id::FunctionId;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    // Monomorphized function support

    /// Declare monomorphized functions (phase 1).
    ///
    /// Each `MonoFunction` has a concrete (non-generic) `FunctionSig`, so the
    /// existing `declare_function` infrastructure works unchanged.
    pub fn declare_mono_functions(&mut self, mono_functions: &[crate::monomorphize::MonoFunction]) {
        for mono_fn in mono_functions {
            self.declare_function(mono_fn.mangled_name, &mono_fn.sig, Span::DUMMY);

            // Build mono dispatch index: original_name -> [(param_types, mangled_name)]
            self.codegen_ctx
                .mono_dispatch
                .entry(mono_fn.original_name)
                .or_default()
                .push((mono_fn.sig.param_types.clone(), mono_fn.mangled_name));
        }
    }

    // Phase 2: Define

    /// Define a single function body via the ARC codegen pipeline.
    ///
    /// Runs: lower -> borrow annotate -> ARC pipeline -> `ArcIrEmitter`.
    pub(super) fn define_function_body(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: CanId,
        canon: &CanonResult,
        is_fbip: bool,
    ) {
        self.define_function_body_arc_with_subst(name, func_id, abi, body, canon, is_fbip, None);
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
    ) {
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

        self.emit_arc_function(name, func_id, abi, arc_func, lambdas);
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
        mut arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) {
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
            self.compile_lambda_arc(&mut lambda);
            // After compile_lambda_arc, lambda.name is the globally unique name
            if lambda.name != original_name {
                lambda_renames.push((original_name, lambda.name));
            }
        }

        // Remap PartialApply callee references in the parent function to use
        // the globally unique lambda names assigned during compilation.
        if !lambda_renames.is_empty() {
            remap_partial_apply_names(&mut arc_func, &lambda_renames);
        }

        // Lambda compilation changes builder.current_function to the last
        // lambda's FunctionId. Reset it to the parent so entry-block allocas
        // (sret temporaries, indirect param storage) land in the right function.
        self.builder.set_current_function(func_id);

        // Shared ARC processing: borrow annotations -> arg ownership -> pipeline
        self.process_arc_function(name, &mut arc_func);

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
        );
        emitter.emit_function(&arc_func, abi);

        // Mark nounwind after emission so LLVM's PruneEH pass can
        // optimize callers (even those compiled before this function).
        if is_nounwind {
            self.codegen_ctx.nounwind_functions.insert(name);
            self.builder.add_nounwind_attribute(func_id);
            debug!(name = name_str, "marked nounwind");
        }

        self.exit_debug_scope();
    }

    /// Compile a lambda `ArcFunction` as a standalone LLVM function.
    ///
    /// The lambda takes `(captures..., user_params...)` as a flat parameter list.
    /// A wrapper function bridging `(env_ptr, user_params...)` -> flat call is
    /// generated later by `emit_partial_apply` in the ARC emitter.
    ///
    /// Registers the lambda in `self.codegen_ctx.functions` so the emitter can look it up.
    fn compile_lambda_arc(&mut self, lambda: &mut ori_arc::ArcFunction) {
        // Shared setup: declare, register, run ARC pipeline
        let (lambda_name, func_id, abi) = self.declare_and_process_lambda(lambda);

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
        );
        emitter.emit_function(lambda, &abi);

        if is_nounwind {
            self.codegen_ctx.nounwind_functions.insert(lambda_name);
            self.builder.add_nounwind_attribute(func_id);
        }
    }

    /// Compute a `FunctionAbi` from an `ArcFunction`'s parameter and return types.
    ///
    /// Used for lambda functions where no `FunctionSig` exists.
    pub(super) fn compute_arc_function_abi(&self, func: &ori_arc::ArcFunction) -> FunctionAbi {
        let params: Vec<ParamAbi> = func
            .params
            .iter()
            .map(|p| ParamAbi {
                name: self.interner.intern(&format!("v{}", p.var.raw())),
                ty: p.ty,
                passing: compute_param_passing(p.ty, self.type_info),
            })
            .collect();

        let return_abi = ReturnAbi {
            ty: func.return_type,
            passing: compute_return_passing(func.return_type, self.type_info),
        };

        FunctionAbi {
            params,
            return_abi,
            call_conv: CallConv::Fast,
        }
    }

    // Shared ARC processing helpers

    /// Apply borrow annotations, annotate arg ownership, and run the ARC
    /// pipeline on a function.
    ///
    /// Shared by both the immediate-emit path ([`Self::emit_arc_function`]) and
    /// the two-pass prepare path ([`Self::prepare_arc_function`]).
    pub(super) fn process_arc_function(&mut self, name: Name, arc_func: &mut ori_arc::ArcFunction) {
        // Apply borrow inference annotations to ARC IR params.
        // Lowering defaults all params to Ownership::Owned (lower/mod.rs).
        // Without this, RC insertion generates unnecessary RcInc/RcDec for
        // params that borrow inference determined should be Borrowed.
        if let Some(sig) = self.annotated_sigs.get(&name) {
            for (param, annotated) in arc_func.params.iter_mut().zip(&sig.params) {
                param.ownership = annotated.ownership;
            }
        } else if !arc_func.params.is_empty() {
            let name_str = self.interner.lookup(name);
            warn!(
                func = name_str,
                params = arc_func.params.len(),
                "borrow signature missing — compiling with all-Owned params"
            );
        }

        ori_arc::annotate_arg_ownership(
            arc_func,
            self.annotated_sigs,
            self.interner,
            &self.builtin_ownership,
            self.pool,
        );
        let arc_problems = ori_arc::run_arc_pipeline(
            arc_func,
            self.arc_classifier,
            self.annotated_sigs,
            self.pool,
            self.interner,
            &self.uniqueness_summaries,
        );
        for problem in &arc_problems {
            debug!(?problem, "ARC pipeline problem");
        }
    }

    /// Declare a lambda LLVM function, register it in `codegen_ctx`, and run
    /// the ARC pipeline.
    ///
    /// Shared by both the immediate-emit path ([`Self::compile_lambda_arc`]) and
    /// the two-pass prepare path ([`Self::prepare_lambda`]).
    ///
    /// Returns `(lambda_name, func_id, abi)` for the caller to either emit
    /// LLVM IR immediately or buffer as a [`PreparedLambda`].
    ///
    /// **Non-capturing optimization**: When `lambda.num_captures == 0`, the
    /// LLVM function is declared with `ccc` + a phantom `ptr %_env` leading
    /// parameter, making it directly callable as a closure without generating
    /// a `_ori_partial_N` trampoline wrapper. The emission ABI (stored in
    /// `codegen_ctx.functions`) does NOT include the phantom param -- it stays
    /// unchanged so `emit_function()` body emission works correctly.
    pub(super) fn declare_and_process_lambda(
        &mut self,
        lambda: &mut ori_arc::ArcFunction,
    ) -> (Name, FunctionId, FunctionAbi) {
        let is_non_capturing = lambda.num_captures == 0;

        let mut abi = self.compute_arc_function_abi(lambda);

        // Non-capturing lambdas use `ccc` so they match the closure calling
        // convention directly: `(ptr %env, user_args...) -> ret`.
        if is_non_capturing {
            abi.call_conv = CallConv::C;
        }

        // Assign a globally unique lambda name to prevent cross-function
        // collisions in codegen_ctx.functions. Each lower_function_can call
        // numbers lambdas starting at 0, so two functions each containing a
        // lambda both produce `__lambda_0`. The global counter ensures
        // uniqueness across the entire module.
        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let unique_name = self.interner.intern(&format!("__lambda_{counter}"));
        lambda.name = unique_name;

        let symbol = self
            .mangler
            .mangle_function(self.module_path, &format!("__lambda_{counter}"));

        debug!(
            name = %self.interner.lookup(unique_name),
            symbol,
            params = abi.params.len(),
            non_capturing = is_non_capturing,
            "declaring lambda"
        );

        // Declare with phantom env param for non-capturing lambdas.
        // The emission ABI (registered below) does NOT include the phantom
        // param -- emit_function() adjusts llvm_param_idx to skip it.
        let func_id = if is_non_capturing {
            let ptr_ty = self.builder.ptr_type();
            self.declare_function_llvm_with_extra_params(&symbol, &abi, &[ptr_ty])
        } else {
            self.declare_function_llvm(&symbol, &abi)
        };

        if is_non_capturing {
            self.codegen_ctx.non_capturing_lambdas.insert(unique_name);
        }

        self.codegen_ctx
            .functions
            .insert(unique_name, (func_id, abi.clone()));

        // ARC processing
        ori_arc::annotate_arg_ownership(
            lambda,
            self.annotated_sigs,
            self.interner,
            &self.builtin_ownership,
            self.pool,
        );
        let arc_problems = ori_arc::run_arc_pipeline(
            lambda,
            self.arc_classifier,
            self.annotated_sigs,
            self.pool,
            self.interner,
            &self.uniqueness_summaries,
        );
        for problem in &arc_problems {
            debug!(?problem, "ARC pipeline problem (lambda)");
        }

        (unique_name, func_id, abi)
    }

    /// Check if an ARC function is nounwind (cannot unwind/panic).
    ///
    /// A function is nounwind if:
    /// 1. All `Invoke` callees are already known-nounwind (in the set), AND
    /// 2. No `Apply` calls a may-unwind function, AND
    /// 3. No `ApplyIndirect` instructions exist (indirect calls through
    ///    closures/function pointers are conservatively may-unwind).
    ///
    /// For `Apply` callees, three cases:
    /// - **Runtime function with `Nounwind` attr**: safe (cannot unwind).
    /// - **Runtime function WITHOUT `Nounwind` attr**: unsafe — may call
    ///   `ori_panic` internally (e.g., `ori_list_get` on OOB, `ori_assert`
    ///   on failure, allocating functions on OOM).
    /// - **User-defined function**: check `nounwind_functions` set.
    ///
    /// Indirect calls (`ApplyIndirect`) cannot be statically resolved to a
    /// known callee, so we must conservatively assume they may unwind. This
    /// prevents UB when a closure target panics inside a `nounwind` function.
    pub(super) fn is_arc_function_nounwind(&self, func: &ori_arc::ArcFunction) -> bool {
        use crate::codegen::runtime_decl::runtime_functions::is_rt_fn_nounwind;

        func.blocks.iter().all(|block| {
            let term_ok = match &block.terminator {
                ori_arc::ir::ArcTerminator::Invoke { func: callee, .. } => {
                    self.codegen_ctx.nounwind_functions.contains(callee)
                }
                _ => true,
            };
            let instrs_ok = block.body.iter().all(|instr| match instr {
                ori_arc::ir::ArcInstr::Apply { func: callee, .. } => {
                    let s = self.interner.lookup(*callee);
                    match is_rt_fn_nounwind(s) {
                        // Known runtime function with Nounwind attribute
                        Some(true) => true,
                        // Known runtime function WITHOUT Nounwind — may unwind
                        Some(false) => false,
                        // Not a runtime function — check user function set
                        None => self.codegen_ctx.nounwind_functions.contains(callee),
                    }
                }
                // Indirect calls through closures/function pointers are
                // conservatively treated as may-unwind — we cannot know
                // the callee's unwind behavior at compile time.
                //
                // §02.5 decision: conservative (document limitation).
                // Interprocedural proof (tracking all possible callees for
                // every closure variable) is a significant analysis investment
                // for a LOW-severity finding. The pessimistic result (using
                // invoke instead of call) is always safe.
                ori_arc::ir::ArcInstr::ApplyIndirect { .. } => false,
                _ => true,
            });
            term_ok && instrs_ok
        })
    }
}

/// Remap `PartialApply { func, .. }` callee names in an ARC function.
///
/// After `declare_and_process_lambda` renames lambdas to globally unique names,
/// the parent function's `PartialApply` instructions still reference the old
/// per-function names (e.g., `__lambda_0`). This function updates them to
/// match the new globally unique names so `emit_partial_apply` finds the
/// correct entry in `codegen_ctx.functions`.
pub(super) fn remap_partial_apply_names(func: &mut ori_arc::ArcFunction, renames: &[(Name, Name)]) {
    for block in &mut func.blocks {
        for instr in &mut block.body {
            if let ori_arc::ArcInstr::PartialApply { ref mut func, .. } = instr {
                for &(old, new) in renames {
                    if *func == old {
                        *func = new;
                        break;
                    }
                }
            }
        }
    }
}
