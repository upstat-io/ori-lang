//! Two-pass function compiler state and artifact binding.
//!
//! `FunctionCompiler` declares the complete callable surface before defining bodies.
//!
//! ## Compilation phases
//!
//! 1. **Declare**: Compute each `FunctionAbi` and declare LLVM symbols.
//! 2. **Define**: Lower canonical expressions through ARC IR into LLVM IR.
//!
//! ## Submodules
//!
//! - `declarations` declares functions and implementation methods in Phase 1.
//! - `define_phase` defines function bodies and runs ARC processing in Phase 2.
//! - `nounwind` prepares, analyzes, and emits two-pass nounwind facts.
//! - `impls` compiles implementation methods, tests, and derived traits.
//! - `entry_point` emits the AOT `main` wrapper.
//! - `seh_main_thunk` emits the SEH/MSVC `ori_try_call` thunk for `@main(args:)`.
//! - `panic_trampoline` emits the `_ori_panic_trampoline` handler.

use super::{
    AnnotatedSig, ArcClassifier, CodegenContext, DebugContext, FxHashMap, Idx, IrBuilder, Mangler,
    MemoryContract, Name, Pool, StringInterner, TypeInfoStore, TypeLayoutResolver,
};

// Env: ORI_DISABLE_RL31_NOALIAS — omits RL-31 `noalias` projection, debug-only.
// INVARIANT: The process-cached value is shared by every function declaration.
static RL31_NOALIAS_DISABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    report_rl31_noalias_toggle(std::env::var_os("ORI_DISABLE_RL31_NOALIAS").is_some())
});

