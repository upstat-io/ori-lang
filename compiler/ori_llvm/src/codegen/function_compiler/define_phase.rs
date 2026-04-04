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
        mut lambdas: Vec<ori_arc::ArcFunction>,
    ) {
        // Compile lambda ArcFunctions (closures).
        // Each lambda is compiled as a separate LLVM function, registered in
        // self.codegen_ctx.functions so that emit_partial_apply can look it up by Name.
        //
        // declare_and_process_lambda renames each lambda to a globally unique
        // name. We collect the (old → new) mapping so we can update the
        // parent function's PartialApply references.
        // Resolve BoundVar types in polymorphic lambdas before compilation.
        // Must resolve ALL lambdas before compiling ANY, because nested lambdas
        // may reference sibling lambdas' types (e.g., inner lambda's PartialApply
        // is in outer lambda's body, not the parent function's body).
        resolve_all_lambda_bound_vars(&mut arc_func, &mut lambdas, self.pool, self.interner);

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
        // Verify no BoundVar types remain after resolution — captures ARE leading
        // params, so resolve_all_lambda_bound_vars must have resolved them too.
        debug_assert!(
            !lambda
                .params
                .iter()
                .any(|p| matches!(self.pool.tag(p.ty), ori_types::Tag::BoundVar)),
            "lambda {} has unresolved BoundVar params after resolution",
            self.interner.lookup(lambda.name),
        );

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

        // Lambda names are globally unique from lowering (include parent function
        // name: `__lambda_{parent}_{idx}`). No renaming needed — the AIMS contract
        // map uses the same names, so ownership lookup succeeds.
        let unique_name = lambda.name;

        let lambda_name_str = self.interner.lookup(unique_name);
        let symbol = self
            .mangler
            .mangle_function(self.module_path, lambda_name_str);

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

        // Store capture param ownership so emit_partial_apply can generate
        // correct env drop functions: borrowed captures must NOT be RC-dec'd.
        if lambda.num_captures > 0 {
            let capture_ownership: Vec<ori_arc::Ownership> = lambda
                .params
                .iter()
                .take(lambda.num_captures)
                .map(|p| p.ownership)
                .collect();
            self.codegen_ctx
                .lambda_capture_ownership
                .insert(unique_name, capture_ownership);
        }

        (unique_name, func_id, abi)
    }
}

