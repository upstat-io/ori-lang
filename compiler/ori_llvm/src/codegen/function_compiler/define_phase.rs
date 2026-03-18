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
use tracing::{debug, trace};

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
            super::purity_analysis::remap_partial_apply_names(&mut arc_func, &lambda_renames);
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

        // Post-emission CFG simplification
        let fn_val = self.builder.get_function_value(func_id);
        crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);

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
                readonly: false,
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
        // Apply AIMS param ownership from pre-computed contracts.
        // Lowering defaults all params to Ownership::Owned (lower/mod.rs).
        // AIMS contracts (from compute_aims_contracts()) provide the correct
        // Owned/Borrowed per param.
        debug!(name = %self.interner.lookup(name), "processing ARC function");
        if let Some(contract) = self.aims_contracts.get(&arc_func.name) {
            for (param, pc) in arc_func.params.iter_mut().zip(&contract.params) {
                param.ownership = match pc.access {
                    ori_arc::aims::lattice::AccessClass::Borrowed => ori_arc::Ownership::Borrowed,
                    ori_arc::aims::lattice::AccessClass::Owned => ori_arc::Ownership::Owned,
                };
            }
        }

        // AIMS pipeline handles arg_ownership internally (Step 4: emit_arg_ownership).
        let arc_problems = ori_arc::run_arc_pipeline(
            arc_func,
            self.arc_classifier,
            self.annotated_sigs,
            self.pool,
            self.interner,
            &self.uniqueness_summaries,
            &self.aims_contracts,
            self.verify_arc,
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

        // Apply AIMS param ownership from pre-computed contracts BEFORE the
        // name change below. The contracts map uses the original lambda name
        // (e.g., `__lambda_0` from lowering). Lambdas need correct
        // Owned/Borrowed annotations so that collect_all_borrowed_defs()
        // correctly identifies borrowed params and their Let aliases.
        // Without this, edge cleanup emits spurious RcDec for
        // borrowed-param aliases (double-free on captured non-scalar
        // values like str, [T]).
        if let Some(contract) = self.aims_contracts.get(&lambda.name) {
            for (param, pc) in lambda.params.iter_mut().zip(&contract.params) {
                param.ownership = match pc.access {
                    ori_arc::aims::lattice::AccessClass::Borrowed => ori_arc::Ownership::Borrowed,
                    ori_arc::aims::lattice::AccessClass::Owned => ori_arc::Ownership::Owned,
                };
            }
        }

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

        // ARC processing — AIMS pipeline handles arg_ownership internally.
        let arc_problems = ori_arc::run_arc_pipeline(
            lambda,
            self.arc_classifier,
            self.annotated_sigs,
            self.pool,
            self.interner,
            &self.uniqueness_summaries,
            &self.aims_contracts,
            self.verify_arc,
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
                ori_arc::ir::ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    let s = self.interner.lookup(*callee);
                    match is_rt_fn_nounwind(s) {
                        // Known runtime function with Nounwind attribute
                        Some(true) => true,
                        // Known runtime function WITHOUT Nounwind — may unwind
                        Some(false) => false,
                        // Not a runtime function — check user function set,
                        // then check if the callee will be intercepted by
                        // builtin handlers (which always emit `call`).
                        None => {
                            self.codegen_ctx.nounwind_functions.contains(callee)
                                || self.is_callee_intercepted(s, *callee, args, func)
                        }
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

    /// Check if a callee will be intercepted by builtin handlers during emission.
    ///
    /// Intercepted calls always emit `call` (never `invoke`), so they are
    /// effectively nounwind from the caller's perspective. This mirrors the
    /// logic in `ArcIrEmitter::callee_will_be_intercepted`.
    fn is_callee_intercepted(
        &self,
        callee_name: &str,
        callee: Name,
        args: &[ori_arc::ir::ArcVarId],
        func: &ori_arc::ArcFunction,
    ) -> bool {
        use crate::codegen::arc_emitter::builtins::prelude::HANDLED_PRELUDE_NAMES;

        // Format call interceptor
        if callee_name.starts_with("ori_format_") {
            return true;
        }
        // Prelude function interceptor
        if HANDLED_PRELUDE_NAMES.contains(&callee_name) {
            return true;
        }
        // Declared user functions use normal dispatch — NOT intercepted
        if self.codegen_ctx.functions.contains_key(&callee) {
            return false;
        }
        // Runtime functions have their own emission paths — NOT intercepted
        if callee_name.starts_with("ori_") || callee_name.starts_with("__") {
            return false;
        }
        // Builtin method: receiver is a builtin type and not in method_functions
        if let Some(&first_arg) = args.first() {
            let receiver_ty = func.var_type(first_arg);
            let type_info = self.type_info.get(receiver_ty);
            if type_info.builtin_type_name().is_some() {
                if let Some(type_name) = self.codegen_ctx.type_idx_to_name.get(&receiver_ty) {
                    if !self
                        .codegen_ctx
                        .method_functions
                        .contains_key(&(*type_name, callee))
                    {
                        return true;
                    }
                } else {
                    // Builtin type but no type_idx_to_name entry — method
                    // dispatch chain can't resolve it, will be intercepted.
                    return true;
                }
            }
        }
        false
    }
}