/// Two-pass function compiler.
///
/// Holds the mapping from function `Name` to `(FunctionId, FunctionAbi)`,
/// enabling call sites to look up the callee's ABI for correct argument
/// passing (direct vs. sret).
pub struct FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// ID-based LLVM instruction builder.
    pub(super) builder: &'a mut IrBuilder<'scx, 'ctx>,
    /// Cache of physical type information.
    pub(super) type_info: &'a TypeInfoStore<'tcx>,
    /// Recursive type layout resolver.
    pub(super) type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
    /// Interner for source and symbol names.
    pub(super) interner: &'a StringInterner,
    /// Canonical type pool for structural queries.
    pub(super) pool: &'tcx Pool,
    /// Symbol mangler for generating unique LLVM symbol names.
    pub(super) mangler: Mangler,
    /// Module path for name mangling (e.g., "", "math", "data/utils").
    pub(super) module_path: &'a str,
    /// Shared function-resolution lookup tables passed to [`ArcIrEmitter`].
    pub(super) codegen_ctx: CodegenContext,
    /// Borrow inference results: function `Name` maps to its annotated signature.
    /// `Ownership::Borrowed` + non-Scalar parameters use
    /// `ParamPassing::Reference` (pointer, no RC at call site).
    pub(super) annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    /// Type classifier for ARC analysis (scalar vs ref classification).
    pub(super) arc_classifier: &'a ArcClassifier<'tcx>,
    /// Debug info context (None for JIT, Some for AOT with debug info enabled).
    pub(super) debug_context: Option<&'a DebugContext<'ctx>>,
    /// Frozen AIMS contracts used only for physical attribute projection.
    /// This map can only be populated from the closed executable artifact.
    pub(super) aims_contracts: FxHashMap<Name, MemoryContract>,
    /// Closed backend-neutral facts consumed by the physical LLVM projection.
    /// Production body emission fails closed when this is absent.
    pub(super) executable_program: Option<&'a ori_repr::executable::ExecutableProgram>,
    /// Qualified ordinary callees mapped to private clone identity and returned yield result.
    pub(super) length_projection_clones: FxHashMap<Name, (Name, ori_arc::ArcVarId)>,
    /// Qualified call sites whose ordinary callees require clone declarations.
    pub(super) length_projection_calls: FxHashMap<(Name, ori_arc::ArcVarId), Name>,
    /// Whether to run ARC IR verification in release builds.
    /// In debug builds, verification always runs regardless of this flag.
    pub(super) verify_arc: bool,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// Creates a function compiler for `module_path`. Annotated signatures and
    /// ARC classification select reference passage for borrowed, non-scalar
    /// parameters; the module path supplies symbol mangling context.
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
            executable_program: None,
            length_projection_clones: FxHashMap::default(),
            length_projection_calls: FxHashMap::default(),
            verify_arc,
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
        self.length_projection_calls = program.repr_plan().length_projection_calls().collect();
        self.length_projection_clones = program
            .repr_plan()
            .length_projection_yields()
            .map(|(callee, result)| {
                (
                    callee,
                    (
                        super::length_projection::projection_name(self.interner, callee),
                        result,
                    ),
                )
            })
            .collect();
        self.aims_contracts.clear();
        self.codegen_ctx.closure_adapters.clear();
        self.codegen_ctx.user_drop_functions.clear();
        self.codegen_ctx.exact_method_functions.clear();
        self.codegen_ctx.executable_call_targets.clear();
        self.codegen_ctx.length_projection_call_targets.clear();
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
                    self.bind_executable_call_target(program, function_id, function.name, *dst);
                }
                if let ori_arc::ArcTerminator::Invoke { dst, .. } = &block.terminator {
                    self.bind_executable_call_target(program, function_id, function.name, *dst);
                }
            }
        }
        self.codegen_ctx.retain_plans = program.retain_plans().clone();
        self.codegen_ctx.executable_facts_bound = true;
    }

    fn bind_executable_call_target(
        &mut self,
        program: &ori_repr::executable::ExecutableProgram,
        function_id: ori_repr::executable::FunctionId,
        function_name: Name,
        destination: ori_arc::ArcVarId,
    ) {
        let Some(target) = program.direct_call_target(function_id, destination) else {
            unreachable!("validated direct call has no executable target");
        };
        if self
            .codegen_ctx
            .executable_call_targets
            .insert((function_name, destination), target)
            .is_some()
        {
            unreachable!("validated direct call destination is duplicated");
        }
    }

    /// Bind the artifact's exact receiver-qualified method census to declared LLVM callables.
    ///
    /// Ordinary ARC calls already carry a frozen per-register target. Compound
    /// builtin emission can synthesize a nested method call after ARC closure,
    /// so it consults this equally frozen semantic table instead of rebuilding
    /// dispatch from source declarations or the open-world derived-method path.
    pub fn bind_executable_method_targets(&mut self) {
        self.codegen_ctx.exact_method_functions.clear();
        let Some(program) = self.executable_program else {
            return;
        };

        for (receiver, method, target) in program.method_targets() {
            let target_name = match target {
                ori_repr::executable::CallableTarget::Function(function) => {
                    program.function(function).name
                }
                ori_repr::executable::CallableTarget::External(function) => {
                    program.external_function(function).name()
                }
                ori_repr::executable::CallableTarget::Runtime(operation) => {
                    self.builder.record_codegen_error_with_msg(format!(
                        "closed executable method target for receiver {receiver:?} and `{}` resolved to runtime operation {operation:?}; rerun with ORI_VERIFY_ARC=1 and report this compiler bug",
                        self.interner.lookup(method),
                    ));
                    continue;
                }
            };
            let Some((function, abi)) = self.codegen_ctx.functions.get(&target_name).cloned()
            else {
                self.builder.record_codegen_error_with_msg(format!(
                    "closed executable method target `{}` for receiver {receiver:?} projects to undeclared callable `{}`; rerun with ORI_VERIFY_ARC=1 and report this compiler bug",
                    self.interner.lookup(method),
                    self.interner.lookup(target_name),
                ));
                continue;
            };
            self.codegen_ctx
                .exact_method_functions
                .insert((receiver, method), (function, abi));
        }
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
}

// Why: Debug output omits the LLVM builder and type services to avoid recursive context dumps.
impl std::fmt::Debug for FunctionCompiler<'_, '_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionCompiler")
            .field("module_path", &self.module_path)
            .field("annotated_sig_count", &self.annotated_sigs.len())
            .field("aims_contract_count", &self.aims_contracts.len())
            .field(
                "length_projection_clone_count",
                &self.length_projection_clones.len(),
            )
            .field(
                "length_projection_call_count",
                &self.length_projection_calls.len(),
            )
            .field(
                "executable_program_bound",
                &self.executable_program.is_some(),
            )
            .field("verify_arc", &self.verify_arc)
            .finish_non_exhaustive()
    }
}

/// Read the process-cached RL-31 `noalias` ablation toggle.
pub(in crate::codegen) fn rl31_noalias_disabled() -> bool {
    *RL31_NOALIAS_DISABLED
}

/// Report whether RL-31 `noalias` projection is disabled.
fn report_rl31_noalias_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_RL31_NOALIAS",
            effect = "omit LLVM projection of RL-31 parameter facts",
            "ablation toggle fired"
        );
    }
    disabled
}