/// Resolve `BoundVar` types in ALL lambdas to concrete types.
///
/// Handles two cases:
/// 1. **Single-instantiation**: a lambda used at one concrete type → resolve directly
/// 2. **Multi-instantiation**: a lambda used at multiple concrete types (e.g.,
///    `let $id = x -> x; id("hello"); id(42)`) → clone per instantiation and
///    rewrite the parent's ARC IR so each use gets the correct specialization
pub(super) fn resolve_all_lambda_bound_vars(
    parent: &mut ori_arc::ArcFunction,
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
) {
    use ori_types::Tag;

    // Check if ANY lambda has BoundVar/Var in params, return type, or var_types.
    // Also check for multi-instantiation (multiple distinct narrowing copies in
    // the parent), which can occur even without BoundVars in the lambda itself
    // when Var types resolve to different concrete types at different call sites.
    let any_polymorphic = lambdas.iter().any(|l| {
        l.params
            .iter()
            .any(|p| matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var))
            || contains_bound_var(pool, l.return_type)
            || contains_nested_var(pool, l.return_type)
            || l.var_types.iter().any(|ty| contains_bound_var(pool, *ty))
    });
    // Also check if any lambda has multiple narrowing copies in the parent
    // (multi-inst detection), which requires cloning regardless of BoundVars.
    let any_multi_inst = !any_polymorphic
        && lambdas
            .iter()
            .any(|l| find_all_instantiation_types(parent, l.name, pool).len() > 1);
    if !any_polymorphic && !any_multi_inst {
        return;
    }

    // Phase 1: Detect multi-instantiation and handle it by cloning lambdas.
    // Must run before the global map build because multi-inst lambdas get
    // specialized clones that are resolved independently.
    let orig_len = lambdas.len();
    let mut multi_inst_lambdas = rustc_hash::FxHashSet::<usize>::default();

    for i in 0..orig_len {
        let has_polymorphic = lambdas[i]
            .params
            .iter()
            .any(|p| matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var))
            || contains_bound_var(pool, lambdas[i].return_type)
            || contains_nested_var(pool, lambdas[i].return_type);

        let lambda_name = lambdas[i].name;
        let instantiations = find_all_instantiation_types(parent, lambda_name, pool);

        if instantiations.len() <= 1 && !has_polymorphic {
            continue;
        }

        if instantiations.len() > 1 {
            clone_multi_inst_lambda(
                parent,
                lambdas,
                i,
                lambda_name,
                &instantiations,
                interner,
                pool,
            );
            multi_inst_lambdas.insert(i);
        }
    }

    // Phase 2: Build global BoundVar → concrete map for single-inst lambdas.
    let mut global_map: rustc_hash::FxHashMap<u32, Idx> = rustc_hash::FxHashMap::default();
    // Track return type resolutions: lambda index → (schema_ret, concrete_ret).
    let mut ret_type_resolutions: rustc_hash::FxHashMap<usize, (Idx, Idx)> =
        rustc_hash::FxHashMap::default();

    for i in 0..orig_len {
        if multi_inst_lambdas.contains(&i) {
            continue; // Already handled above.
        }
        let has_polymorphic = lambdas[i]
            .params
            .iter()
            .any(|p| matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var))
            || contains_bound_var(pool, lambdas[i].return_type)
            || contains_nested_var(pool, lambdas[i].return_type);
        if !has_polymorphic {
            continue;
        }

        let lambda_name = lambdas[i].name;
        let concrete_fn_ty =
            find_partial_apply_concrete_type(parent, lambdas, i, lambda_name, pool);

        if let Some(concrete_ty) = concrete_fn_ty {
            build_bound_var_map(
                pool,
                concrete_ty,
                &lambdas[i].params,
                lambdas[i].return_type,
                &mut global_map,
            );

            // Track return type resolution: find the concrete return type from
            // ApplyIndirect results in the parent (not from the function type,
            // which may still contain unresolved Vars inside containers).
            // Only resolve return types that contain Vars inside containers
            // (e.g., Option<Var>, Result<Var>). Direct Var return types are
            // already handled by apply_bound_var_map.
            let schema_ret = lambdas[i].return_type;
            if contains_var(pool, schema_ret) {
                // The pool's function types may still contain Var indices inside
                // containers. Get the concrete return type from ApplyIndirect
                // results in the parent, which use fully-resolved types.
                if let Some(concrete_ret) =
                    find_apply_indirect_result_type(parent, lambdas[i].name, pool)
                {
                    if concrete_ret != schema_ret {
                        ret_type_resolutions.insert(i, (schema_ret, concrete_ret));
                    }
                }
            }
        }
    }

    // Apply the global map + return type resolutions to ALL non-multi-inst lambdas.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue; // Multi-inst originals are not compiled.
        }
        apply_bound_var_map(lambda, &global_map, pool);

        if let Some(&(schema_ret, concrete_ret)) = ret_type_resolutions.get(&i) {
            resolve_lambda_return_types(lambda, schema_ret, concrete_ret);
        }
    }

    // Final fallback: any remaining BoundVars → Idx::INT.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue;
        }
        fallback_bound_vars_to_int(lambda, pool);
    }

    // Remove multi-inst originals — replaced by specialized clones.
    remove_multi_inst_originals(lambdas, multi_inst_lambdas);
}

/// Remove original multi-instantiated lambdas from the vec. These have been
/// replaced by specialized clones — if left in, `emit_arc_function` compiles
/// them with unresolved type variables. Removes in reverse index order so
/// earlier indices remain valid after each removal.
fn remove_multi_inst_originals(
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    multi_inst_indices: rustc_hash::FxHashSet<usize>,
) {
    if multi_inst_indices.is_empty() {
        return;
    }
    let mut to_remove: Vec<usize> = multi_inst_indices.into_iter().collect();
    to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for idx in to_remove {
        lambdas.remove(idx);
    }
}

