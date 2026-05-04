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
//! Called from `compile_common::compile_to_llvm` and
//! `compile_common::compile_to_llvm_with_imports`.

#[cfg(feature = "llvm")]
mod finalize;
#[cfg(feature = "llvm")]
mod pc2_hooks;

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
/// Lowers each function to ARC IR and runs per-SCC Salsa-tracked borrow
/// inference queries. Returns both the annotated signatures and a cache of
/// pre-lowered ARC functions for zero-copy consumption by codegen.
///
/// # Flow
///
/// 1. Lower each function to ARC IR
/// 2. Clone into flat map for Salsa, keep grouped cache for codegen
/// 3. Create [`ArcModuleInput`] Salsa input from lowered functions
/// 4. Query [`arc_scc_decomposition`] for SCC structure
/// 5. Query [`infer_borrow_scc`] per SCC (creates Salsa dependency edges)
/// 6. Return `(annotated_sigs, arc_cache)`
///
/// Salsa memoizes per-SCC results. On recompilation, only SCCs with changed
/// function bodies are re-analyzed. Early cutoff skips dependent SCCs when
/// borrow signatures are unchanged.
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
    impl_sigs: &[(Name, FunctionSig)],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    source_path: &str,
    mono_instances: &[ori_types::MonoInstance],
) -> BorrowInferenceResult {
    use crate::query::arc_queries::{arc_scc_decomposition, infer_borrow_scc, ArcModuleInput};

    // 1. Lower functions to ARC IR.
    // We build both a grouped cache (parent → lambdas) for codegen and a flat
    // map for Salsa. The flat map is cloned from the grouped data before being
    // consumed by ArcModuleInput::sorted_functions.
    let mut arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> =
        FxHashMap::default();
    let mut arc_problems = Vec::new();

    // PC-2 diagnostic localization (PC-2); primary seam owns
    // record_codegen_error(). Empty exempt set: pre-mono skips generics via
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
    // The JIT runner already does this (runner/mod.rs), but the AOT path was
    // missing it — mono function names would be absent from annotated_sigs,
    // causing the warn! fallback to all-Owned in define_function_body_arc_with_subst.
    let mono_functions = ori_llvm::monomorphize::collect_mono_functions(
        mono_instances,
        function_sigs,
        impl_sigs,
        interner,
        pool,
    );
    for mono_fn in &mono_functions {
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            mono_fn.mangled_name,
            &mono_fn.sig,
            mono_fn.original_name,
            canon,
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
            "aot_mono",
            "aot_mono_lambda",
        );
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return BorrowInferenceResult {
                sigs: FxHashMap::default(),
                arc_cache: FxHashMap::default(),
                mono_functions: Vec::new(),
            };
        }
    }

    // 2. Build flat map for Salsa (clone from grouped cache).
    // The clone cost is negligible vs borrow inference + LLVM codegen,
    // and it replaces a full second lowering pass.
    let mut arc_functions_map: FxHashMap<Name, ori_arc::ArcFunction> = FxHashMap::default();
    for (parent, lambdas) in arc_cache.values() {
        arc_functions_map.insert(parent.name, parent.clone());
        for lambda in lambdas {
            arc_functions_map.insert(lambda.name, lambda.clone());
        }
    }

    // 3. Create ArcModuleInput Salsa input.
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

    // 4. Query SCC decomposition (Salsa-tracked).
    let decomp = arc_scc_decomposition(db, module);

    // 5. Query per-SCC borrow inference and collect results.
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

    BorrowInferenceResult {
        sigs: annotated_sigs,
        arc_cache,
        mono_functions,
    }
}

