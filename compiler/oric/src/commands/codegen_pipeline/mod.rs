//! Codegen pipeline implementation for AOT compilation.
//!
//! Contains the heavy implementation details:
//! - ARC borrow inference (`run_borrow_inference`)
//! - LLVM codegen orchestration (`run_codegen_pipeline`)
//!
//! Helpers extracted into sibling modules:
//! - `repr_setup`: repr plan computation and impl-method ARC lowering for analysis
//! - `pc2_hooks`: PC-2 invariant check helper for AOT pre-mono / mono sites
//! - `finalize`: ARC phase dumps + post-codegen diagnostics-and-verify
//!
//! Called from `compile_common::compile_to_llvm_with_imported_monos` and
//! `compile_common::compile_to_llvm_with_imports`.

#[cfg(feature = "llvm")]
mod finalize;
#[cfg(feature = "llvm")]
pub(crate) mod imported_mono;
#[cfg(feature = "llvm")]
mod pc2_hooks;

#[cfg(feature = "llvm")]
use imported_mono::ImportedSurfaces;
#[cfg(feature = "llvm")]
use ori_ir::canon::CanonResult;
#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;
#[cfg(feature = "llvm")]
use ori_types::{FunctionSig, Pool, TypeCheckResult};
#[cfg(feature = "llvm")]
use oric::ir::{Name, StringInterner};
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db};
#[cfg(feature = "llvm")]
use rustc_hash::{FxHashMap, FxHashSet};

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
    impl_sigs: &[(ori_types::Idx, Name, FunctionSig)],
    import_sigs: &[(Name, String, FunctionSig)],
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
        pc2_hooks::run_pc2_hook_aot(
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
        pc2_hooks::run_pc2_hook_aot(
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
        pc2_hooks::run_pc2_hook_aot(
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

/// Run the codegen pipeline on a pre-checked module.
///
/// Shared by `compile_to_llvm_with_imported_monos` and
/// `compile_to_llvm_with_imports`: borrow inference, repr planning, two-pass
/// function compilation, monomorphization, impl + derive emission, and
/// main-wrapper generation.
/// `symbol_prefix`: `""` (single-file) or the module name (multi-file).
/// `import_sigs`: external symbols from other modules; `&[]` for single-file.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "private pipeline helper — params match the data flow from both public compile functions"
)]
#[allow(unsafe_code, reason = "LLVM C API requires unsafe FFI calls")]
#[expect(
    clippy::too_many_lines,
    reason = "sequential pipeline — further splitting would fragment the compilation flow"
)]
pub(super) fn run_codegen_pipeline<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &'ctx Pool,
    canon: &CanonResult,
    source_path: &str,
    module_name: &str,
    symbol_prefix: &str,
    import_sigs: &[(Name, String, FunctionSig)],
    imported: ImportedSurfaces<'_>,
    target_triple: Option<&str>,
    narrowing_policy: ori_repr::NarrowingPolicy,
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
) -> Result<ori_llvm::inkwell::module::Module<'ctx>, String> {
    use ori_llvm::codegen::eh_model::EhModel;
    use ori_llvm::codegen::function_compiler::FunctionCompiler;
    use ori_llvm::codegen::ir_builder::IrBuilder;
    use ori_llvm::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use ori_llvm::codegen::type_registration;
    use ori_llvm::SimpleCx;

    use std::mem::ManuallyDrop;

    let interner = db.interner();

    // SAFETY: FunctionCompiler's lifetimes tie the block's borrow of `scx` to
    // the return lifetime; ManuallyDrop + raw-pointer reborrow detaches it.
    // Sound: `scx` lives for the whole function and the borrows end at the block.
    let scx = ManuallyDrop::new(SimpleCx::new(context, module_name));

    let (codegen_errors, codegen_descriptions) = {
        // SAFETY: Detached reference to scx — see comment above.
        let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

        let eh_model = target_triple.map_or(EhModel::Itanium, EhModel::from_triple);
        let mut builder = IrBuilder::new_aot(scx_ref, eh_model);

        // Runtime functions are declared lazily on first `builder.runtime_fn(name)` use.
        let function_sigs = oric::typeck::build_function_sigs(parse_result, type_result);
        let classifier = ori_arc::ArcClassifier::new(pool);
        let Some(BorrowInferenceResult {
            sigs: annotated_sigs,
            mut arc_cache,
            mono_functions,
        }) = run_borrow_inference(
            db,
            parse_result,
            &function_sigs,
            &type_result.typed.impl_sigs,
            import_sigs,
            imported,
            canon,
            interner,
            pool,
            source_path,
            &type_result.typed.mono_instances,
        )
        else {
            // Why: diagnostics are already emitted; continuing with an empty
            // arc cache would recurse in codegen resolving missing callees.
            return Err("ARC lowering reported errors; codegen aborted".to_string());
        };

        // INVARIANT: Apply / Invoke targets in cached ArcFunctions resolve
        // to mangled mono names before AIMS contract lookup (PL-5: no-stale-
        // summary). run_borrow_inference pre-lowers monos with generic call-
        // site names — without this rewrite, forwarder bodies miss
        // analyze_program's mono-keyed contract map and transitive
        // transfers_through_return propagation silently fails.
        if !mono_functions.is_empty() {
            ori_llvm::codegen::function_compiler::rewrite_apply_targets_for_monos(
                &mut arc_cache,
                &mono_functions,
                pool,
                interner,
            );
        }

        // Why: finish_with_pool consumes the live registry, so codegen rebuilds
        // it from TypedModule exports to surface UserBurdenSpec to the burden walker.
        // Spec: Annex E §AIMS.
        let mut type_registry = ori_types::TypeRegistry::from_typed_exports(
            type_result.typed.types.clone(),
            type_result.typed.collection_burdens.clone(),
        );
        // An imported function's body resolves its internal collection types into
        // the importer's pool, but register_imported_function composes burdens for
        // signature types only — so emit_burden_ops finds no burden for an
        // imported body's internal collection (e.g. a dead for-yield local) and
        // emits no RC. Fill the gaps from the merged pool. Spec: Annex E §AIMS.
        ori_types::register_resolved_collection_burdens(pool, &mut type_registry);

        // ARC-IR phase dumps (ORI_DUMP_AFTER_ARC + ORI_EMIT_ARC_DOT gates)
        finalize::dump_arc_phases(
            &arc_cache,
            &annotated_sigs,
            &classifier,
            pool,
            interner,
            &type_registry,
            source_path,
        );

        // Why: impl methods join the analysis set so interprocedural range
        // analysis sees their call sites; they are lowered once (with
        // type-qualified analysis names) and reused for both the repr plan
        // and the as-compiled impl-method contract pre-pass below.
        let (impl_analysis_funcs, impl_qualified_by_recv) =
            super::repr_setup::lower_impl_methods_for_analysis(
                parse_result,
                type_result,
                interner,
                canon,
                pool,
            );
        let all_arc_funcs = {
            let mut funcs = super::repr_setup::collect_all_arc_functions(&arc_cache);
            funcs.extend(impl_analysis_funcs.iter().cloned());
            funcs
        };
        // Why: only non-generic impl methods enter the analysis set — generics
        // are skipped by the ARC lowering loop.
        let has_impl_methods = type_result
            .typed
            .impl_sigs
            .iter()
            .any(|(_, _, sig)| !sig.is_generic());
        let repr_plan = super::repr_setup::compute_module_repr_plan(
            pool,
            &all_arc_funcs,
            narrowing_policy,
            type_result,
            Some(interner),
            imported_type_metadata,
            imported_collection_surfaces,
            has_impl_methods,
        );

        let store = TypeInfoStore::new_with_plan(pool, &repr_plan);
        let resolver = TypeLayoutResolver::new(&store, scx_ref, Some(interner), Some(&repr_plan));

        type_registration::register_user_types(&resolver, &type_result.typed.types);

        // INVARIANT: uniqueness analysis runs after borrow inference (needs
        // ownership annotations) and before the per-function ARC pipeline
        // (which consumes the summaries for CowMode annotation).
        let uniqueness_summaries = {
            let all_funcs = super::repr_setup::collect_all_arc_functions(&arc_cache);
            ori_arc::run_uniqueness_analysis(&all_funcs, &classifier, interner)
        };

        let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
        let aims_contracts = {
            let mut all_funcs = super::repr_setup::collect_all_arc_functions(&arc_cache);
            ori_arc::compute_aims_contracts(&mut all_funcs, &classifier, interner, &builtins)
        };

        // Why: impl methods compile on the `compile_impls()` immediate-emit
        // path and never enter `compute_aims_contracts` above, so callers see
        // no contract for them (IC-1 gap) and every impl-method call site gets
        // the conservative no-contract treatment. The as-compiled pre-pass
        // computes each non-generic impl method's sanitized contract
        // (conservative except the structural `transfers_through_return`
        // Direct pair); `FunctionCompiler` binds them per caller function at
        // Phase-5 via `augment_contracts_with_impl_callees`.
        // `ORI_DISABLE_IMPL_METHOD_CONTRACTS=1` bisects.
        let impl_method_contracts = if std::env::var("ORI_DISABLE_IMPL_METHOD_CONTRACTS")
            .is_ok_and(|v| v != "0")
        {
            rustc_hash::FxHashMap::default()
        } else {
            let by_name = ori_arc::compute_impl_method_contracts(
                &impl_analysis_funcs,
                &classifier,
                &builtins,
                interner,
            );
            impl_qualified_by_recv
                .iter()
                .filter_map(|(&key, qualified)| by_name.get(qualified).map(|c| (key, c.clone())))
                .collect()
        };

        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            pool,
            &type_registry,
            symbol_prefix,
            &annotated_sigs,
            &classifier,
            None, // Debug info wiring deferred to AOT pipeline integration
            uniqueness_summaries,
            aims_contracts,
            std::env::var(crate::debug_flags::ORI_VERIFY_ARC).is_ok_and(|v| v != "0"),
        );
        fc.set_impl_method_contracts(impl_method_contracts);

        if !import_sigs.is_empty() {
            fc.declare_imports(import_sigs);
        }
        fc.declare_all(&parse_result.module.functions, &function_sigs);
        fc.declare_mono_functions(&mono_functions);

        // Why: impl methods use type-qualified canon lookup paths and are not
        // pre-lowered for borrow inference, so they compile inline here.
        if !parse_result.module.impls.is_empty() {
            fc.compile_impls(
                &parse_result.module.impls,
                &type_result.typed.impl_sigs,
                canon,
                &parse_result.module.traits,
            );
        }

        if parse_result
            .module
            .types
            .iter()
            .any(|t| !t.derives.is_empty())
        {
            fc.compile_derives(&parse_result.module, &type_result.typed.types);
        }

        // Why: two-pass (prepare everything, then emit) so the nounwind set is
        // complete before any LLVM IR is emitted.
        let mut prepared = fc.prepare_all_cached(
            &parse_result.module.functions,
            &function_sigs,
            canon,
            &mut arc_cache,
        );

        prepared.extend(fc.prepare_mono_cached(&mono_functions, canon, &mut arc_cache));

        fc.compute_nounwind_set(&prepared);
        fc.emit_prepared_functions(prepared);

        // Why: inlined Apply callees (e.g. built-in @length) can leave no invoke
        // instructions, so nounwind is re-derived post-hoc.
        fc.apply_posthoc_nounwind();

        let panic_name = parse_result
            .module
            .functions
            .iter()
            .find(|f| interner.lookup(f.name) == "panic")
            .map(|f| f.name);
        if let Some((func, sig)) = parse_result
            .module
            .functions
            .iter()
            .zip(function_sigs.iter())
            .find(|(_, sig)| sig.is_main)
        {
            fc.generate_main_wrapper(func.name, sig, panic_name);
        }

        // Why: AOT must check soft codegen errors (as the JIT path does) to
        // avoid emitting crashing binaries.
        (
            builder.codegen_error_count() + store.type_error_count(),
            builder.codegen_error_descriptions(),
        )
    };

    // SAFETY: the compilation block's borrows have ended; ManuallyDrop only
    // suppressed the borrow checker. `into_inner()` would move SimpleCx's other
    // fields while the ManuallyDrop exists, so `finalize_module` clones the module.
    finalize::finalize_module(
        &scx,
        codegen_errors,
        &codegen_descriptions,
        source_path,
        pool,
        interner,
    )
}