/// Clone a multi-instantiated lambda: create one clone per distinct concrete
/// instantiation, resolve each clone's types, and rewrite the parent's ARC IR.
fn clone_multi_inst_lambda(
    parent: &mut ori_arc::ArcFunction,
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    orig_idx: usize,
    lambda_name: Name,
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    pool: &ori_types::Pool,
) {
    let pa_args = find_partial_apply_args(parent, lambda_name);
    let schema_ret = lambdas[orig_idx].return_type;

    for (inst_idx, concrete_fn_ty) in instantiations.iter().enumerate() {
        let mut clone = lambdas[orig_idx].clone();
        let base = interner.lookup(lambda_name);
        let spec_name_str = format!("{base}${inst_idx}");
        clone.name = interner.intern(&spec_name_str);

        // Build per-instantiation BoundVar map and apply it.
        let mut inst_map = rustc_hash::FxHashMap::<u32, Idx>::default();
        build_bound_var_map(
            pool,
            *concrete_fn_ty,
            &clone.params,
            clone.return_type,
            &mut inst_map,
        );
        apply_bound_var_map(&mut clone, &inst_map, pool);

        // Set return type and matching var_types/instructions from the concrete
        // instantiation. Only exact Idx match to avoid over-replacing.
        let concrete_ret = pool.function_return(*concrete_fn_ty);
        resolve_lambda_return_types(&mut clone, schema_ret, concrete_ret);

        fallback_bound_vars_to_int(&mut clone, pool);
        lambdas.push(clone);
    }

    rewrite_parent_for_multi_inst(
        parent,
        lambda_name,
        &pa_args,
        instantiations,
        interner,
        pool,
    );
}

/// Find all distinct concrete Function types that a polymorphic lambda is
/// narrowed to in the parent function's `var_types` (via Let copies).
fn find_all_instantiation_types(
    parent: &ori_arc::ArcFunction,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Vec<Idx> {
    use ori_types::Tag;

    // Find the PartialApply dst variable for this lambda.
    let mut pa_dst = None;
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == lambda_name {
                    pa_dst = Some(*dst);
                    break;
                }
            }
        }
        if pa_dst.is_some() {
            break;
        }
    }

    let Some(pa_dst) = pa_dst else {
        return Vec::new();
    };

    // Find all Let copies: `%N = %pa_dst` where %N has a concrete Function type.
    let mut instantiations: Vec<Idx> = Vec::new();
    let mut seen = rustc_hash::FxHashSet::<Vec<Idx>>::default();

    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    let ty = parent.var_type(*dst);
                    let resolved = pool.resolve_fully(ty);
                    if pool.tag(resolved) == Tag::Function {
                        let params = pool.function_params(resolved);
                        let ret = pool.function_return(resolved);
                        let all_concrete = params.iter().chain(std::iter::once(&ret)).all(|p| {
                            let pt = pool.resolve_fully(*p);
                            !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                        });
                        if all_concrete {
                            // Deduplicate by param types + return type.
                            let key: Vec<Idx> = params
                                .iter()
                                .chain(std::iter::once(&ret))
                                .map(|p| pool.resolve_fully(*p))
                                .collect();
                            if seen.insert(key) {
                                instantiations.push(resolved);
                            }
                        }
                    }
                }
            }
        }
    }

    instantiations
}

/// Find the capture arguments from a `PartialApply` instruction for a lambda.
fn find_partial_apply_args(
    parent: &ori_arc::ArcFunction,
    lambda_name: Name,
) -> Vec<ori_arc::ir::ArcVarId> {
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                func: callee, args, ..
            } = instr
            {
                if *callee == lambda_name {
                    return args.clone();
                }
            }
        }
    }
    Vec::new()
}

