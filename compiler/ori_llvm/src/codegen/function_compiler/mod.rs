//! Two-pass function compilation for V2 codegen.
//!
//! `FunctionCompiler` implements the declare-then-define pattern:
//!
//! 1. **Phase 1 (declare)**: Walk all functions, compute `FunctionAbi` from
//!    `ori_types::FunctionSig`, declare LLVM functions with correct types,
//!    calling conventions, and attributes (sret, noalias).
//!
//! 2. **Phase 2 (define)**: Walk all functions again, lower through the ARC
//!    pipeline (`CanExpr` → ARC IR → `ArcIrEmitter` → LLVM IR).
//!
//! Submodules:
//! - [`nounwind`]: Two-pass nounwind analysis (prepare → analyze → emit)
//! - [`impls`]: Impl method, test, and derived trait compilation
//! - [`entry_point`]: AOT `main()` wrapper and panic trampoline

mod entry_point;
mod impls;
mod nounwind;

pub use nounwind::PreparedFunction;

use std::cell::Cell;

use ori_arc::{lower_function_can, AnnotatedSig, ArcClassifier};
use ori_ir::canon::{CanId, CanonResult};
use ori_ir::{Function, Name, Span, StringInterner};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use crate::aot::debug::DebugContext;
use crate::aot::mangle::Mangler;

use super::abi::{
    compute_function_abi_with_ownership, compute_param_passing, compute_return_passing, CallConv,
    FunctionAbi, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing,
};
use super::arc_emitter::{ArcIrEmitter, CodegenContext};
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

// ---------------------------------------------------------------------------
// FunctionCompiler
// ---------------------------------------------------------------------------

