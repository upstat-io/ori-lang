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
use super::{Backend, OutputFormat, TestRunner, TestRunnerConfig};

struct CompileFailContext<'a> {
    type_result: &'a ori_types::TypeCheckResult,
    pattern_problems: &'a [ori_ir::canon::PatternProblem],
    const_problems: &'a [ori_ir::canon::ConstEvalProblem],
    source: &'a str,
    interner: &'a crate::ir::StringInterner,
    pool: &'a ori_types::Pool,
    config: &'a TestRunnerConfig,
}

struct RegularRunContext<'a> {
    db: &'a CompilerDb,
    path: &'a Path,
    parse: &'a ori_parse::ParseOutput,
    typed: &'a ori_types::TypeCheckResult,
    pool: &'a ori_types::Pool,
    canon: &'a ori_ir::canon::SharedCanonResult,
    skippable: &'a rustc_hash::FxHashSet<crate::ir::Name>,
    config: &'a TestRunnerConfig,
    interner: &'a crate::ir::StringInterner,
}

impl TestRunner {
    /// Read `path`, create a fresh per-file `CompilerDb` sharing `interner`, and
    /// parse it. Returns `None` (with the failure already recorded on
    /// `summary`) when the file cannot be read, fails to parse, or has no
    /// tests — in every such case the caller returns `summary` immediately.
    fn read_and_parse_file(
        path: &Path,
        interner: &crate::ir::SharedInterner,
        summary: &mut FileSummary,
    ) -> Option<(CompilerDb, SourceFile, ori_parse::ParseOutput)> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                summary.add_error(format!("Failed to read file: {e}"));
                return None;
            }
        };
        // Each file gets its own Salsa query cache, but all share the same
        // interner so Name values are comparable across files.
        let db = CompilerDb::with_interner(interner.clone());
        let file = SourceFile::new(&db, path.to_path_buf(), content);
        let parse_result = parsed(&db, file);
        if parse_result.has_errors() {
            for error in &parse_result.errors {
                summary.add_error(format!("{}: {}", error.span(), error.message()));
            }
            return None;
        }
        if parse_result.module.tests.is_empty() {
            return None;
        }
        Some((db, file, parse_result))
    }

    /// Run all tests in a single file with a shared interner.
    ///
    /// This is the core implementation that creates a fresh `CompilerDb` per file
    /// while sharing the interner across all files. This allows parallel execution
    /// (each file gets its own Salsa query cache) while maintaining `Name` comparability
    /// (all `Name` values come from the same interner).
    pub(super) fn run_file_with_interner(
        path: &Path,
        interner: &crate::ir::SharedInterner,
        config: &TestRunnerConfig,
        cache: &parking_lot::Mutex<TestRunCache>,
    ) -> FileSummary {
        let mut summary = FileSummary::new(path.to_path_buf());

        let Some((db, file, parse_result)) =
            Self::read_and_parse_file(path, interner, &mut summary)
        else {
            return summary;
        };
        // Retrieve source from SourceFile for error matching (borrows from Salsa).
        // No clone needed: all subsequent `db` usage is shared borrows, so the
        // `&String` returned by `file.text(&db)` remains valid.
        let source = file.text(&db);
        let interner = db.interner();

        // Type check via Salsa query — ensures PoolCache is populated and
        // Salsa dependency tracking is consistent with the query pipeline.
        let type_result = typed(&db, file);
        let Some(pool) = typed_pool(&db, file) else {
            summary.add_error("internal error: Pool not cached after type checking".to_string());
            return summary;
        };

        // Canonicalize once for all tests (compile_fail and regular); runs
        // even with type errors since pattern problems are independent, and
        // skips only on parse errors. Cached for downstream reuse (eval, LLVM).
        let shared_canon =
            crate::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);

        // Incremental change detection. In worker mode the parent owns the
        // cache and passes skip decisions via config (the worker's own cache
        // is always-fresh and unused); in-process runs compute skips locally.
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
        // compile_fail tests don't need evaluation - they check type errors
        // and pattern problems (exhaustiveness/redundancy) instead
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

        // Why: effectful tests run first (more likely to surface real
        // regressions); pure tests are cacheable and run last.
        if config.incremental {
            Self::prioritize_tests(&mut regular_tests, &type_result.typed, interner);
        }

        Self::run_compile_fail_tests(
            &mut summary,
            &compile_fail_tests,
            &CompileFailContext {
                type_result: &type_result,
                pattern_problems: &shared_canon.problems,
                const_problems: &shared_canon.const_problems,
                source,
                interner,
                pool: &pool,
                config,
            },
        );

        // Skip regular test execution if there are no regular tests
        if regular_tests.is_empty() {
            return summary;
        }

        // Check for type errors before running regular tests.
        // Errors within compile_fail test bodies are expected and should not block
        // regular tests. Only errors OUTSIDE compile_fail tests indicate real problems.
        let non_compile_fail_errors =
            non_compile_fail_type_errors(&type_result, &compile_fail_tests);

        if Self::record_blocked_regular_tests(
            &mut summary,
            &regular_tests,
            &non_compile_fail_errors,
            config,
            interner,
            &pool,
        ) {
            return summary;
        }
        if Self::record_blocked_constant_tests(
            &mut summary,
            &regular_tests,
            &shared_canon.const_problems,
            config,
            interner,
        ) {
            return summary;
        }

        Self::run_regular_backend(
            &mut summary,
            &regular_tests,
            &RegularRunContext {
                db: &db,
                path,
                parse: &parse_result,
                typed: &type_result,
                pool: &pool,
                canon: &shared_canon,
                skippable: &skippable,
                config,
                interner,
            },
        );

        summary
    }

    fn run_compile_fail_tests(
        summary: &mut FileSummary,
        tests: &[&TestDef],
        context: &CompileFailContext<'_>,
    ) {
        for test in tests {
            if !Self::test_passes_filter(test, context.config, context.interner) {
                continue;
            }
            if let Some(reason) = Self::backend_skip_reason(test, context.config.backend) {
                let result = TestResult::skipped_for(test, reason, context.interner);
                Self::protocol_result(&result, context.config, context.interner);
                summary.add_result(result);
                continue;
            }
            Self::protocol_start(test.name, context.config, context.interner);
            let inner = Self::run_compile_fail_test(
                test,
                context.type_result,
                context.pattern_problems,
                context.const_problems,
                context.source,
                context.interner,
                context.pool,
            );
            let result = match test.fail_expected {
                Some(expected) => Self::apply_fail_wrapper(inner, expected, context.interner),
                None => inner,
            };
            Self::protocol_result(&result, context.config, context.interner);
            summary.add_result(result);
        }
    }

    fn record_blocked_regular_tests(
        summary: &mut FileSummary,
        tests: &[&TestDef],
        errors: &[&ori_types::TypeCheckError],
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
        pool: &ori_types::Pool,
    ) -> bool {
        if errors.is_empty() {
            return false;
        }
        let is_llvm = matches!(config.backend, Backend::LLVM);
        for test in tests
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
        for error in errors {
            summary.add_error(error.format_with(pool, interner));
        }
        summary.llvm_compile_error = is_llvm;
        true
    }

    fn record_blocked_constant_tests(
        summary: &mut FileSummary,
        tests: &[&TestDef],
        problems: &[ori_ir::canon::ConstEvalProblem],
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
    ) -> bool {
        if problems.is_empty() {
            return false;
        }

        let is_llvm = matches!(config.backend, Backend::LLVM);
        for test in tests
            .iter()
            .filter(|test| Self::test_passes_filter(test, config, interner))
        {
            let result = if is_llvm {
                TestResult {
                    name: test.name,
                    targets: test.targets.clone(),
                    outcome: TestOutcome::LlvmCompileFail(
                        "blocked by constant evaluation errors in file".to_string(),
                    ),
                    duration: Duration::ZERO,
                }
            } else {
                TestResult::failed(
                    test.name,
                    test.targets.clone(),
                    "blocked by constant evaluation errors in file".to_string(),
                    Duration::ZERO,
                )
            };
            Self::protocol_result(&result, config, interner);
            summary.add_result(result);
        }

        for problem in problems {
            let diagnostic =
                crate::problem::semantic::const_eval_problem_to_diagnostic(problem, interner);
            summary.add_error(format!(
                "error[{}]: {}",
                diagnostic.code.as_str(),
                diagnostic.message
            ));
        }
        summary.llvm_compile_error = is_llvm;
        true
    }

    fn run_regular_backend(
        summary: &mut FileSummary,
        tests: &[&TestDef],
        context: &RegularRunContext<'_>,
    ) {
        let selected: Vec<&TestDef> = tests
            .iter()
            .copied()
            .filter(|test| {
                if !Self::test_passes_filter(test, context.config, context.interner) {
                    return true;
                }
                let Some(reason) = Self::backend_skip_reason(test, context.config.backend) else {
                    return true;
                };
                let result = TestResult::skipped_for(test, reason, context.interner);
                Self::protocol_result(&result, context.config, context.interner);
                summary.add_result(result);
                false
            })
            .collect();
        match context.config.backend {
            Backend::Interpreter => Self::run_interpreter_tests(
                summary,
                context.db,
                context.path,
                context.parse,
                &selected,
                context.canon,
                context.skippable,
                context.config,
                context.interner,
            ),
            #[cfg(feature = "llvm")]
            Backend::LLVM => Self::run_file_llvm(
                summary,
                super::llvm_backend::LlvmTestFile {
                    db: context.db,
                    path: context.path,
                    parse: context.parse,
                    typed: context.typed,
                    pool: context.pool,
                    canon: context.canon,
                    interner: context.interner,
                },
                super::llvm_backend::LlvmTestSelection {
                    tests: &selected,
                    skippable: context.skippable,
                    config: context.config,
                },
            ),
            #[cfg(not(feature = "llvm"))]
            Backend::LLVM => summary
                .add_error("LLVM backend not available (compile with --features llvm)".to_string()),
        }
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
            let mut builder = Evaluator::builder(interner, &parse_result.arena, db)
                .mode(ori_eval::EvalMode::TestRun {
                    only_attached: false,
                })
                .canon(shared_canon.clone());
            // In json mode stdout is the machine-readable summary channel:
            // buffer print() output instead of contaminating it (drained to
            // stderr after each test runs); text mode keeps output inline.
            if config.format == OutputFormat::Json {
                builder = builder.print_handler(ori_eval::buffer_handler());
            }
            let mut evaluator = builder.build();
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
