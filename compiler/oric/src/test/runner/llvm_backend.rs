//! LLVM JIT backend for the test runner.
//!
//! Compiles test functions via the LLVM pipeline for JIT execution,
//! including cross-module type re-interning and ARC lowering.

use std::time::{Duration, Instant};

use std::path::Path;

use ori_llvm::evaluator::{ImportedFunctionForCodegen, OwnedLLVMEvaluator};
use ori_types::TypeCheckResult;
use rustc_hash::FxHashMap;

use super::super::result::{FileSummary, TestOutcome, TestResult};
use super::arc_lowering::lower_and_infer_borrows;
use super::TestRunner;
use super::TestRunnerConfig;

/// Index into `imported_sigs_storage` and `resolved.modules` for linking
/// imported function codegen structs back to their source data.
struct FnRef {
    func_index: usize,
    module_index: usize,
    local_name: crate::ir::Name,
    original_name: crate::ir::Name,
}

/// Declare a module-aliased module's functions under their ORIGINAL names into
/// the codegen sig/ref sets.
///
/// An aliased module's bodies call their same-module callees (public OR private)
/// by original name — the module never sees the importer's alias — so the
/// importer-facing qualified `alias.func` entries do not cover internal calls.
/// These entries are NOT in `resolved.imported_functions` (that list is shared
/// with typeck, where an original-name entry would wrongly bring a bare `func`
/// into the importer's scope and break alias scoping). Keyed by SOURCE-FILE
/// identity (not the per-`use` `module_index`, which differs for two aliases of
/// the same module) so two aliases of one source module re-intern + declare its
/// internals once; `declare_all` separately dedups by `func.name`.
#[expect(
    clippy::too_many_arguments,
    reason = "threads the test runner's re-intern context (pools, caches, var-remaps, output vectors) for one codegen step"
)]
fn declare_module_alias_internals(
    resolved: &crate::imports::ResolvedImports,
    imported_type_results: &[TypeCheckResult],
    imported_pools: &[std::sync::Arc<ori_types::Pool>],
    per_module_caches: &mut [rustc_hash::FxHashMap<ori_types::Idx, ori_types::Idx>],
    per_module_var_remaps: &mut [rustc_hash::FxHashMap<u32, u32>],
    merged_pool: &mut ori_types::Pool,
    re_interned_sigs: &mut Vec<ori_types::FunctionSig>,
    fn_refs: &mut Vec<FnRef>,
) {
    let mut declared_internal: rustc_hash::FxHashSet<(
        Option<crate::input::SourceFile>,
        crate::ir::Name,
    )> = rustc_hash::FxHashSet::default();
    for func_ref in &resolved.imported_functions {
        if !func_ref.is_module_alias {
            continue;
        }
        let module_index = func_ref.module_index;
        let imp_module = &resolved.modules[module_index];
        let tc = &imported_type_results[module_index];
        for (idx, func) in imp_module.parse_output.module.functions.iter().enumerate() {
            if !declared_internal.insert((imp_module.source_file, func.name)) {
                continue;
            }
            let Some(sig) = tc.typed.functions.iter().find(|s| s.name == func.name) else {
                continue;
            };
            if sig.is_generic() {
                continue;
            }
            let source_pool = &imported_pools[module_index];
            let cache = &mut per_module_caches[module_index];
            let var_remap = &mut per_module_var_remaps[module_index];
            let re_interned = ori_types::re_intern_sig_with_var_remap(
                sig,
                source_pool,
                merged_pool,
                cache,
                var_remap,
            );
            re_interned_sigs.push(re_interned);
            fn_refs.push(FnRef {
                func_index: idx,
                module_index,
                local_name: func.name,
                original_name: func.name,
            });
        }
    }
}

