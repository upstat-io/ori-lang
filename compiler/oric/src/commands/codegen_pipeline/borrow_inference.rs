//! ARC borrow inference for the codegen pipeline.
//!
//! Lowers every function (local, mono, imported mono) to ARC IR and runs the
//! per-SCC Salsa-tracked borrow inference queries, producing the annotated
//! signatures plus a cache of pre-lowered `ArcFunction`s codegen consumes.

#[cfg(feature = "llvm")]
use ori_ir::canon::CanonResult;
#[cfg(feature = "llvm")]
use ori_types::{FunctionSig, Pool};
#[cfg(feature = "llvm")]
use oric::ir::{Name, StringInterner};
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db};
#[cfg(feature = "llvm")]
use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(feature = "llvm")]
use super::imported_mono::ImportedSurfaces;

/// Result of borrow inference: annotated signatures + pre-lowered ARC cache.
///
/// The `arc_cache` contains pre-lowered `ArcFunction`s grouped by parent
/// function. These are consumed by `prepare_all_cached` during codegen,
/// eliminating the redundant second lowering pass.
#[cfg(feature = "llvm")]
pub(super) struct BorrowInferenceResult {
    /// Borrow-annotated function signatures from SCC analysis.
    pub(super) sigs: FxHashMap<Name, ori_arc::AnnotatedSig>,
    /// Pre-lowered ARC functions: parent → (`ArcFunction`, lambdas).
    pub(super) arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
    /// Monomorphized generic functions (reused by codegen to avoid recomputation).
    pub(super) mono_functions: Vec<ori_llvm::monomorphize::MonoFunction>,
}

/// Run ARC borrow inference on all non-generic module functions.
///
/// Lowers each function (local, mono, imported mono) to ARC IR and runs
/// per-SCC Salsa-tracked borrow inference queries. Returns annotated
/// signatures plus a cache of pre-lowered ARC functions for zero-copy
/// consumption by codegen; Salsa memoizes per-SCC results, so only SCCs with
/// changed bodies re-analyze on recompilation.
/// Returns `None` when ARC lowering reported errors (diagnostics already
/// emitted); the caller aborts codegen instead of continuing with an empty cache.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline helper — distinct data flow inputs per compilation stage"
)]
#[expect(
    clippy::too_many_lines,
    reason = "pipeline driver composing SCC analysis + ARC lowering loops"
)]
pub(super) fn run_borrow_inference(
    db: &CompilerDb,
    parse_result: &ParseOutput,
    function_sigs: &[FunctionSig],
    impl_sigs: &[ori_types::ImplSig],
    import_sigs: &[ori_llvm::monomorphize::ImportSig],
    imported: ImportedSurfaces<'_>,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    source_path: &str,
    mono_instances: &[ori_types::MonoInstance],
) -> Option<BorrowInferenceResult> {
    use crate::query::arc_queries::{arc_scc_decomposition, infer_borrow_scc, ArcModuleInput};

    let ImportedSurfaces {
        imported_mono_fns,
        re_interned_canons,
    } = imported;

    // Grouped cache (parent → lambdas) feeds codegen; a flat clone feeds Salsa.
    let mut arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> =
        FxHashMap::default();
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
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    // Lower monomorphized generic functions.
    // Why: without mono entries in annotated_sigs, borrow lookup falls back to all-Owned.
    let mono_functions = ori_llvm::monomorphize::collect_mono_functions(
        mono_instances,
        function_sigs,
        impl_sigs,
        import_sigs,
        interner,
        pool,
    );
    for mono_fn in &mono_functions {
        // Method specializations lower against the impl-method body namespace
        // (`method_root_for`); free functions against `root_for`.
        let (arc_fn, lambdas) = match mono_fn.receiver_type_name {
            Some(type_name) => crate::arc_lowering::lower_impl_method_to_arc(
                mono_fn.mangled_name,
                &mono_fn.sig,
                mono_fn.original_name,
                type_name,
                canon,
                interner,
                pool,
                &mut arc_problems,
                Some(&mono_fn.body_type_map),
            ),
            None => crate::arc_lowering::lower_to_arc(
                mono_fn.mangled_name,
                &mono_fn.sig,
                mono_fn.original_name,
                canon,
                interner,
                pool,
                &mut arc_problems,
                Some(&mono_fn.body_type_map),
            ),
        };
        super::pc2_hooks::run_pc2_hook_aot(
            pool,
            &arc_fn,
            &lambdas,
            interner,
            &exempt,
            "aot_mono",
            "aot_mono_lambda",
        );
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
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
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return None;
        }
    }

    // Why: cloning the grouped cache into a flat Salsa map is negligible vs
    // re-lowering every function.
    let mut arc_functions_map: FxHashMap<Name, ori_arc::ArcFunction> = FxHashMap::default();
    for (parent, lambdas) in arc_cache.values() {
        arc_functions_map.insert(parent.name, parent.clone());
        for lambda in lambdas {
            arc_functions_map.insert(lambda.name, lambda.clone());
        }
    }

    debug_assert!(
        db.pool_cache()
            .get(&std::path::PathBuf::from(source_path))
            .is_some(),
        "Pool not cached for source_path '{source_path}' — path may diverge from file.path(db)"
    );

    let sorted_functions = ArcModuleInput::sorted_functions(arc_functions_map);
    let module = ArcModuleInput::new(db, std::path::PathBuf::from(source_path), sorted_functions);

    tracing::debug!(
        function_count = module.functions(db).len(),
        "created ArcModuleInput for Salsa borrow inference"
    );

    let decomp = arc_scc_decomposition(db, module);

    let mut annotated_sigs = FxHashMap::default();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "SCC count bounded by function count, fits in u32"
    )]
    for i in 0..decomp.len() {
        let result = infer_borrow_scc(db, module, i as u32);
        for (name, sig) in result.iter() {
            annotated_sigs.insert(*name, sig.clone());
        }
    }

    tracing::debug!(
        sig_count = annotated_sigs.len(),
        scc_count = decomp.len(),
        "Salsa borrow inference complete"
    );

    // Why: imported monos merge into mono_functions so declare/prepare sees
    // them through the same emission path as local monos.
    let mut all_mono_functions = mono_functions;
    all_mono_functions.extend(imported_mono_fns.iter().map(|(mf, _, _)| mf.clone()));

    Some(BorrowInferenceResult {
        sigs: annotated_sigs,
        arc_cache,
        mono_functions: all_mono_functions,
    })
}
