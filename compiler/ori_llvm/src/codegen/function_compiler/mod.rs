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
//! - `define_phase`: Function body definition (Phase 2) and ARC processing
//! - `nounwind`: Two-pass nounwind analysis (prepare → analyze → emit)
//! - `impls`: Impl method, test, and derived trait compilation
//! - `entry_point`: AOT `main` wrapper
//! - `seh_main_thunk`: SEH/MSVC `ori_try_call` thunk for `@main(args:)`
//! - `panic_trampoline`: Panic handler trampoline (`_ori_panic_trampoline`)

mod accessors;
mod artifact_projection;
mod define_phase;
mod derive_methods;
mod effect_projection;
mod entry_point;
mod error_ctor;
mod impls;
mod lambda_rewrite;
mod nounwind;
mod panic_trampoline;
mod return_projection;
mod rl31_projection;
mod seh_main_thunk;
mod shared_seam;
mod test_wrappers;

pub use nounwind::PreparedFunction;

use ori_arc::{AnnotatedSig, ArcClassifier, MemoryContract};
use ori_ir::{Function, Name, Span, StringInterner};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use crate::aot::debug::DebugContext;
use crate::aot::mangle::Mangler;

use super::abi::{
    abi_size, compute_function_abi, compute_function_abi_with_ownership, CallConv, FunctionAbi,
    ParamPassing, ReturnPassing,
};
use super::arc_emitter::CodegenContext;
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

/// Process-cached `ORI_DISABLE_RL31_NOALIAS=1` flag.
///
/// Read once at first access; reused for every function declaration.
/// `true` omits the RL-31 param `noalias` emission (diagnostic bisection).
// Env: ORI_DISABLE_RL31_NOALIAS — omits RL-31 param noalias emission for AIMS-noalias bisection, debug-only
static RL31_NOALIAS_DISABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    let disabled = std::env::var_os("ORI_DISABLE_RL31_NOALIAS").is_some();
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_RL31_NOALIAS",
            effect = "omit LLVM projection of RL-31 parameter facts",
            "ablation toggle fired"
        );
    }
    disabled
});