impl TestRunner {
    /// Run regular (non-`compile_fail`) tests using the LLVM JIT backend.
    ///
    /// Uses the "compile once, run many" pattern: compiles all functions and test
    /// wrappers into a single JIT engine, then runs each test from that engine.
    /// This avoids O(n²) recompilation that caused LLVM resource exhaustion.
    ///
    /// Note: `compile_fail` tests are handled in the common path of
    /// `run_file_with_interner()` before backend dispatch — they are NOT
    /// passed here. This avoids double-counting.
    #[expect(
        clippy::too_many_arguments,
        reason = "test runner mirrors the full compilation pipeline — all inputs are required"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "JIT test pipeline — splitting would fragment the compile→run flow"
    )]
    pub(super) fn run_file_llvm(
        summary: &mut FileSummary,
        db: &crate::db::CompilerDb,
        file_path: &Path,
        parse_result: &crate::parser::ParseOutput,
        regular_tests: &[&crate::ir::TestDef],
        type_result: &TypeCheckResult,
        pool: &ori_types::Pool,
        shared_canon: &ori_ir::canon::SharedCanonResult,
        skippable: &rustc_hash::FxHashSet<crate::ir::Name>,
        interner: &crate::ir::StringInterner,
        config: &TestRunnerConfig,
    ) {
        // Skip LLVM compilation if no regular tests to run
        if regular_tests.is_empty() {
            return;
        }

        // Filter regular tests before compilation
        let (skipped_unchanged, filtered_tests): (Vec<_>, Vec<_>) = regular_tests
            .iter()
            .filter(|test| Self::test_passes_filter(test, config, interner))
            .copied()
            .partition(|test| skippable.contains(&test.name));

        // Incremental: report unchanged tests as skipped (mirrors the
        // interpreter path) and keep them out of JIT compilation entirely.
        for test in &skipped_unchanged {
            let result = TestResult {
                name: test.name,
                targets: test.targets.clone(),
                outcome: TestOutcome::SkippedUnchanged,
                duration: Duration::ZERO,
            };
            Self::protocol_result(&result, config, interner);
            summary.add_result(result);
        }

        if filtered_tests.is_empty() {
            return;
        }

        // Install custom LLVM fatal error handler so LLVM errors panic
        // instead of aborting the process (allows catch_unwind recovery).
        ori_llvm::install_fatal_error_handler();

        // Resolve imports so imported functions can be compiled into the JIT module.
        // Uses the unified import pipeline — same resolution path as the type checker
        // and interpreter.
        let resolved = crate::imports::resolve_imports(db, parse_result, file_path);

        // Type-check each explicitly imported module to get expr_types + function_sigs.
        // Note: prelude functions are NOT compiled into the JIT module because:
        // 1. Most prelude content is traits (no code to compile)
        // 2. Generic utility functions are skipped by codegen
        // 3. Some non-generic prelude functions (e.g., `compare`) use types the
        //    V2 codegen doesn't support yet (sum types), causing IR verification failures
        // Prelude functions that are needed for testing (assert, assert_eq) come from
        // std.testing via explicit import, not from the prelude.
        // Type-check each imported module via Salsa queries (when SourceFile is available).
        // This ensures results are cached in Salsa's dependency graph and the Pool
        // is stored in PoolCache, avoiding redundant work when the same module is
        // imported by multiple test files.
        // The prelude is processed as an additional imported-module slot so its
        // `pub` generic free functions (e.g. `min`/`max`) participate in the
        // imported-generic monomorphization pipeline exactly like an explicitly
        // imported generic. Its slot index is `resolved.modules.len()` (appended
        // last); `register_prelude_generic_sigs` keys `imported_generic_sigs`
        // entries to that index below.
        let all_import_modules: Vec<&crate::imports::ResolvedImportedModule> = resolved
            .modules
            .iter()
            .chain(resolved.prelude.as_ref())
            .collect();
        let prelude_module_index = resolved.prelude.as_ref().map(|_| resolved.modules.len());

        let mut imported_type_results: Vec<TypeCheckResult> = Vec::new();
        let mut imported_canon_results: Vec<ori_ir::canon::SharedCanonResult> = Vec::new();
        let mut imported_pools: Vec<std::sync::Arc<ori_types::Pool>> = Vec::new();
        for &imp_module in &all_import_modules {
            // Type-check via shared helper (Salsa queries when SourceFile is
            // available, direct type checking otherwise).
            let Some((imp_tc, imp_pool)) = crate::query::type_check_module(
                db,
                &imp_module.parse_output,
                &imp_module.module_path,
                imp_module.source_file,
            ) else {
                // Pool not cached — internal error. Push empty results to
                // maintain index alignment with resolved.modules.
                imported_type_results.push(TypeCheckResult::ok(ori_types::TypedModule::default()));
                imported_canon_results.push(ori_ir::canon::SharedCanonResult::new(
                    ori_ir::canon::CanonResult::empty(),
                ));
                imported_pools.push(std::sync::Arc::new(ori_types::Pool::new()));
                continue;
            };
            // Use cached canonicalization — avoids re-canonicalizing the same
            // module (e.g., std.testing) when imported by multiple test files.
            let imp_canon = crate::query::canonicalize_cached_by_path(
                db,
                &imp_module.module_path,
                &imp_module.parse_output,
                &imp_tc,
                &imp_pool,
            );
            imported_type_results.push(imp_tc);
            imported_canon_results.push(imp_canon);
            imported_pools.push(imp_pool);
        }

        // Merkle Pool Identity: Single-Pool Re-interning
        //
        // Clone the main pool and re-intern all imported types into it. This
        // eliminates cross-pool Idx misuse: every Idx value in every sig and
        // canon is valid in the merged pool. ARC lowering and LLVM codegen
        // can then operate on a single pool without dual-pool juggling.
        //
        // Cost: O(n) in pool size — clones 10+ Vecs (items, extra, flags,
        // hashes, intern_map). Necessary for the merged-pool pattern; no
        // way to avoid without an architectural change to dual-pool codegen.
        let mut merged_pool = pool.clone();

        // Re-map canon results per module: clone each CanonResult and remap
        // its TypeId values from the source pool to the merged pool. Build
        // per-module re-interning caches AND per-module var_remap maps so sig
        // re-interning reuses them (all 3 consumer sites below — canon arena
        // re-intern, concrete sig re-intern, generic sig re-intern — for a
        // given imported module share the SAME var_remap, keeping
        // scheme_var_ids and the leaf Tag::Var ids they bind coherent).
        let mut per_module_caches: Vec<rustc_hash::FxHashMap<ori_types::Idx, ori_types::Idx>> =
            vec![rustc_hash::FxHashMap::default(); imported_pools.len()];
        let mut per_module_var_remaps: Vec<rustc_hash::FxHashMap<u32, u32>> =
            vec![rustc_hash::FxHashMap::default(); imported_pools.len()];
        let mut re_interned_canons: Vec<ori_ir::canon::CanonResult> = imported_canon_results
            .iter()
            .enumerate()
            .map(|(module_idx, shared_canon)| {
                let source_pool = &imported_pools[module_idx];
                let cache = &mut per_module_caches[module_idx];
                let var_remap = &mut per_module_var_remaps[module_idx];

                // Cost: O(n) clone of the full CanonResult (struct-of-arrays arena)
                // per imported module. Necessary because each import's TypeIds must
                // be remapped independently into the merged pool.
                let mut remapped: ori_ir::canon::CanonResult = (**shared_canon).clone();
                remapped.arena.remap_types(|type_id| {
                    let source_idx = ori_types::Idx::from_raw(type_id.raw());
                    let target_idx = ori_types::re_intern_type_with_var_remap(
                        source_pool,
                        source_idx,
                        &mut merged_pool,
                        cache,
                        var_remap,
                    );
                    ori_ir::TypeId::from_raw(target_idx.raw())
                });
                remapped
            })
            .collect();

        // Build per-function codegen structs for explicitly imported functions only.
        let mut fn_refs: Vec<FnRef> = Vec::new();
        let mut re_interned_sigs: Vec<ori_types::FunctionSig> = Vec::new();

        for func_ref in &resolved.imported_functions {
            if func_ref.is_module_alias {
                continue;
            }
            let imp_module = &resolved.modules[func_ref.module_index];
            let tc = &imported_type_results[func_ref.module_index];

            // Find the function by original_name in the imported module
            if let Some((idx, _func)) = imp_module
                .parse_output
                .module
                .functions
                .iter()
                .enumerate()
                .find(|(_, f)| f.name == func_ref.original_name)
            {
                // Find its type-checked signature
                if let Some(sig) = tc
                    .typed
                    .functions
                    .iter()
                    .find(|s| s.name == func_ref.original_name)
                {
                    if sig.is_generic() {
                        continue;
                    }
                    // Re-intern the signature from the source pool into the merged pool,
                    // reusing the per-module cache AND var_remap built during canon
                    // re-mapping — scheme_var_ids and leaf Tag::Var ids must remap
                    // coherently through the same var_remap.
                    let source_pool = &imported_pools[func_ref.module_index];
                    let cache = &mut per_module_caches[func_ref.module_index];
                    let var_remap = &mut per_module_var_remaps[func_ref.module_index];
                    let mut re_interned = ori_types::re_intern_sig_with_var_remap(
                        sig,
                        source_pool,
                        &mut merged_pool,
                        cache,
                        var_remap,
                    );
                    // Aliased imports (incl. the synthesized `"alias.func"`
                    // module-alias entries) are codegen'd + mangled under their
                    // LOCAL name so a `Call(FunctionRef(local_name))` links to the
                    // declared symbol. `sig.name` is one of three surfaces keyed
                    // under local_name; the renamed `Function` (declare/body key)
                    // and the aliased `CanonRoot` (body lookup) are set below.
                    if func_ref.local_name != func_ref.original_name {
                        re_interned.name = func_ref.local_name;
                    }
                    re_interned_sigs.push(re_interned);
                    fn_refs.push(FnRef {
                        func_index: idx,
                        module_index: func_ref.module_index,
                        local_name: func_ref.local_name,
                        original_name: func_ref.original_name,
                    });
                }
            }
        }

        // Codegen-only: declare each module-aliased module's internal callees
        // under their ORIGINAL names (importer-facing `alias.func` entries do
        // not cover same-module internal calls). See helper doc for the keying
        // + scoping contract.
        declare_module_alias_internals(
            &resolved,
            &imported_type_results,
            &imported_pools,
            &mut per_module_caches,
            &mut per_module_var_remaps,
            &mut merged_pool,
            &mut re_interned_sigs,
            &mut fn_refs,
        );

        // Collect imported generic sigs for monomorphization resolution.
        // Generic sigs are skipped for ImportedFunctionForCodegen (they aren't
        // compiled directly), but we need them to build concrete MonoFunctions
        // for their call-site instantiations.
        //
        // Key by local_name (not original_name): MonoInstance.fn_name uses the
        // call-site identifier, which is the local/aliased name from the import.
        // Value: (re_interned_sig, module_index, original_name_in_source_module)
        let mut imported_generic_sigs: FxHashMap<
            ori_ir::Name,
            (ori_types::FunctionSig, usize, ori_ir::Name),
        > = FxHashMap::default();
        for func_ref in &resolved.imported_functions {
            if func_ref.is_module_alias {
                continue;
            }
            let tc = &imported_type_results[func_ref.module_index];
            if let Some(sig) = tc
                .typed
                .functions
                .iter()
                .find(|s| s.name == func_ref.original_name)
            {
                if !sig.is_generic() {
                    continue;
                }
                let source_pool = &imported_pools[func_ref.module_index];
                let cache = &mut per_module_caches[func_ref.module_index];
                let var_remap = &mut per_module_var_remaps[func_ref.module_index];
                let re_interned = ori_types::re_intern_sig_with_var_remap(
                    sig,
                    source_pool,
                    &mut merged_pool,
                    cache,
                    var_remap,
                );
                imported_generic_sigs.insert(
                    func_ref.local_name,
                    (re_interned, func_ref.module_index, func_ref.original_name),
                );
            }
        }

        // Register the prelude's `pub` generic free functions (min/max/…) so
        // their MonoInstances resolve in build_imported_mono_functions — they
        // are implicit (not ImportedFunctionRef entries) so the loop above
        // never sees them.
        if let (Some(prelude_idx), Some(prelude_module)) =
            (prelude_module_index, resolved.prelude.as_ref())
        {
            let source_pool = &imported_pools[prelude_idx];
            // SAFETY-of-borrows: split the cache/var_remap borrow from the
            // pool borrow by cloning the Arc — the source pool is read-only.
            let source_pool = std::sync::Arc::clone(source_pool);
            let cache = &mut per_module_caches[prelude_idx];
            let var_remap = &mut per_module_var_remaps[prelude_idx];
            super::imported_mono::register_prelude_generic_sigs(
                &mut imported_generic_sigs,
                &prelude_module.parse_output,
                &imported_type_results[prelude_idx].typed,
                &source_pool,
                prelude_idx,
                &mut merged_pool,
                cache,
                var_remap,
            );
        }

        // Build imported MonoFunction structs for imported generic
        // instantiations. Delegated to `imported_mono::build_imported_mono_functions`
        // to keep `run_file_llvm` under the 100-line fn-length limit.
        let imported_mono_fns = super::imported_mono::build_imported_mono_functions(
            type_result,
            &imported_generic_sigs,
            &per_module_caches,
            &mut merged_pool,
            interner,
        );

        // Rename aliased imports onto their local_name across the two remaining
        // codegen surfaces (the sig is already renamed above): a `Function` clone
        // (declare_all + prepare_all_cached key the symbol/body on `func.name`)
        // and an aliased `CanonRoot` (so `canon.root_for(local_name)` resolves to
        // the original body). All three keys then agree, so
        // `Call(FunctionRef(local_name))` declares + links.
        let mut renamed_functions: Vec<Option<crate::ir::Function>> =
            Vec::with_capacity(fn_refs.len());
        for fref in &fn_refs {
            if fref.local_name == fref.original_name {
                renamed_functions.push(None);
                continue;
            }
            let canon = &mut re_interned_canons[fref.module_index];
            if canon.root_for(fref.local_name).is_none() {
                if let Some(mut aliased) = canon
                    .roots
                    .iter()
                    .find(|r| r.name == fref.original_name)
                    .cloned()
                {
                    aliased.name = fref.local_name;
                    canon.roots.push(aliased);
                }
            }
            let mut renamed = resolved.modules[fref.module_index]
                .parse_output
                .module
                .functions[fref.func_index]
                .clone();
            renamed.name = fref.local_name;
            renamed_functions.push(Some(renamed));
        }

        // Build ImportedFunctionForCodegen — all Idx values are valid in merged_pool
        let imported_for_codegen: Vec<ImportedFunctionForCodegen<'_>> = fn_refs
            .iter()
            .enumerate()
            .map(|(sig_idx, fref)| {
                let parse_output = &resolved.modules[fref.module_index].parse_output;
                let function = match &renamed_functions[sig_idx] {
                    Some(renamed) => renamed,
                    None => &parse_output.module.functions[fref.func_index],
                };
                ImportedFunctionForCodegen {
                    function,
                    sig: re_interned_sigs[sig_idx].clone(),
                    canon: &re_interned_canons[fref.module_index],
                }
            })
            .collect();

        // Create LLVM evaluator with merged pool for proper compound type resolution
        // (needed for sret convention on large struct returns like List, Map, etc.)
        // Must be created AFTER re-interning so all Idx values in the merged pool
        // are valid when the evaluator resolves types during codegen.
        let llvm_eval = OwnedLLVMEvaluator::with_pool(&merged_pool);

        // Build function signatures aligned with module.functions source order.
        // Delegates to shared implementation in typeck.
        let function_sigs = crate::typeck::build_function_sigs(parse_result, type_result);

        // Collect exported type metadata from imported modules for repr plan
        // construction. This ensures imported `pub` and `#repr(...)` types are
        // correctly exempted from integer narrowing.
        //
        // Each module's `exported_type_metadata` now includes transitive metadata
        // (forwarded from dependencies via generate_exported_type_metadata merge
        // in the type checker's finish_with_pool).
        let imported_type_metadata: Vec<ori_types::ExportedTypeMetadata> = imported_type_results
            .iter()
            .flat_map(|tc| tc.typed.exported_type_metadata.iter().cloned())
            .collect();

        // Collect imported collection surface hashes for cross-module ABI
        // protection.
        let imported_collection_surfaces: Vec<u64> = imported_type_results
            .iter()
            .flat_map(|tc| tc.typed.exported_collection_surfaces.iter().copied())
            .collect();

        // ARC lowering + borrow inference + compilation, wrapped in catch_unwind
        // to gracefully handle panics in any phase (ARC classification, LLVM codegen,
        // etc.) without aborting the entire test runner.
        let compile_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // `lower_and_infer_borrows` returns `None` when ARC lowering emits
            // errors. Proceeding to `compile_module_with_tests`
            // with empty arc_cache recurses in codegen when tests reference
            // missing callees, leading to stack overflow. Treat `None` as a
            // compile failure — the `Err(_)` arm of `compile_result` below
            // emits `LlvmCompileFail` outcomes for each filtered test.
            let Some((annotated_sigs, arc_cache)) = lower_and_infer_borrows(
                &parse_result.module,
                &function_sigs,
                shared_canon,
                interner,
                &merged_pool,
                &type_result.typed.impl_sigs,
                // Test runner JIT: imported mono bodies are handled via
                // `imported_mono_fns` below, not via the import_sigs lookup chain.
                &[],
                &imported_for_codegen,
                &type_result.typed.mono_instances,
                &type_result.typed.types,
                &imported_mono_fns,
                &re_interned_canons,
            ) else {
                return Err(ori_llvm::evaluator::LLVMEvalError::new(
                    "ARC lowering failed — see diagnostics above".to_string(),
                ));
            };

            // Strip module indices — codegen only needs the MonoFunctions
            let imported_mono_for_codegen: Vec<ori_repr::monomorphize::MonoFunction> =
                imported_mono_fns.into_iter().map(|(mf, _, _)| mf).collect();

            llvm_eval.compile_module_with_tests(
                &parse_result.module,
                &filtered_tests,
                shared_canon,
                interner,
                &function_sigs,
                &type_result.typed.types,
                &type_result.typed.collection_burdens,
                &type_result.typed.impl_sigs,
                &imported_for_codegen,
                &type_result.typed.mono_instances,
                &annotated_sigs,
                arc_cache,
                None, // JIT: use env var fallback for narrowing policy
                &imported_type_metadata,
                &imported_collection_surfaces,
                &type_result.typed.trait_impl_fn_names,
                imported_mono_for_codegen,
            )
        }));

        let compiled = match compile_result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                // Record the compilation error for display
                summary.add_error(e.message.clone());
                summary.llvm_compile_error = true;
                // Each blocked test counts FAILED, with the LlvmCompileFail
                // outcome carrying the reason.
                Self::add_compile_fail_results(
                    summary,
                    &filtered_tests,
                    &format!("LLVM compilation failed: {}", e.message),
                    interner,
                    config,
                );
                return;
            }
            Err(panic_info) => {
                let msg = super::panic_message(panic_info.as_ref());
                summary.add_error(format!("LLVM backend error: {msg}"));
                summary.llvm_compile_error = true;
                Self::add_compile_fail_results(
                    summary,
                    &filtered_tests,
                    &format!("LLVM backend error: {msg}"),
                    interner,
                    config,
                );
                return;
            }
        };

        // Run each test from the compiled module (no recompilation!)
        for test in &filtered_tests {
            Self::protocol_start(test.name, config, interner);
            let inner_result = Self::run_single_test_from_compiled(&compiled, test, interner);

            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

            Self::protocol_result(&result, config, interner);
            summary.add_result(result);
        }
    }

    /// Record an `LlvmCompileFail` result (counted as failed) for every test
    /// blocked by a per-file LLVM compilation error.
    fn add_compile_fail_results(
        summary: &mut FileSummary,
        tests: &[&crate::ir::TestDef],
        reason: &str,
        interner: &crate::ir::StringInterner,
        config: &TestRunnerConfig,
    ) {
        for test in tests {
            let result = TestResult {
                name: test.name,
                targets: test.targets.clone(),
                outcome: TestOutcome::LlvmCompileFail(reason.to_string()),
                duration: Duration::ZERO,
            };
            Self::protocol_result(&result, config, interner);
            summary.add_result(result);
        }
    }

    /// Run a single test from an already-compiled module.
    ///
    /// This is the efficient path: the module was compiled once and we just
    /// call into the JIT engine to run each test.
    pub(super) fn run_single_test_from_compiled(
        compiled: &ori_llvm::evaluator::CompiledTestModule,
        test: &crate::ir::TestDef,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            return TestResult::skipped_for(test, reason, interner);
        }

        // Time the test execution
        let start = Instant::now();

        // Run the test from the compiled module (no recompilation!)
        match compiled.run_test(test.name) {
            Ok(_) => TestResult::passed(test.name, test.targets.clone(), start.elapsed()),
            Err(e) => {
                TestResult::failed(test.name, test.targets.clone(), e.message, start.elapsed())
            }
        }
    }
}