/// Rewrite the parent's ARC IR for a multi-instantiated lambda: replace
/// narrowing Let copies with `PartialApply` of the correct specialization.
fn rewrite_parent_for_multi_inst(
    parent: &mut ori_arc::ArcFunction,
    lambda_name: Name,
    pa_args: &[ori_arc::ir::ArcVarId],
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    pool: &ori_types::Pool,
) {
    use ori_types::Tag;

    // Find the PartialApply dst variable.
    let mut pa_dst = None;
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == lambda_name {
                    pa_dst = Some(*dst);
                    break;
                }
            }
        }
        if pa_dst.is_some() {
            break;
        }
    }
    let Some(pa_dst) = pa_dst else { return };

    let base = interner.lookup(lambda_name);

    // Replace narrowing Let copies with PartialApply of the correct clone.
    for block in &mut parent.blocks {
        for instr in &mut block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ty,
            } = instr
            {
                if *src == pa_dst {
                    let var_ty = parent.var_types[dst.index()];
                    let resolved = pool.resolve_fully(var_ty);
                    if pool.tag(resolved) == Tag::Function {
                        let params = pool.function_params(resolved);
                        let ret = pool.function_return(resolved);
                        let all_concrete = params.iter().chain(std::iter::once(&ret)).all(|p| {
                            let pt = pool.resolve_fully(*p);
                            !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                        });
                        if all_concrete {
                            // Find which instantiation index matches (param types + return type).
                            let key: Vec<Idx> = params
                                .iter()
                                .chain(std::iter::once(&ret))
                                .map(|p| pool.resolve_fully(*p))
                                .collect();
                            for (idx, inst_ty) in instantiations.iter().enumerate() {
                                let inst_params = pool.function_params(*inst_ty);
                                let inst_ret = pool.function_return(*inst_ty);
                                let inst_key: Vec<Idx> = inst_params
                                    .iter()
                                    .chain(std::iter::once(&inst_ret))
                                    .map(|p| pool.resolve_fully(*p))
                                    .collect();
                                if key == inst_key {
                                    let spec_name_str = format!("{base}${idx}");
                                    let spec_name = interner.intern(&spec_name_str);
                                    *instr = ori_arc::ir::ArcInstr::PartialApply {
                                        dst: *dst,
                                        ty: *ty,
                                        func: spec_name,
                                        args: pa_args.to_vec(),
                                    };
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Remove RcInc/RcDec on the original PartialApply result that fed the
    // now-replaced Let copies. Each specialization creates its own closure.
    for block in &mut parent.blocks {
        block.body.retain(|instr| {
            !matches!(instr,
                ori_arc::ir::ArcInstr::RcInc { var, .. } | ori_arc::ir::ArcInstr::RcDec { var, .. }
                if *var == pa_dst
            )
        });
    }
}

/// Search parent + all sibling lambdas for a `PartialApply` that references the
/// given lambda, and return the concrete instantiated function type and the
/// capture argument variable IDs (for capture type resolution).
fn find_partial_apply_concrete_type(
    parent: &ori_arc::ArcFunction,
    lambdas: &[ori_arc::ArcFunction],
    skip_idx: usize,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    use ori_types::Tag;

    // Check if PartialApply dst type is concrete.
    let check_concrete =
        |func: &ori_arc::ArcFunction, dst: &ori_arc::ir::ArcVarId| -> Option<Idx> {
            let pa_ty = func.var_type(*dst);
            let resolved = pool.resolve_fully(pa_ty);
            if pool.tag(resolved) == Tag::Function {
                let params = pool.function_params(resolved);
                let all_concrete = params.iter().all(|p| {
                    let pt = pool.resolve_fully(*p);
                    !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                });
                if all_concrete {
                    return Some(resolved);
                }
            }
            None
        };

    // Find the PartialApply in a function's blocks and return its dst var.
    let find_pa = |func: &ori_arc::ArcFunction| -> Option<ori_arc::ir::ArcVarId> {
        for block in &func.blocks {
            for instr in &block.body {
                if let ori_arc::ir::ArcInstr::PartialApply {
                    dst, func: callee, ..
                } = instr
                {
                    if *callee == lambda_name {
                        return Some(*dst);
                    }
                }
            }
        }
        None
    };

    // Search parent first.
    if let Some(dst) = find_pa(parent) {
        if let Some(ty) = check_concrete(parent, &dst) {
            return Some(ty);
        }
        // PartialApply type not concrete — scan narrowing Let copies of this
        // specific PartialApply dst.
        if let Some(ty) = find_concrete_copy_of(parent, dst, pool) {
            return Some(ty);
        }
        // Fallback: scan all concrete function types in parent.
        if let Some(ty) = find_any_concrete_fn_type(parent, pool) {
            return Some(ty);
        }
    }

    // Search sibling lambdas (skip self).
    for (j, sibling) in lambdas.iter().enumerate() {
        if j == skip_idx {
            continue;
        }
        if let Some(dst) = find_pa(sibling) {
            if let Some(ty) = check_concrete(sibling, &dst) {
                return Some(ty);
            }
            if let Some(ty) = find_concrete_copy_of(sibling, dst, pool) {
                return Some(ty);
            }
            // For nested lambdas, the concrete type may be in the parent.
            // Use scoped search first, fall back to unscoped if only one
            // polymorphic lambda exists (no ambiguity risk).
            if let Some(parent_dst) = find_pa(parent) {
                if let Some(ty) = find_concrete_copy_of(parent, parent_dst, pool) {
                    return Some(ty);
                }
            }
            // Final fallback: scan all concrete function types in the sibling
            // and parent. Safe only when there's a single lambda in scope.
            if let Some(ty) = find_any_concrete_fn_type(sibling, pool) {
                return Some(ty);
            }
            if let Some(ty) = find_any_concrete_fn_type(parent, pool) {
                return Some(ty);
            }
        }
    }

    None
}

/// Apply a `BoundVar` → concrete mapping to a lambda's types.
fn apply_bound_var_map(
    lambda: &mut ori_arc::ArcFunction,
    map: &rustc_hash::FxHashMap<u32, Idx>,
    pool: &ori_types::Pool,
) {
    use ori_types::Tag;

    if map.is_empty() {
        return;
    }

    for param in &mut lambda.params {
        if matches!(pool.tag(param.ty), Tag::BoundVar | Tag::Var) {
            let var_id = pool.data(param.ty);
            if let Some(&concrete) = map.get(&var_id) {
                param.ty = concrete;
            }
        }
    }

    for ty in &mut lambda.var_types {
        if matches!(pool.tag(*ty), Tag::BoundVar | Tag::Var) {
            let var_id = pool.data(*ty);
            if let Some(&concrete) = map.get(&var_id) {
                *ty = concrete;
            }
        }
    }

    if matches!(pool.tag(lambda.return_type), Tag::BoundVar | Tag::Var) {
        let var_id = pool.data(lambda.return_type);
        if let Some(&concrete) = map.get(&var_id) {
            lambda.return_type = concrete;
        }
    }
}

/// Fall back: any remaining `BoundVar`s/`Var`s → `Idx::INT`.
fn fallback_bound_vars_to_int(lambda: &mut ori_arc::ArcFunction, pool: &ori_types::Pool) {
    use ori_types::Tag;

    for param in &mut lambda.params {
        if matches!(pool.tag(param.ty), Tag::BoundVar | Tag::Var) {
            param.ty = Idx::INT;
        }
    }
    for ty in &mut lambda.var_types {
        if matches!(pool.tag(*ty), Tag::BoundVar | Tag::Var) {
            *ty = Idx::INT;
        }
    }
    if contains_bound_var(pool, lambda.return_type) {
        lambda.return_type = Idx::INT;
    }
}

/// Resolve a lambda's return type, `var_types`, and `Construct` instruction types
/// from a schema->concrete mapping. Used by both multi-inst cloning and
/// single-inst return-type resolution.
fn resolve_lambda_return_types(
    lambda: &mut ori_arc::ArcFunction,
    schema_ret: Idx,
    concrete_ret: Idx,
) {
    lambda.return_type = concrete_ret;
    for ty in &mut lambda.var_types {
        if *ty == schema_ret {
            *ty = concrete_ret;
        }
    }
    for block in &mut lambda.blocks {
        for instr in &mut block.body {
            if let ori_arc::ir::ArcInstr::Construct { ty, .. } = instr {
                if *ty == schema_ret {
                    *ty = concrete_ret;
                }
            }
        }
    }
}

/// Scan all `var_types` for the first concrete Function type. Less precise
/// than `find_concrete_copy_of` — may match types from unrelated lambdas.
/// Use only as a last-resort fallback for nested lambda resolution.
fn find_any_concrete_fn_type(func: &ori_arc::ArcFunction, pool: &ori_types::Pool) -> Option<Idx> {
    use ori_types::Tag;
    for ty in &func.var_types {
        let resolved = pool.resolve_fully(*ty);
        if pool.tag(resolved) == Tag::Function {
            let params = pool.function_params(resolved);
            let all_concrete = params.iter().all(|p| {
                let pt = pool.resolve_fully(*p);
                !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
            });
            if all_concrete {
                return Some(resolved);
            }
        }
    }
    None
}

/// Find the first concrete Function type from a Let copy of a specific
/// `PartialApply` dst variable. Only considers `Let { src: pa_dst }` instructions
/// — avoids matching concrete types from unrelated lambdas.
fn find_concrete_copy_of(
    func: &ori_arc::ArcFunction,
    pa_dst: ori_arc::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    use ori_types::Tag;
    for block in &func.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    let ty = func.var_type(*dst);
                    let resolved = pool.resolve_fully(ty);
                    if pool.tag(resolved) == Tag::Function {
                        let params = pool.function_params(resolved);
                        let all_concrete = params.iter().all(|p| {
                            let pt = pool.resolve_fully(*p);
                            !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                        });
                        if all_concrete {
                            return Some(resolved);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Build a `BoundVar` → concrete type mapping by comparing the concrete function
/// type's parameter types with the lambda's BoundVar-typed parameters.
fn build_bound_var_map(
    pool: &ori_types::Pool,
    concrete_fn_ty: Idx,
    lambda_params: &[ori_arc::ir::ArcParam],
    lambda_return_type: Idx,
    map: &mut rustc_hash::FxHashMap<u32, Idx>,
) {
    use ori_types::Tag;

    if pool.tag(concrete_fn_ty) != Tag::Function {
        return;
    }

    let concrete_params = pool.function_params(concrete_fn_ty);
    let concrete_ret = pool.function_return(concrete_fn_ty);

    // Map lambda param BoundVars to concrete param types.
    // Lambda params include captures (leading) + user params.
    // The concrete function type only has user params (no captures).
    // So we align from the END of the lambda params.
    let num_captures = lambda_params.len().saturating_sub(concrete_params.len());

    for (i, concrete_ty) in concrete_params.iter().enumerate() {
        let lambda_idx = num_captures + i;
        if lambda_idx < lambda_params.len() {
            let param_ty = lambda_params[lambda_idx].ty;
            if matches!(pool.tag(param_ty), Tag::BoundVar | Tag::Var) {
                let var_id = pool.data(param_ty);
                let resolved_concrete = pool.resolve_fully(*concrete_ty);
                map.insert(var_id, resolved_concrete);
            }
        }
    }

    // Map return type BoundVars by structural comparison with the concrete
    // return type. Only for multi-inst lambdas where the return type contains
    // BoundVars that aren't already mapped from params.
    // Unwrap Scheme if needed to reach the inner type.
    let schema_ret = if pool.tag(lambda_return_type) == Tag::Scheme {
        pool.scheme_body(lambda_return_type)
    } else {
        lambda_return_type
    };
    // Only run structural matching if the return type actually has unmapped
    // BoundVars. For lambdas where params share BoundVars with the return
    // type (e.g., `a -> a + b`), the params already populated the map.
    if contains_bound_var(pool, schema_ret) {
        map_types_structural(pool, schema_ret, pool.resolve_fully(concrete_ret), map);
    }
}

/// Walk `schema_ty` and `concrete_ty` in parallel. When a `BoundVar` is found
/// in `schema_ty`, map it to the corresponding type in `concrete_ty`.
/// Recurses into `Option`, `Result`, `List`, `Function`, and other container types.
fn map_types_structural(
    pool: &ori_types::Pool,
    schema_ty: Idx,
    concrete_ty: Idx,
    map: &mut rustc_hash::FxHashMap<u32, Idx>,
) {
    use ori_types::Tag;

    let schema_tag = pool.tag(schema_ty);

    // Direct BoundVar/Var → map to the concrete type.
    if matches!(schema_tag, Tag::BoundVar | Tag::Var) {
        let var_id = pool.data(schema_ty);
        map.insert(var_id, concrete_ty);
        return;
    }

    let concrete_tag = pool.tag(concrete_ty);
    if schema_tag != concrete_tag {
        return; // Structural mismatch — can't extract mappings.
    }

    // Recurse into container types.
    match schema_tag {
        Tag::Option => {
            let s_inner = pool.option_inner(schema_ty);
            let c_inner = pool.option_inner(concrete_ty);
            map_types_structural(pool, s_inner, c_inner, map);
        }
        Tag::Result => {
            let s_ok = pool.result_ok(schema_ty);
            let c_ok = pool.result_ok(concrete_ty);
            map_types_structural(pool, s_ok, c_ok, map);
            let s_err = pool.result_err(schema_ty);
            let c_err = pool.result_err(concrete_ty);
            map_types_structural(pool, s_err, c_err, map);
        }
        Tag::List => {
            let s_elem = pool.list_elem(schema_ty);
            let c_elem = pool.list_elem(concrete_ty);
            map_types_structural(pool, s_elem, c_elem, map);
        }
        Tag::Function => {
            let s_params = pool.function_params(schema_ty);
            let c_params = pool.function_params(concrete_ty);
            for (sp, cp) in s_params.iter().zip(c_params.iter()) {
                map_types_structural(pool, *sp, *cp, map);
            }
            let s_ret = pool.function_return(schema_ty);
            let c_ret = pool.function_return(concrete_ty);
            map_types_structural(pool, s_ret, c_ret, map);
        }
        _ => {} // Primitives and other leaf types — nothing to extract.
    }
}

/// Find the concrete return type by looking at `ApplyIndirect` results in the
/// parent function. When a narrowing Let copy of a lambda is called via
/// `ApplyIndirect`, the result's `var_type` is the concrete return type.
fn find_apply_indirect_result_type(
    parent: &ori_arc::ArcFunction,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    use ori_types::Tag;

    // Find the PartialApply dst for this lambda.
    let mut pa_dst = None;
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == lambda_name {
                    pa_dst = Some(*dst);
                    break;
                }
            }
        }
        if pa_dst.is_some() {
            break;
        }
    }
    let pa_dst = pa_dst?;

    // Find Let copies of the PartialApply dst.
    let mut narrowing_vars = Vec::new();
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    narrowing_vars.push(*dst);
                }
            }
        }
    }

    // Find ApplyIndirect calls on narrowing vars and return the result type.
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::ApplyIndirect { dst, closure, .. } = instr {
                if narrowing_vars.contains(closure) {
                    let result_ty = parent.var_type(*dst);
                    let resolved = pool.resolve_fully(result_ty);
                    if !matches!(pool.tag(resolved), Tag::BoundVar | Tag::Var | Tag::Scheme) {
                        return Some(resolved);
                    }
                }
            }
        }
    }

    None
}

