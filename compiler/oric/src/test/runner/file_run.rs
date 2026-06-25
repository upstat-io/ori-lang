//! Per-file test execution: parse, type-check, canonicalize, dispatch.

use std::path::Path;
use std::time::Duration;

use crate::db::{CompilerDb, Db};
use crate::eval::Evaluator;
use crate::input::SourceFile;
use crate::ir::TestDef;
use crate::query::{parsed, typed, typed_pool};

use super::super::change_detection::{compute_skippable_and_update, TestRunCache};
use super::super::result::{FileSummary, TestOutcome, TestResult};
use super::{Backend, TestRunner, TestRunnerConfig};

impl TestRunner {
    /// Run all tests in a single file with a shared interner.
    ///
    /// This is the core implementation that creates a fresh `CompilerDb` per file
    /// while sharing the interner across all files. This allows parallel execution
    /// (each file gets its own Salsa query cache) while maintaining `Name` comparability
    /// (all `Name` values come from the same interner).
    #[expect(
        clippy::too_many_lines,
        reason = "multi-phase test file execution pipeline"
    )]
    pub(super) fn run_file_with_interner(
        path: &Path,
        interner: &crate::ir::SharedInterner,
        config: &TestRunnerConfig,
        cache: &parking_lot::Mutex<TestRunCache>,
    ) -> FileSummary {
        let mut summary = FileSummary::new(path.to_path_buf());

        // Read and parse the file
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                summary.add_error(format!("Failed to read file: {e}"));
                return summary;
            }
        };

        // Create a fresh CompilerDb with the shared interner.
        // Each file gets its own Salsa query cache, but all share the same interner
        // so Name values are comparable across files.
        let db = CompilerDb::with_interner(interner.clone());
        let file = SourceFile::new(&db, path.to_path_buf(), content);
        // Retrieve source from SourceFile for error matching (borrows from Salsa).
        // No clone needed: all subsequent `db` usage is shared borrows, so the
        // `&String` returned by `file.text(&db)` remains valid.
        let source = file.text(&db);

        // Parse the file
        let parse_result = parsed(&db, file);
        if parse_result.has_errors() {
            for error in &parse_result.errors {
                summary.add_error(format!("{}: {}", error.span(), error.message()));
            }
            return summary;
        }

        // Check if there are any tests
        if parse_result.module.tests.is_empty() {
            return summary;
        }

        let interner = db.interner();

        // Type check via Salsa query — ensures PoolCache is populated and
        // Salsa dependency tracking is consistent with the query pipeline.
        let type_result = typed(&db, file);
        let Some(pool) = typed_pool(&db, file) else {
            summary.add_error("internal error: Pool not cached after type checking".to_string());
            return summary;
        };

        // Canonicalize once for all tests (compile_fail and regular).
        // Runs even with type errors — pattern problems are independent.
        // Skip only if parse errors exist (AST may be malformed).
        // Store in CanonCache so downstream consumers (evaluator, LLVM) can reuse.
        let shared_canon =
            crate::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);

        // Incremental change detection. In worker mode the PARENT owns the
        // incremental cache and passes its skip decisions via config — the
        // worker never consults its own (always-fresh) cache. In-process runs
        // compute skip decisions against the runner-owned cache.
        let skippable: rustc_hash::FxHashSet<crate::ir::Name> = if config.worker_protocol {
            config
                .skip_unchanged
                .iter()
                .map(|name| interner.intern(name))
                .collect()
        } else if config.incremental {
            compute_skippable_and_update(cache, path, &shared_canon, &parse_result.module)
        } else {
            rustc_hash::FxHashSet::default()
        };

        // Separate compile_fail tests from regular tests
        // compile_fail tests don't need evaluation - they just check for type errors
        let (compile_fail_tests, mut regular_tests): (Vec<_>, Vec<_>) = parse_result
            .module
            .tests
            .iter()
            .partition(|t| t.is_compile_fail());

        Self::protocol_plan(
            compile_fail_tests
                .iter()
                .chain(regular_tests.iter())
                .copied(),
            config,
            interner,
        );

        // Effect-driven prioritization: effectful tests first, pure tests last.
        // Effectful tests are more likely to detect real regressions because they
        // exercise I/O paths and external interactions. Pure tests are deterministic
        // and cacheable, so running them last allows early failure detection.
        if config.incremental {
            Self::prioritize_tests(&mut regular_tests, &type_result.typed, interner);
        }

        // Run compile_fail tests first (they don't need load_module)
        for test in &compile_fail_tests {
            if !Self::test_passes_filter(test, config, interner) {
                continue;
            }
            // Honor `#skip(backend: "<name>")` for compile_fail tests too, in
            // parity with the regular-test partition below: a compile_fail test
            // naming the current backend emits a Skipped result instead of running.
            if let Some(reason) = Self::backend_skip_reason(test, config.backend) {
                let result = TestResult::skipped_for(test, reason, interner);
                Self::protocol_result(&result, config, interner);
                summary.add_result(result);
                continue;
            }
            Self::protocol_start(test.name, config, interner);

            let inner_result = Self::run_compile_fail_test(
                test,
                &type_result,
                &shared_canon.problems,
                source,
                interner,
                &pool,
            );

            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

            Self::protocol_result(&result, config, interner);
            summary.add_result(result);
        }

        // Skip regular test execution if there are no regular tests
        if regular_tests.is_empty() {
            return summary;
        }

        // Check for type errors before running regular tests.
        // Errors within compile_fail test bodies are expected and should not block
        // regular tests. Only errors OUTSIDE compile_fail tests indicate real problems.
        let non_compile_fail_errors =
            non_compile_fail_type_errors(&type_result, &compile_fail_tests);

        if !non_compile_fail_errors.is_empty() {
            // Type errors outside compile_fail tests block all regular tests.
            // For interpreter: these are real failures.
            // For LLVM: these are LLVM compile failures (type errors the interpreter
            // handles but LLVM can't codegen yet).
            let is_llvm = matches!(config.backend, Backend::LLVM);

            // Honor the name filter: blocked results are emitted only for
            // tests that would have run (and, in worker mode, only for tests
            // the plan records announced — an unfiltered emission here would
            // surface as unplanned-result protocol anomalies at the parent).
            for test in regular_tests
                .iter()
                .filter(|test| Self::test_passes_filter(test, config, interner))
            {
                let result = if is_llvm {
                    TestResult {
                        name: test.name,
                        targets: test.targets.clone(),
                        outcome: TestOutcome::LlvmCompileFail(
                            "blocked by type errors in file".to_string(),
                        ),
                        duration: Duration::ZERO,
                    }
                } else {
                    TestResult::failed(
                        test.name,
                        test.targets.clone(),
                        "blocked by type errors in file".to_string(),
                        Duration::ZERO,
                    )
                };
                Self::protocol_result(&result, config, interner);
                summary.add_result(result);
            }
            for error in non_compile_fail_errors {
                summary.add_error(error.format_with(&pool, interner));
            }
            if is_llvm {
                summary.llvm_compile_error = true;
            }
            return summary;
        }

        // Honor `#skip(backend: "<name>", reason: ...)` for the current backend:
        // emit a Skipped result for each test naming this backend (keeping the
        // plan/result accounting consistent for worker mode) and run only the
        // remainder. A test naming the OTHER backend still runs here.
        let backend_run_tests: Vec<&TestDef> = regular_tests
            .iter()
            .copied()
            .filter(|test| {
                if !Self::test_passes_filter(test, config, interner) {
                    return true;
                }
                match Self::backend_skip_reason(test, config.backend) {
                    Some(reason) => {
                        let result = TestResult::skipped_for(test, reason, interner);
                        Self::protocol_result(&result, config, interner);
                        summary.add_result(result);
                        false
                    }
                    None => true,
                }
            })
            .collect();

        // Run regular tests based on backend
        match config.backend {
            Backend::Interpreter => {
                Self::run_interpreter_tests(
                    &mut summary,
                    &db,
                    path,
                    &parse_result,
                    &backend_run_tests,
                    &shared_canon,
                    &skippable,
                    config,
                    interner,
                );
            }
            #[cfg(feature = "llvm")]
            Backend::LLVM => {
                // Use LLVM JIT backend — only pass regular_tests since
                // compile_fail tests are already handled in the common path above.
                Self::run_file_llvm(
                    &mut summary,
                    &db,
                    path,
                    &parse_result,
                    &backend_run_tests,
                    &type_result,
                    &pool,
                    &shared_canon,
                    &skippable,
                    interner,
                    config,
                );
            }
            #[cfg(not(feature = "llvm"))]
            Backend::LLVM => {
                summary.add_error(
                    "LLVM backend not available (compile with --features llvm)".to_string(),
                );
            }
        }

        summary
    }

    /// Run regular tests on the interpreter backend.
    ///
    /// The evaluator is constructed per FILE and shared by the file's tests;
    /// a caught test panic poisons it (interpreter state may be left
    /// inconsistent mid-unwind), so a fresh evaluator is rebuilt before the
    /// next test instead of reusing the poisoned one.
    #[expect(
        clippy::too_many_arguments,
        reason = "test runner mirrors the full compilation pipeline — all inputs are required"
    )]
    fn run_interpreter_tests(
        summary: &mut FileSummary,
        db: &CompilerDb,
        path: &Path,
        parse_result: &crate::parser::ParseOutput,
        regular_tests: &[&TestDef],
        shared_canon: &ori_ir::canon::SharedCanonResult,
        skippable: &rustc_hash::FxHashSet<crate::ir::Name>,
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
    ) {
        // Create evaluator in TestRun mode with type information.
        // TestRun mode: 500-depth recursion limit, test result collection.
        let build_evaluator = || -> Result<Evaluator, Vec<String>> {
            let mut evaluator = Evaluator::builder(interner, &parse_result.arena, db)
                .mode(ori_eval::EvalMode::TestRun {
                    only_attached: false,
                })
                .canon(shared_canon.clone())
                .build();
            evaluator.register_prelude();
            match evaluator.load_module(parse_result, path, Some(shared_canon)) {
                Ok(()) => Ok(evaluator),
                Err(errors) => Err(errors.iter().map(|e| e.message.clone()).collect()),
            }
        };

        let mut evaluator = match build_evaluator() {
            Ok(evaluator) => evaluator,
            Err(errors) => {
                for message in errors {
                    summary.add_error(message);
                }
                return;
            }
        };

        let mut poisoned = false;
        let mut rebuild_failed = false;
        for test in regular_tests {
            if !Self::test_passes_filter(test, config, interner) {
                continue;
            }

            // Incremental: skip tests whose targets are unchanged.
            if skippable.contains(&test.name) {
                let result = TestResult {
                    name: test.name,
                    targets: test.targets.clone(),
                    outcome: TestOutcome::SkippedUnchanged,
                    duration: Duration::ZERO,
                };
                Self::protocol_result(&result, config, interner);
                summary.add_result(result);
                continue;
            }

            // A prior test's caught panic may have left evaluator state
            // inconsistent: rebuild before running anything else on it.
            if poisoned && !rebuild_failed {
                match build_evaluator() {
                    Ok(fresh) => {
                        evaluator = fresh;
                        poisoned = false;
                    }
                    Err(errors) => {
                        rebuild_failed = true;
                        for message in errors {
                            summary.add_error(message);
                        }
                    }
                }
            }
            if poisoned {
                // Rebuild failed: fail the remaining tests loudly rather
                // than run them on poisoned evaluator state.
                let result = TestResult::failed(
                    test.name,
                    test.targets.clone(),
                    "not run: evaluator rebuild failed after a panicked test".to_string(),
                    Duration::ZERO,
                );
                Self::protocol_result(&result, config, interner);
                summary.add_result(result);
                continue;
            }

            Self::protocol_start(test.name, config, interner);
            let run = Self::run_single_test(&mut evaluator, test, interner);
            poisoned = run.evaluator_poisoned;

            // If #[fail] is present, wrap the result
            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(run.result, expected_failure, interner)
            } else {
                run.result
            };

            Self::protocol_result(&result, config, interner);
            summary.add_result(result);
        }
    }
}

/// Type errors OUTSIDE `compile_fail` test spans.
///
/// Errors within `compile_fail` test bodies are expected (those tests assert
/// them); only errors outside them block regular tests.
pub(super) fn non_compile_fail_type_errors<'a>(
    type_result: &'a ori_types::TypeCheckResult,
    compile_fail_tests: &[&TestDef],
) -> Vec<&'a ori_types::TypeCheckError> {
    let compile_fail_spans: Vec<_> = compile_fail_tests.iter().map(|t| t.span).collect();
    type_result
        .errors()
        .iter()
        .filter(|error| {
            let error_span = error.span();
            // Keep the error if it is NOT contained in any compile_fail test span.
            !compile_fail_spans
                .iter()
                .any(|test_span| test_span.contains_span(error_span))
        })
        .collect()
}