/// Two-pass function compiler.
///
/// Holds the mapping from function `Name` → `(FunctionId, FunctionAbi)`,
/// enabling call sites to look up the callee's ABI for correct argument
/// passing (direct vs. sret).
pub struct FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    type_info: &'a TypeInfoStore<'tcx>,
    type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    interner: &'a StringInterner,
    pool: &'tcx Pool,
    /// Symbol mangler for generating unique LLVM symbol names.
    mangler: Mangler,
    /// Module path for name mangling (e.g., "", "math", "data/utils").
    module_path: &'a str,
    /// Shared function-resolution lookup tables passed to [`ArcIrEmitter`].
    codegen_ctx: CodegenContext,
    /// Module-wide lambda counter for unique lambda function names.
    lambda_counter: Cell<u32>,
    /// Borrow inference results: function `Name` → annotated signature.
    /// `Ownership::Borrowed` + non-Scalar parameters use
    /// `ParamPassing::Reference` (pointer, no RC at call site).
    annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    /// Type classifier for ARC analysis (scalar vs ref classification).
    arc_classifier: &'a ArcClassifier<'tcx>,
    /// Debug info context (None for JIT, Some for AOT with debug info enabled).
    debug_context: Option<&'a DebugContext<'ctx>>,
    /// Builtin method names whose receiver is borrowed (e.g., `len`, `is_empty`).
    /// Passed to `annotate_arg_ownership` so inline-compiled builtins get
    /// borrowing semantics instead of the default all-Owned.
    borrowing_builtins: rustc_hash::FxHashSet<Name>,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new function compiler.
    ///
    /// `module_path` determines name mangling: `""` for the root module,
    /// `"math"` or `"data/utils"` for nested modules. All LLVM symbols
    /// are mangled (e.g., `add` → `_ori_add`, `math.add` → `_ori_math$add`).
    ///
    /// `annotated_sigs` and `arc_classifier` drive borrow-aware ABI:
    /// `Borrowed` + non-Scalar parameters use `Reference` passing
    /// (pointer, no RC at call site).
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'tcx Pool,
        module_path: &'a str,
        annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
        arc_classifier: &'a ArcClassifier<'tcx>,
        debug_context: Option<&'a DebugContext<'ctx>>,
    ) -> Self {
        let borrowing_builtins = ori_arc::borrowing_builtin_names(interner);
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            mangler: Mangler::new(),
            module_path,
            codegen_ctx: CodegenContext::default(),
            lambda_counter: Cell::new(0),
            annotated_sigs,
            arc_classifier,
            debug_context,
            borrowing_builtins,
        }
    }

    // -----------------------------------------------------------------------
    // Phase 1: Declare
    // -----------------------------------------------------------------------

    /// Declare all module functions from type checker signatures.
    ///
    /// Iterates over `module.functions` paired with their `FunctionSig` from the
    /// type checker. Generic functions are skipped (they require monomorphization).
    pub fn declare_all(&mut self, module_functions: &[Function], function_sigs: &[FunctionSig]) {
        for (func, sig) in module_functions.iter().zip(function_sigs.iter()) {
            // Skip generic functions
            if sig.is_generic() {
                trace!(
                    name = %self.interner.lookup(func.name),
                    "skipping generic function declaration"
                );
                continue;
            }

            self.declare_function(func.name, sig, func.span);
        }
    }

    /// Declare a single function from its type checker signature.
    ///
    /// The LLVM symbol uses the mangled name (e.g., `_ori_add`), while the
    /// `functions` map key uses the interned `Name` for internal lookups.
    fn declare_function(&mut self, name: Name, sig: &FunctionSig, span: Span) {
        let name_str = self.interner.lookup(name);
        let symbol = self.mangler.mangle_function(self.module_path, name_str);
        self.declare_function_with_symbol(name, &symbol, sig, span);
    }

    /// Declare an LLVM function from pre-computed ABI and symbol name.
    ///
    /// Shared core for function declaration: builds LLVM parameter types
    /// (sret pointer, direct, indirect/reference), declares the function
    /// (direct vs void return), sets calling convention, and applies sret
    /// attributes. Callers handle ABI computation, debug info, and registration.
    pub(super) fn declare_function_llvm(&mut self, symbol: &str, abi: &FunctionAbi) -> FunctionId {
        self.declare_function_llvm_with_extra_params(symbol, abi, &[])
    }

    /// Declare an LLVM function with optional extra leading params before ABI params.
    ///
    /// `extra_leading_params` are inserted after the sret pointer (if any) but
    /// before the ABI-derived parameters. Used by non-capturing lambdas to
    /// prepend a phantom `ptr %_env` parameter that makes the declaration
    /// compatible with the closure calling convention `(env_ptr, user_args...)`.
    fn declare_function_llvm_with_extra_params(
        &mut self,
        symbol: &str,
        abi: &FunctionAbi,
        extra_leading_params: &[LLVMTypeId],
    ) -> FunctionId {
        let mut llvm_param_types =
            Vec::with_capacity(abi.params.len() + extra_leading_params.len() + 1);

        let return_llvm_type = self.type_resolver.resolve(abi.return_abi.ty);
        let return_llvm_id = self.builder.register_type(return_llvm_type);

        if matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }) {
            llvm_param_types.push(self.builder.ptr_type());
        }

        llvm_param_types.extend_from_slice(extra_leading_params);

        for param in &abi.params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = self.type_resolver.resolve(param.ty);
                    llvm_param_types.push(self.builder.register_type(ty));
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    llvm_param_types.push(self.builder.ptr_type());
                }
                ParamPassing::Void => {}
            }
        }

        let func_id = match &abi.return_abi.passing {
            ReturnPassing::Direct => {
                self.builder
                    .declare_function(symbol, &llvm_param_types, return_llvm_id)
            }
            ReturnPassing::Sret { .. } | ReturnPassing::Void => self
                .builder
                .declare_void_function(symbol, &llvm_param_types),
        };

        match abi.call_conv {
            CallConv::Fast => self.builder.set_fastcc(func_id),
            CallConv::C => self.builder.set_ccc(func_id),
        }

        if let ReturnPassing::Sret { .. } = &abi.return_abi.passing {
            self.builder.add_sret_attribute(func_id, 0, return_llvm_id);
            self.builder.add_noalias_attribute(func_id, 0);
        }

        func_id
    }

    /// Declare a function with an explicit LLVM symbol name.
    ///
    /// Computes ABI from signature, delegates to [`Self::declare_function_llvm`]
    /// for LLVM-level declaration, then attaches debug info and registers the
    /// function for internal lookup.
    pub(super) fn declare_function_with_symbol(
        &mut self,
        name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) {
        let name_str = self.interner.lookup(name);

        let abi = compute_function_abi_with_ownership(
            sig,
            self.type_info,
            self.annotated_sigs.get(&name),
            self.arc_classifier,
        );

        debug!(
            name = name_str,
            symbol,
            params = abi.params.len(),
            call_conv = ?abi.call_conv,
            return_passing = ?abi.return_abi.passing,
            "declaring function"
        );

        let func_id = self.declare_function_llvm(symbol, &abi);

        if let Some(dc) = self.debug_context {
            if span != Span::DUMMY {
                if let Ok(subprogram) = dc.create_function_at_offset(name_str, span.start) {
                    let func_val = self.builder.get_function_value(func_id);
                    dc.di().attach_function(func_val, subprogram);
                }
            }
        }

        self.codegen_ctx.functions.insert(name, (func_id, abi));
    }

    // -----------------------------------------------------------------------
    // Phase 2: Define
    // -----------------------------------------------------------------------

    // Monomorphized function support

    /// Declare monomorphized functions (phase 1).
    ///
    /// Each `MonoFunction` has a concrete (non-generic) `FunctionSig`, so the
    /// existing `declare_function` infrastructure works unchanged.
    pub fn declare_mono_functions(&mut self, mono_functions: &[crate::monomorphize::MonoFunction]) {
        for mono_fn in mono_functions {
            self.declare_function(mono_fn.mangled_name, &mono_fn.sig, Span::new(0, 0));

            // Build mono dispatch index: original_name → [(param_types, mangled_name)]
            self.codegen_ctx
                .mono_dispatch
                .entry(mono_fn.original_name)
                .or_default()
                .push((mono_fn.sig.param_types.clone(), mono_fn.mangled_name));
        }
    }

    /// Define a single function body via the ARC codegen pipeline.
    ///
    /// Runs: lower → borrow annotate → ARC pipeline → `ArcIrEmitter`.
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

    /// ARC IR → LLVM IR codegen (with RC lifecycle).
    ///
    /// Runs the full ARC pipeline: lower → liveness → RC insert → detect/expand
    /// reuse → RC eliminate → `ArcIrEmitter`. The emitter handles block creation,
    /// parameter binding, and return emission internally.
    ///
    /// When `type_subst` is `Some`, expression types from the canonical IR are
    /// substituted before ARC lowering — used for monomorphized generic functions.
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

        // Step 1: Lower canonical IR → ARC IR
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

    /// Shared post-lowering pipeline: apply borrows → compile lambdas →
    /// annotate arg ownership → ARC pipeline → emit LLVM IR.
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
        for mut lambda in lambdas {
            self.compile_lambda_arc(&mut lambda);
        }

        // Shared ARC processing: borrow annotations → arg ownership → pipeline
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
    /// A wrapper function bridging `(env_ptr, user_params...)` → flat call is
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

    // -----------------------------------------------------------------------
    // Shared ARC processing helpers
    // -----------------------------------------------------------------------

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
            &self.borrowing_builtins,
        );
        let arc_problems = ori_arc::run_arc_pipeline(
            arc_func,
            self.arc_classifier,
            self.annotated_sigs,
            self.pool,
            self.interner,
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
    /// `codegen_ctx.functions`) does NOT include the phantom param — it stays
    /// unchanged so `emit_function()` body emission works correctly.
    pub(super) fn declare_and_process_lambda(
        &mut self,
        lambda: &mut ori_arc::ArcFunction,
    ) -> (Name, FunctionId, FunctionAbi) {
        let lambda_name = lambda.name;
        let is_non_capturing = lambda.num_captures == 0;

        let mut abi = self.compute_arc_function_abi(lambda);

        // Non-capturing lambdas use `ccc` so they match the closure calling
        // convention directly: `(ptr %env, user_args...) -> ret`.
        if is_non_capturing {
            abi.call_conv = CallConv::C;
        }

        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let symbol = self
            .mangler
            .mangle_function(self.module_path, &format!("__lambda_{counter}"));

        debug!(
            name = %self.interner.lookup(lambda_name),
            symbol,
            params = abi.params.len(),
            non_capturing = is_non_capturing,
            "declaring lambda"
        );

        // Declare with phantom env param for non-capturing lambdas.
        // The emission ABI (registered below) does NOT include the phantom
        // param — emit_function() adjusts llvm_param_idx to skip it.
        let func_id = if is_non_capturing {
            let ptr_ty = self.builder.ptr_type();
            self.declare_function_llvm_with_extra_params(&symbol, &abi, &[ptr_ty])
        } else {
            self.declare_function_llvm(&symbol, &abi)
        };

        if is_non_capturing {
            self.codegen_ctx.non_capturing_lambdas.insert(lambda_name);
        }

        self.codegen_ctx
            .functions
            .insert(lambda_name, (func_id, abi.clone()));

        // ARC processing
        ori_arc::annotate_arg_ownership(
            lambda,
            self.annotated_sigs,
            self.interner,
            &self.borrowing_builtins,
        );
        let arc_problems = ori_arc::run_arc_pipeline(
            lambda,
            self.arc_classifier,
            self.annotated_sigs,
            self.pool,
            self.interner,
        );
        for problem in &arc_problems {
            debug!(?problem, "ARC pipeline problem (lambda)");
        }

        (lambda_name, func_id, abi)
    }

    /// Check if an ARC function is nounwind (cannot unwind/panic).
    ///
    /// A function is nounwind if:
    /// 1. All `Invoke` callees are already known-nounwind (in the set), AND
    /// 2. No `Apply` calls a may-unwind runtime function (`ori_panic*`), AND
    /// 3. No `ApplyIndirect` instructions exist (indirect calls through
    ///    closures/function pointers are conservatively may-unwind).
    ///
    /// `ori_panic` is the sole runtime function that unwinds — it uses Rust's
    /// panic infrastructure. All other `ori_*` / `__*` runtime functions are
    /// nounwind (abort on failure or never fail).
    ///
    /// Indirect calls (`ApplyIndirect`) cannot be statically resolved to a
    /// known callee, so we must conservatively assume they may unwind. This
    /// prevents UB when a closure target panics inside a `nounwind` function.
    pub(super) fn is_arc_function_nounwind(&self, func: &ori_arc::ArcFunction) -> bool {
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
                    !s.starts_with("ori_panic")
                }
                // Indirect calls through closures/function pointers are
                // conservatively treated as may-unwind — we cannot know
                // the callee's unwind behavior at compile time.
                ori_arc::ir::ArcInstr::ApplyIndirect { .. } => false,
                _ => true,
            });
            term_ok && instrs_ok
        })
    }

    /// Enter debug scope for the function being compiled.
    pub(super) fn enter_debug_scope(&self, func_id: FunctionId) {
        if let Some(dc) = self.debug_context {
            let func_val = self.builder.get_function_value(func_id);
            if let Some(subprogram) = func_val.get_subprogram() {
                dc.enter_function(subprogram);
            }
        }
    }

    /// Exit debug scope after function compilation.
    pub(super) fn exit_debug_scope(&self) {
        if let Some(dc) = self.debug_context {
            dc.exit_function();
        }
    }

    /// Emit the return instruction based on ABI passing mode.
    pub(crate) fn emit_return(
        &mut self,
        func_id: FunctionId,
        abi: &FunctionAbi,
        result: Option<ValueId>,
        name_str: &str,
    ) {
        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                if let Some(val) = result {
                    let sret_ptr = self.builder.get_param(func_id, 0);
                    self.builder.store(val, sret_ptr);
                }
                self.builder.ret_void();
            }
            ReturnPassing::Direct => {
                if let Some(val) = result {
                    self.builder.ret(val);
                } else {
                    warn!(name = name_str, "direct return function produced no value");
                    self.builder.record_codegen_error();
                    self.builder.ret_void();
                }
            }
            ReturnPassing::Void => {
                self.builder.ret_void();
            }
        }
    }

    /// Load all parameter values from an LLVM function, respecting ABI passing.
    ///
    /// Returns one `ValueId` per non-Void parameter in ABI order. Direct params
    /// are returned as-is; Indirect/Reference params are loaded from their
    /// pointers. Does not set value names or bind to scope — callers handle that.
    pub(super) fn load_param_values(
        &mut self,
        func_id: FunctionId,
        abi: &FunctionAbi,
    ) -> Vec<ValueId> {
        let has_sret = matches!(abi.return_abi.passing, ReturnPassing::Sret { .. });
        let mut llvm_idx: u32 = u32::from(has_sret);
        let mut values = Vec::with_capacity(abi.params.len());

        for (i, param) in abi.params.iter().enumerate() {
            match &param.passing {
                ParamPassing::Direct => {
                    values.push(self.builder.get_param(func_id, llvm_idx));
                    llvm_idx += 1;
                }
                ParamPassing::Indirect { .. } => {
                    let ptr = self.builder.get_param(func_id, llvm_idx);
                    let ty = self.type_resolver.resolve(param.ty);
                    let ty_id = self.builder.register_type(ty);
                    // IrBuilder::load() auto-decomposes struct types via
                    // per-field GEP+load+insert_value (FastISel safety).
                    values.push(self.builder.load(ty_id, ptr, &format!("param.{i}")));
                    llvm_idx += 1;
                }
                ParamPassing::Reference => {
                    let ptr = self.builder.get_param(func_id, llvm_idx);
                    let ty = self.type_resolver.resolve(param.ty);
                    let ty_id = self.builder.register_type(ty);
                    values.push(self.builder.load(ty_id, ptr, &format!("param.{i}")));
                    llvm_idx += 1;
                }
                ParamPassing::Void => {}
            }
        }

        values
    }

    /// Declare external imported functions (for multi-module AOT compilation).
    pub fn declare_imports(&mut self, imports: &[(Name, FunctionSig)]) {
        for (name, sig) in imports {
            self.declare_function(*name, sig, Span::DUMMY);
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Look up a declared function by name.
    pub fn get_function(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        self.codegen_ctx.functions.get(&name)
    }

    /// Borrow the function map (for call-site ABI lookup).
    pub fn function_map(&self) -> &FxHashMap<Name, (FunctionId, FunctionAbi)> {
        &self.codegen_ctx.functions
    }

    /// Borrow the type-qualified method map.
    pub fn method_function_map(&self) -> &FxHashMap<(Name, Name), (FunctionId, FunctionAbi)> {
        &self.codegen_ctx.method_functions
    }

    /// Borrow the type index → type name mapping.
    pub fn type_idx_to_name_map(&self) -> &FxHashMap<Idx, Name> {
        &self.codegen_ctx.type_idx_to_name
    }

    // -----------------------------------------------------------------------
    // Derive Codegen Accessors (pub(crate))
    // -----------------------------------------------------------------------

    /// Mutable borrow of the `IrBuilder`.
    pub(crate) fn builder_mut(&mut self) -> &mut IrBuilder<'scx, 'ctx> {
        self.builder
    }

    /// Create an alloca at the function entry block.
    ///
    /// Entry-block placement ensures LLVM's frame lowering accounts for the
    /// alloca during prologue emission. Allocas interleaved with calls can
    /// cause stack corruption in `fastcc` functions at O0 (LLVM `FastISel`
    /// miscalculates stack adjustments).
    pub(crate) fn entry_alloca(&mut self, ty: LLVMTypeId, name: &str) -> ValueId {
        let func = self
            .builder
            .current_function
            .expect("entry_alloca called without current function");
        self.builder.create_entry_alloca(func, name, ty)
    }

    /// Borrow the type info store.
    pub(crate) fn type_info(&self) -> &TypeInfoStore<'tcx> {
        self.type_info
    }

    /// Resolve a type Idx to its LLVM representation.
    pub(crate) fn resolve_type(&self, idx: Idx) -> inkwell::types::BasicTypeEnum<'ctx> {
        self.type_resolver.resolve(idx)
    }

    /// Look up an interned name.
    pub(crate) fn lookup_name(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    /// Intern a string.
    pub(crate) fn intern(&self, s: &str) -> Name {
        self.interner.intern(s)
    }

    /// Generate a mangled method symbol.
    pub(crate) fn mangle_method(&self, type_name: &str, method_name: &str) -> String {
        self.mangler
            .mangle_method(self.module_path, type_name, method_name)
    }

    /// Look up a type name from a type Idx.
    pub(crate) fn type_idx_to_name(&self, idx: Idx) -> Option<Name> {
        self.codegen_ctx.type_idx_to_name.get(&idx).copied()
    }

    /// Look up a method function by type and method name.
    pub(crate) fn get_method_function(
        &self,
        type_name: Name,
        method_name: Name,
    ) -> Option<(FunctionId, FunctionAbi)> {
        self.codegen_ctx
            .method_functions
            .get(&(type_name, method_name))
            .cloned()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::doc_markdown,
    clippy::default_trait_access,
    reason = "test code — style relaxed for clarity"
)]
mod tests;