/// Run the codegen pipeline on a pre-checked module.
///
/// Shared implementation for [`compile_to_llvm`] and [`compile_to_llvm_with_imports`].
/// The pipeline:
/// 1. Declares runtime functions
/// 2. Registers user-defined types
/// 3. Runs ARC borrow inference (per-SCC Salsa queries)
/// 4. Two-pass function compilation (declare, then define)
/// 5. Monomorphization of generic functions
/// 6. Impl method and derived trait compilation
/// 7. Main wrapper generation
///
/// `symbol_prefix` controls symbol mangling: `""` for single-file (no module
/// prefix on symbols), or the module name for multi-file compilation.
/// `import_sigs` declares external symbols from other modules; pass `&[]` for
/// single-file compilation.
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
    import_sigs: &[(Name, FunctionSig)],
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

    // ManuallyDrop + raw-pointer reborrow to work around a borrow checker
    // limitation: FunctionCompiler's lifetime parameters tie the compilation
    // block's borrow of `scx` to the return lifetime, preventing us from
    // consuming `scx` afterward. The raw-pointer roundtrip creates a detached
    // reference whose borrow doesn't leak out of the block. Sound because `scx`
    // lives for the entire function and compilation borrows end at the block
    // boundary.
    let scx = ManuallyDrop::new(SimpleCx::new(context, module_name));

    let (codegen_errors, codegen_descriptions) = {
        // SAFETY: Detached reference to scx — see comment above.
        let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

        let eh_model = target_triple.map_or(EhModel::Itanium, EhModel::from_triple);
        let mut builder = IrBuilder::new_aot(scx_ref, eh_model);

        // Runtime functions are declared lazily via `builder.runtime_fn(name)`.
        // No eager `declare_runtime()` call needed — each function is declared
        // on first use during codegen and cached thereafter.

        // 3. Run ARC borrow inference pipeline (per-SCC Salsa queries)
        // Returns both annotated sigs and pre-lowered ARC functions to
        // eliminate the redundant second lowering pass during codegen.
        let function_sigs = oric::typeck::build_function_sigs(parse_result, type_result);
        let classifier = ori_arc::ArcClassifier::new(pool);
        let BorrowInferenceResult {
            sigs: annotated_sigs,
            mut arc_cache,
            mono_functions,
        } = run_borrow_inference(
            db,
            parse_result,
            &function_sigs,
            &type_result.typed.impl_sigs,
            canon,
            interner,
            pool,
            source_path,
            &type_result.typed.mono_instances,
        );

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
            );
        }

        // ARC-IR phase dumps (ORI_DUMP_AFTER_ARC + ORI_EMIT_ARC_DOT gates)
        finalize::dump_arc_phases(
            &arc_cache,
            &annotated_sigs,
            &classifier,
            pool,
            interner,
            source_path,
        );

        // 3a. Compute representation plan.
        // Include impl methods in the analysis set so interprocedural range
        // analysis sees their call sites. They are ARC-lowered into a separate
        // vec (not the codegen arc_cache) to avoid interfering with
        // compile_impls() which does its own ARC lowering for LLVM emission.
        let all_arc_funcs = {
            let mut funcs = super::repr_setup::collect_all_arc_functions(&arc_cache);
            funcs.extend(super::repr_setup::lower_impl_methods_for_analysis(
                parse_result,
                type_result,
                interner,
                canon,
                pool,
            ));
            funcs
        };
        // Only count non-generic impl methods — generic ones are skipped by
        // the ARC lowering loop, so they don't enter the analysis set.
        let has_impl_methods = type_result
            .typed
            .impl_sigs
            .iter()
            .any(|(_, sig)| !sig.is_generic());
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

        // Create type store with repr plan for triviality delegation.
        let store = TypeInfoStore::new_with_plan(pool, &repr_plan);
        // Create type resolver with the repr plan.
        let resolver = TypeLayoutResolver::new(&store, scx_ref, Some(interner), Some(&repr_plan));

        // Register user-defined types (creates named LLVM struct types).
        type_registration::register_user_types(&resolver, &type_result.typed.types);

        // 3b. Interprocedural uniqueness analysis (COW check elimination).
        // Runs AFTER borrow inference (needs ownership annotations) and BEFORE
        // per-function ARC pipeline (which uses summaries for CowMode annotation).
        let uniqueness_summaries = {
            let all_funcs = super::repr_setup::collect_all_arc_functions(&arc_cache);
            ori_arc::run_uniqueness_analysis(&all_funcs, &classifier, interner)
        };

        // 3c. AIMS interprocedural contracts (param/arg ownership).
        let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
        let aims_contracts = {
            let mut all_funcs = super::repr_setup::collect_all_arc_functions(&arc_cache);
            ori_arc::compute_aims_contracts(&mut all_funcs, &classifier, interner, &builtins)
        };

        // 4. Two-pass function compilation with borrow annotations
        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            pool,
            symbol_prefix,
            &annotated_sigs,
            &classifier,
            None, // Debug info wiring deferred to AOT pipeline integration
            uniqueness_summaries,
            aims_contracts,
            std::env::var(crate::debug_flags::ORI_VERIFY_ARC).is_ok_and(|v| v != "0"),
        );

        // Declare imports (no-op when import_sigs is empty for single-file compilation)
        if !import_sigs.is_empty() {
            fc.declare_imports(import_sigs);
        }
        fc.declare_all(&parse_result.module.functions, &function_sigs);

        // 4b. Declare monomorphized generic functions (reused from borrow inference)
        fc.declare_mono_functions(&mono_functions);

        // 5. Compile impl methods (still inline — they use type-qualified
        // canon lookup paths and are not pre-lowered for borrow inference)
        if !parse_result.module.impls.is_empty() {
            fc.compile_impls(
                &parse_result.module.impls,
                &type_result.typed.impl_sigs,
                canon,
                &parse_result.module.traits,
            );
        }

        // 5b. Compile derived trait methods
        if parse_result
            .module
            .types
            .iter()
            .any(|t| !t.derives.is_empty())
        {
            fc.compile_derives(&parse_result.module, &type_result.typed.types);
        }

        // 6. Two-pass function compilation for sound nounwind analysis:
        //    a) Lower all functions to ARC IR (no LLVM emission)
        //    b) Build complete nounwind set via fixed-point analysis
        //    c) Emit LLVM IR using the complete nounwind set
        let mut prepared = fc.prepare_all_cached(
            &parse_result.module.functions,
            &function_sigs,
            canon,
            &mut arc_cache,
        );

        // 6b. Prepare monomorphized function bodies from pre-lowered cache.
        prepared.extend(fc.prepare_mono_cached(&mono_functions, canon, &mut arc_cache));

        // 6c. Build complete nounwind set and emit LLVM IR
        fc.compute_nounwind_set(&prepared);
        fc.emit_prepared_functions(prepared);

        // 6d. Post-hoc nounwind: catch impl methods and functions whose
        // ARC IR Apply callees were inlined (e.g., built-in @length) leaving
        // no invoke instructions in the final LLVM IR.
        fc.apply_posthoc_nounwind();

        // 7. Generate C main() entry-point wrapper for @main (AOT only),
        // registering the @panic handler with it when present.
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

        // Extract soft codegen error info before builder goes out of scope.
        // The JIT path checks this in evaluator/compile.rs; the AOT path
        // must also check to avoid producing crashing binaries.
        (
            builder.codegen_error_count() + store.type_error_count(),
            builder.codegen_error_descriptions(),
        )
    };

    // SAFETY: ManuallyDrop is used only to suppress the borrow checker — the
    // compilation block's borrows have ended. `finalize_module` runs post-codegen
    // dump + audit + verify and returns the cloned module. We can't call
    // into_inner() because SimpleCx has other fields that would be moved while
    // the ManuallyDrop still exists, so the helper clones the module internally.
    finalize::finalize_module(
        &scx,
        codegen_errors,
        &codegen_descriptions,
        source_path,
        pool,
        interner,
    )
}
