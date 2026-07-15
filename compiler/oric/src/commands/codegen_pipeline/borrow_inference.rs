//! Complete pre-AIMS ARC lowering for the codegen pipeline.
//!
//! Lowers every function (local, mono, imported mono) to ARC IR. The resulting
//! batch is consumed once by backend-neutral executable realization; LLVM does
//! not run a second ownership calculus over these bodies.

#[cfg(feature = "llvm")]
use ori_ir::canon::CanonResult;
#[cfg(feature = "llvm")]
use ori_types::{FunctionSig, Pool};
#[cfg(feature = "llvm")]
use oric::ir::StringInterner;
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use rustc_hash::FxHashSet;

#[cfg(feature = "llvm")]
use super::imported_mono::ImportedSurfaces;

/// Complete pre-AIMS ARC families and their monomorphized emission inventory.
///
#[cfg(feature = "llvm")]
pub(super) struct ArcBatchLoweringResult {
    /// Pre-lowered ARC parent/lambda families before shared preparation.
    pub(super) groups: Vec<crate::realization::ArcFunctionGroup>,
    /// Monomorphized generic functions (reused by codegen to avoid recomputation).
    pub(super) mono_functions: Vec<ori_repr::monomorphize::MonoFunction>,
}

/// Failure before closed executable realization can consume the ARC batch.
#[cfg(feature = "llvm")]
pub(super) enum ArcBatchLoweringFailure {
    /// ARC lowering emitted its structured diagnostics.
    ArcLowering,
    /// Final mono identities could not be bound to one source namespace.
    MonoInventory(crate::realization::MonoFunctionInventoryError),
}

/// Lower every non-generic, specialized, and imported-specialized body.
///
/// Lowers each function (local, mono, imported mono) to ARC IR. Returns one
/// cache of pre-lowered ARC functions for zero-copy consumption by
/// shared executable realization and subsequent backend projection.
/// ARC lowering diagnostics are emitted here. The returned lowered state has
/// not crossed the shared specialization and target-closure seam.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline helper — distinct data flow inputs per compilation stage"
)]
#[expect(
    clippy::too_many_lines,
    reason = "pipeline driver composing local, mono, and imported-mono lowering loops"
)]
pub(super) fn lower_arc_batch(
    parse_result: &ParseOutput,
    function_sigs: &[FunctionSig],
    impl_sigs: &[ori_types::ImplSig],
    import_sigs: &[ori_repr::monomorphize::ImportSig],
    imported: ImportedSurfaces<'_>,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &mut Pool,
    mono_instances: &[ori_types::MonoInstance],
    accepted_derives: &[ori_types::AcceptedDerivedImpl],
) -> Result<ArcBatchLoweringResult, ArcBatchLoweringFailure> {
    let ImportedSurfaces {
        imported_mono_fns,
        re_interned_canons,
    } = imported;

    let mut groups = Vec::new();
    let mut arc_problems = Vec::new();

    // Why: the PC-2 exempt set is empty — pre-mono skips generics via
    // sig.is_generic(); mono instances are fully substituted (empty scheme_var_ids).
    let exempt: FxHashSet<u32> = FxHashSet::default();

    for (func, sig) in parse_result
        .module
        .functions
        .iter()
        .zip(function_sigs.iter())
    {
        if sig.is_generic() {
            continue;
        }
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            func.name,
            sig,
            func.name,
            canon,
            interner,
            pool,
            &mut arc_problems,
            None,
        );
        super::pc2_hooks::run_pc2_hook_aot(
            pool,
            &arc_fn,
            &lambdas,
            interner,
            &exempt,
            "aot_pre_mono",
            "aot_pre_mono_lambda",
        );
        groups.push(crate::realization::ArcFunctionGroup::new(arc_fn, lambdas));
    }

    // Lower monomorphized generic functions.
    // Why: without mono entries in annotated_sigs, borrow lookup falls back to all-Owned.
    let mono_functions = ori_repr::monomorphize::collect_mono_functions(
        mono_instances,
        function_sigs,
        impl_sigs,
        accepted_derives,
        import_sigs,
        interner,
        pool,
    );
    let mono_inventory = crate::realization::MonoFunctionInventory::try_new(
        mono_functions,
        imported_mono_fns
            .iter()
            .map(|(function, _, _)| function.clone()),
        interner,
    )
    .map_err(ArcBatchLoweringFailure::MonoInventory)?;
    for mono_fn in mono_inventory.local_bodies() {
        let Some(group) = crate::realization::lower_mono_function_for_analysis(
            mono_fn,
            accepted_derives,
            canon,
            interner,
            pool,
            &mut arc_problems,
        ) else {
            continue;
        };
        let (arc_fn, lambdas) = group.into_parts();
        super::pc2_hooks::run_pc2_hook_aot(
            pool,
            &arc_fn,
            &lambdas,
            interner,
            &exempt,
            "aot_mono",
            "aot_mono_lambda",
        );
        groups.push(crate::realization::ArcFunctionGroup::new(arc_fn, lambdas));
    }

    // Lower imported monos via body-import linkage; an out-of-bounds
    // source_module_idx falls back to the host canon.
    // Why: the generic body lives in the SOURCE module's re-interned canon.
    for (mono_fn, source_module_idx, source_body_name) in imported_mono_fns {
        let source_canon = re_interned_canons.get(*source_module_idx).unwrap_or(canon);
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            mono_fn.mangled_name,
            &mono_fn.sig,
            *source_body_name,
            source_canon,
            interner,
            pool,
            &mut arc_problems,
            Some(&mono_fn.body_type_map),
        );
        super::pc2_hooks::run_pc2_hook_aot(
            pool,
            &arc_fn,
            &lambdas,
            interner,
            &exempt,
            "aot_imported_mono",
            "aot_imported_mono_lambda",
        );
        groups.push(crate::realization::ArcFunctionGroup::new(arc_fn, lambdas));
    }

    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return Err(ArcBatchLoweringFailure::ArcLowering);
        }
    }

    Ok(ArcBatchLoweringResult {
        groups,
        mono_functions: mono_inventory.into_all(),
    })
}