/// Check if a type contains a `Var` INSIDE a container (not at the top level).
/// A bare `Var` return type is handled by `apply_bound_var_map`/`fallback_bound_vars_to_int`.
/// A `Var` inside `Option<Var>` or `Result<Var, E>` requires explicit return
/// type resolution because the pool is immutable and can't substitute inside containers.
fn contains_nested_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    use ori_types::Tag;
    match pool.tag(ty) {
        Tag::Option => contains_var(pool, pool.option_inner(ty)),
        Tag::Result => {
            contains_var(pool, pool.result_ok(ty)) || contains_var(pool, pool.result_err(ty))
        }
        Tag::List => contains_var(pool, pool.list_elem(ty)),
        _ => false,
    }
}

/// Check if a type contains a `Var` (inference variable) at any nesting level.
/// Unlike `contains_bound_var`, this checks for `Var` tags WITHOUT resolving —
/// a `Var` inside `Option<Var>` means the pool Idx is polymorphic and the LLVM
/// emitter may produce the wrong type layout.
fn contains_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    use ori_types::Tag;
    match pool.tag(ty) {
        Tag::Var => true,
        Tag::Option => contains_var(pool, pool.option_inner(ty)),
        Tag::Result => {
            contains_var(pool, pool.result_ok(ty)) || contains_var(pool, pool.result_err(ty))
        }
        Tag::List => contains_var(pool, pool.list_elem(ty)),
        Tag::Function => {
            pool.function_params(ty)
                .iter()
                .any(|p| contains_var(pool, *p))
                || contains_var(pool, pool.function_return(ty))
        }
        _ => false,
    }
}

/// Check if a type contains any unresolvable `BoundVar` (not `Var` — `Var`s may
/// be resolved through pool links). Only `BoundVar` is truly unresolved.
/// Recurses into `Option`, `Result`, `List`, `Function`, and `Scheme`.
fn contains_bound_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    use ori_types::Tag;

    let resolved = pool.resolve_fully(ty);
    match pool.tag(resolved) {
        Tag::BoundVar | Tag::Scheme => true,
        Tag::Option => contains_bound_var(pool, pool.option_inner(resolved)),
        Tag::Result => {
            contains_bound_var(pool, pool.result_ok(resolved))
                || contains_bound_var(pool, pool.result_err(resolved))
        }
        Tag::List => contains_bound_var(pool, pool.list_elem(resolved)),
        Tag::Function => {
            pool.function_params(resolved)
                .iter()
                .any(|p| contains_bound_var(pool, *p))
                || contains_bound_var(pool, pool.function_return(resolved))
        }
        _ => false,
    }
}
