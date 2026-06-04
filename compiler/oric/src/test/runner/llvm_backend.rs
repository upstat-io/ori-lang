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
        interner: &crate::ir::StringInterner,
        config: &TestRunnerConfig,
    ) {
        /// Index into `imported_sigs_storage` and `resolved.modules` for linking
        /// imported function codegen structs back to their source data.
        struct FnRef {
            func_index: usize,
            module_index: usize,
        }

        // Skip LLVM compilation if no regular tests to run
        if regular_tests.is_empty() {
            return;
        }

        // Filter regular tests before compilation
        let filtered_tests: Vec<_> = regular_tests
            .iter()
            .filter(|test| {
                if let Some(ref filter_str) = config.filter {
                    let test_name = interner.lookup(test.name);
                    test_name.contains(filter_str.as_str())
                } else {
                    true
                }
            })
            .copied()
            .collect();

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
        let mut imported_type_results: Vec<TypeCheckResult> = Vec::new();
        let mut imported_canon_results: Vec<ori_ir::canon::SharedCanonResult> = Vec::new();
        let mut imported_pools: Vec<std::sync::Arc<ori_types::Pool>> = Vec::new();
        for imp_module in &resolved.modules {
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

        // === Merkle Pool Identity: Single-Pool Re-interning ===
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
        // scheme_var_ids and the leaf Tag::Var ids they bind coherent per
        // §08.3).
        let mut per_module_caches: Vec<rustc_hash::FxHashMap<ori_types::Idx, ori_types::Idx>> =
            vec![rustc_hash::FxHashMap::default(); imported_pools.len()];
        let mut per_module_var_remaps: Vec<rustc_hash::FxHashMap<u32, u32>> =
            vec![rustc_hash::FxHashMap::default(); imported_pools.len()];
        let re_interned_canons: Vec<ori_ir::canon::CanonResult> = imported_canon_results
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
                    // coherently through the same var_remap (§08.3).
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
                    re_interned_sigs.push(re_interned);
                    fn_refs.push(FnRef {
                        func_index: idx,
                        module_index: func_ref.module_index,
                    });
                }
            }
        }

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

        // Build imported MonoFunction structs for imported generic
        // instantiations. Delegated to `imported_mono::build_imported_mono_functions`
        // to keep `run_file_llvm` under the 100-line fn-length limit
        // (§08.H F13).
        let imported_mono_fns = super::imported_mono::build_imported_mono_functions(
            type_result,
            &imported_generic_sigs,
            &per_module_caches,
            &mut merged_pool,
            interner,
        );

        // Build ImportedFunctionForCodegen — all Idx values are valid in merged_pool
        let imported_for_codegen: Vec<ImportedFunctionForCodegen<'_>> = fn_refs
            .iter()
            .enumerate()
            .map(|(sig_idx, fref)| {
                let parse_output = &resolved.modules[fref.module_index].parse_output;
                ImportedFunctionForCodegen {
                    function: &parse_output.module.functions[fref.func_index],
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
            let imported_mono_for_codegen: Vec<ori_llvm::monomorphize::MonoFunction> =
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
                // Create LlvmCompileFail results for each test — these are
                // tracked separately and don't count as real failures.
                for test in &filtered_tests {
                    summary.add_result(TestResult {
                        name: test.name,
                        targets: test.targets.clone(),
                        outcome: TestOutcome::LlvmCompileFail(format!(
                            "LLVM compilation failed: {}",
                            e.message
                        )),
                        duration: Duration::ZERO,
                    });
                }
                return;
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    (*s).to_string()
                } else {
                    "LLVM compilation panicked".to_string()
                };
                summary.add_error(format!("LLVM backend error: {msg}"));
                summary.llvm_compile_error = true;
                // Create LlvmCompileFail results for each test.
                for test in &filtered_tests {
                    summary.add_result(TestResult {
                        name: test.name,
                        targets: test.targets.clone(),
                        outcome: TestOutcome::LlvmCompileFail(format!("LLVM backend error: {msg}")),
                        duration: Duration::ZERO,
                    });
                }
                return;
            }
        };

        // Run each test from the compiled module (no recompilation!)
        for test in &filtered_tests {
            let inner_result = Self::run_single_test_from_compiled(&compiled, test, interner);

            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

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
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test.name, test.targets.clone(), reason_str);
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
