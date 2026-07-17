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
//! - `declarations`: Function and impl-method declaration (Phase 1)
//! - `define_phase`: Function body definition (Phase 2) and ARC processing
//! - `nounwind`: Two-pass nounwind analysis (prepare → analyze → emit)
//! - `impls`: Impl method, test, and derived trait compilation
//! - `entry_point`: AOT `main` wrapper
//! - `seh_main_thunk`: SEH/MSVC `ori_try_call` thunk for `@main(args:)`
//! - `panic_trampoline`: Panic handler trampoline (`_ori_panic_trampoline`)

mod accessors;
mod artifact_projection;
mod declarations;
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
#[cfg(test)]
use ori_ir::Function;
use ori_ir::{Name, Span, StringInterner};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::warn;

use crate::aot::debug::DebugContext;
use crate::aot::mangle::Mangler;

use super::abi::{compute_function_abi, FunctionAbi, ParamPassing, ReturnPassing};
use super::arc_emitter::CodegenContext;
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

#[cfg(test)]
use super::abi::CallConv;

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
}

#[cfg(test)]
#[expect(clippy::doc_markdown, reason = "test code — style relaxed for clarity")]
mod tests;