/// Two-pass function compiler.
///
/// Holds the mapping from function `Name` → `(FunctionId, FunctionAbi)`,
/// enabling call sites to look up the callee's ABI for correct argument
/// passing (direct vs. sret).
pub struct FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    type_info: &'a TypeInfoStore<'tcx>,
    type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
    interner: &'a StringInterner,
    pool: &'tcx Pool,
    /// Symbol mangler for generating unique LLVM symbol names.
    mangler: Mangler,
    /// Module path for name mangling (e.g., "", "math", "data/utils").
    module_path: &'a str,
    /// Shared function-resolution lookup tables passed to [`ArcIrEmitter`].
    codegen_ctx: CodegenContext,
    /// Borrow inference results: function `Name` → annotated signature.
    /// `Ownership::Borrowed` + non-Scalar parameters use
    /// `ParamPassing::Reference` (pointer, no RC at call site).
    annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    /// Type classifier for ARC analysis (scalar vs ref classification).
    arc_classifier: &'a ArcClassifier<'tcx>,
    /// Debug info context (None for JIT, Some for AOT with debug info enabled).
    debug_context: Option<&'a DebugContext<'ctx>>,
    /// Frozen AIMS contracts used only for physical attribute projection.
    /// This map can only be populated from the closed executable artifact.
    aims_contracts: FxHashMap<Name, MemoryContract>,
    /// Whether to run ARC IR verification in release builds.
    /// In debug builds, verification always runs regardless of this flag.
    verify_arc: bool,
    /// Closed backend-neutral facts consumed by the physical LLVM projection.
    /// Production body emission fails closed when this is absent.
    executable_program: Option<&'a ori_repr::executable::ExecutableProgram>,
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
        type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
        interner: &'a StringInterner,
        pool: &'tcx Pool,
        module_path: &'a str,
        annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
        arc_classifier: &'a ArcClassifier<'tcx>,
        debug_context: Option<&'a DebugContext<'ctx>>,
        verify_arc: bool,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            mangler: Mangler::new(),
            module_path,
            codegen_ctx: CodegenContext::default(),
            annotated_sigs,
            arc_classifier,
            debug_context,
            aims_contracts: FxHashMap::default(),
            verify_arc,
            executable_program: None,
        }
    }

    /// Bind the closed shared artifact that owns backend-neutral AIMS facts.
    ///
    /// Return, effect, and parameter attributes are projected during emission. Omitting
    /// this binding is conservative and never triggers backend-local analysis.
    pub fn bind_executable_program(
        &mut self,
        program: &'a ori_repr::executable::ExecutableProgram,
    ) {
        self.executable_program = Some(program);
        self.aims_contracts.clear();
        self.codegen_ctx.closure_adapters.clear();
        self.codegen_ctx.user_drop_functions.clear();
        self.codegen_ctx.executable_call_targets.clear();
        self.codegen_ctx.executable_function_names = program
            .functions()
            .iter()
            .map(|function| function.name)
            .collect();
        self.codegen_ctx.executable_external_names = program
            .external_functions()
            .iter()
            .map(ori_repr::executable::ExternalCallable::name)
            .collect();
        for function in program.functions() {
            let Some(function_id) = program.function_id(function.name) else {
                unreachable!("validated executable function has no stable identity");
            };
            self.aims_contracts.insert(
                function.name,
                program.function_contract(function_id).clone(),
            );
            if let Some(adapter) = program.closure_adapter(function_id) {
                self.codegen_ctx
                    .closure_adapters
                    .insert(function.name, adapter.clone());
            }
            for block in &function.blocks {
                for instruction in &block.body {
                    let ori_arc::ArcInstr::Apply { dst, .. } = instruction else {
                        continue;
                    };
                    let Some(target) = program.direct_call_target(function_id, *dst) else {
                        unreachable!("validated direct Apply has no executable target");
                    };
                    if self
                        .codegen_ctx
                        .executable_call_targets
                        .insert((function.name, *dst), target)
                        .is_some()
                    {
                        unreachable!("validated direct call destination is duplicated");
                    }
                }
                if let ori_arc::ArcTerminator::Invoke { dst, .. } = &block.terminator {
                    let Some(target) = program.direct_call_target(function_id, *dst) else {
                        unreachable!("validated direct Invoke has no executable target");
                    };
                    if self
                        .codegen_ctx
                        .executable_call_targets
                        .insert((function.name, *dst), target)
                        .is_some()
                    {
                        unreachable!("validated direct call destination is duplicated");
                    }
                }
            }
        }
        self.codegen_ctx.retain_plans = program.retain_plans().clone();
        self.codegen_ctx.executable_facts_bound = true;
    }

    /// Bind each artifact user-drop operation to its declared physical callable.
    ///
    /// The executable plan owns semantic identity and exact target selection. This
    /// projection runs only after impl declarations exist, and deliberately does
    /// not rediscover `Drop` implementations through the general method map.
    pub fn bind_user_drop_targets(&mut self) {
        self.codegen_ctx.user_drop_functions.clear();
        let Some(program) = self.executable_program else {
            return;
        };

        for operation in program.user_drop_plan().entries() {
            let target_name = program.functions()[operation.target().index()].name;
            let Some((function, abi)) = self.codegen_ctx.functions.get(&target_name).cloned()
            else {
                self.builder.record_codegen_error_with_msg(format!(
                    "closed executable user-drop target {target_name:?} was not declared"
                ));
                continue;
            };

            let canonical = self.pool.resolve_fully(operation.ty());
            let signature_matches = abi.params.len() == 1
                && self.pool.resolve_fully(abi.params[0].ty) == canonical
                && self.pool.resolve_fully(abi.return_abi.ty) == Idx::UNIT;
            if !signature_matches {
                self.builder.record_codegen_error_with_msg(format!(
                    "closed executable user-drop target {target_name:?} has a physical ABI inconsistent with fn(Self) -> unit"
                ));
                continue;
            }

            self.codegen_ctx
                .user_drop_functions
                .insert(operation.ty(), (function, abi.clone()));
            self.codegen_ctx
                .user_drop_functions
                .insert(canonical, (function, abi));
        }
    }

    // Phase 1: Declare

    /// Declare all module functions from type checker signatures.
    ///
    /// Iterates over `module.functions` paired with their `FunctionSig` from the
    /// type checker. Generic functions are skipped (they require monomorphization).
    pub fn declare_all(&mut self, module_functions: &[Function], function_sigs: &[FunctionSig]) {
        // Build a name→sig lookup. function_sigs is deduped by name (one per
        // unique function). Multi-clause functions have multiple entries in
        // module_functions but only one sig — lookup is by name to avoid
        // positional misalignment.
        let sig_map: rustc_hash::FxHashMap<Name, &FunctionSig> =
            function_sigs.iter().map(|s| (s.name, s)).collect();

        let mut seen = rustc_hash::FxHashSet::default();
        for func in module_functions {
            // Skip duplicate clause declarations (multi-clause functions).
            if !seen.insert(func.name) {
                continue;
            }

            let Some(sig) = sig_map.get(&func.name) else {
                continue;
            };

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

        self.declare_error_constructor();
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
            // sret pointer is a fresh caller alloca — safe for noalias.
            // Do NOT add noalias to regular ptr params — RC buffers can alias
            // (e.g., `f(a: xs, b: xs)`). Only sret, ori_rc_alloc returns, and
            // COW StaticUnique call sites qualify.
            self.builder.add_noalias_attribute(func_id, 0);
        }

        // uwtable: required for stack unwinding on all EH-capable targets.
        // See `ir_builder/attributes.rs` for full rationale.
        self.builder.add_uwtable_attribute(func_id);

        // noundef on all non-Void params — Ori values are always fully defined.
        // Direct params: the value itself is noundef (no poison/undef).
        // Indirect/Reference params: the pointer is noundef (always a valid,
        // defined address — never poison or undef). Also add readonly for borrowed.
        //
        // Extra leading params (e.g., phantom env ptr for non-capturing lambdas)
        // also get noundef — a null pointer is still a defined value (not undef/poison).
        let sret_offset = u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));
        for (i, _) in extra_leading_params.iter().enumerate() {
            self.builder
                .add_noundef_param_attribute(func_id, sret_offset + i as u32);
        }
        let mut nidx = sret_offset + extra_leading_params.len() as u32;
        for param in &abi.params {
            if matches!(param.passing, ParamPassing::Direct) {
                self.builder.add_noundef_param_attribute(func_id, nidx);
                nidx += 1;
            } else if !matches!(param.passing, ParamPassing::Void) {
                // Indirect/Reference pointer params: noundef (pointer is defined),
                // nonnull (Ori never passes null pointers), dereferenceable(N)
                // (pointer points to at least N bytes of valid memory),
                // + readonly if borrowed.
                self.builder.add_noundef_param_attribute(func_id, nidx);
                self.builder.add_nonnull_param_attribute(func_id, nidx);
                // dereferenceable(N): abi_size may underestimate due to missing
                // alignment padding; underestimation is legal — LLVM treats
                // dereferenceable as a minimum guarantee.
                let size = abi_size(param.ty, self.type_info, self.repr_plan());
                if size > 0 {
                    self.builder
                        .add_dereferenceable_param_attribute(func_id, nidx, size);
                }
                if param.readonly {
                    self.builder.add_readonly_param_attribute(func_id, nidx);
                }
                nidx += 1;
            }
        }
        if matches!(abi.return_abi.passing, ReturnPassing::Direct) {
            self.builder.add_noundef_return_attribute(func_id);
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
        let (func_id, abi) = self.declare_impl_method(name, symbol, sig, span);
        self.codegen_ctx.functions.insert(name, (func_id, abi));
    }

    /// Declare an impl method LLVM function without registering it in the bare
    /// `functions` map.
    ///
    /// Impl methods must be resolved via the type-qualified `method_functions`
    /// map (keyed by `(type_name, method_name)`). Inserting them into the bare
    /// `functions` map (keyed by `method_name` alone) causes wrong-function calls:
    /// when `Box$to_str` is registered under the bare key `to_str`, a later call
    /// to `to_str` on an `int` field inside `Box$to_str`'s body resolves to the
    /// struct method instead of the primitive method.
    ///
    /// Use this for all impl methods (inherent and trait). The caller is
    /// responsible for inserting into `method_functions` and `type_idx_to_name`.
    pub(super) fn declare_impl_method(
        &mut self,
        name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) -> (FunctionId, FunctionAbi) {
        self.declare_impl_method_with_fact_name(name, name, symbol, sig, span)
    }

    /// Declare an impl method whose realized callable facts use a distinct,
    /// type-qualified identity.
    ///
    /// `source_name` remains the user-facing method name used for diagnostics
    /// and debug information. `fact_name` selects the exact ownership contract
    /// frozen into the bound executable artifact.
    pub(super) fn declare_impl_method_with_fact_name(
        &mut self,
        source_name: Name,
        fact_name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) -> (FunctionId, FunctionAbi) {
        let name_str = self.interner.lookup(source_name);

        let abi = compute_function_abi_with_ownership(
            sig,
            self.type_info,
            self.annotated_sigs.get(&fact_name),
            self.arc_classifier,
            self.repr_plan(),
        );

        debug!(
            name = name_str,
            symbol,
            params = abi.params.len(),
            call_conv = ?abi.call_conv,
            return_passing = ?abi.return_abi.passing,
            "declaring impl method"
        );

        let func_id = self.declare_function_llvm(symbol, &abi);

        if let Some(dc) = self.debug_context {
            if span != Span::DUMMY {
                match dc.create_function_at_offset(name_str, span.start) {
                    Ok(subprogram) => {
                        let func_val = self.builder.get_function_value(func_id);
                        dc.di().attach_function(func_val, subprogram);
                    }
                    Err(err) => {
                        // Debug info is best-effort; the function still
                        // compiles, but the miss must be visible.
                        tracing::warn!(name = name_str, ?err, "debug info attachment failed");
                    }
                }
            }
        }

        (func_id, abi)
    }
}

#[cfg(test)]
#[expect(
    clippy::doc_markdown,
    clippy::default_trait_access,
    reason = "test code — style relaxed for clarity"
)]
mod tests;
