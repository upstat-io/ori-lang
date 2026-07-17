//! LLVM JIT backend for the test runner.
//!
//! Compiles test functions via the LLVM pipeline for JIT execution,
//! including cross-module type re-interning and ARC lowering.

use std::path::Path;
use std::time::{Duration, Instant};

use ori_types::TypeCheckResult;

use super::super::result::{FileSummary, TestOutcome, TestResult};
use super::TestRunner;
use super::TestRunnerConfig;

mod compile;
mod imports;

#[derive(Clone, Copy)]
pub(super) struct LlvmTestFile<'a> {
    pub(super) db: &'a crate::db::CompilerDb,
    pub(super) path: &'a Path,
    pub(super) parse: &'a crate::parser::ParseOutput,
    pub(super) typed: &'a TypeCheckResult,
    pub(super) pool: &'a ori_types::Pool,
    pub(super) canon: &'a ori_ir::canon::SharedCanonResult,
    pub(super) interner: &'a crate::ir::StringInterner,
}

#[derive(Clone, Copy)]
pub(super) struct LlvmTestSelection<'a> {
    pub(super) tests: &'a [&'a crate::ir::TestDef],
    pub(super) skippable: &'a rustc_hash::FxHashSet<crate::ir::Name>,
    pub(super) config: &'a TestRunnerConfig,
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
    pub(super) fn run_file_llvm(
        summary: &mut FileSummary,
        file: LlvmTestFile<'_>,
        selection: LlvmTestSelection<'_>,
    ) {
        let LlvmTestSelection {
            tests: regular_tests,
            skippable,
            config,
        } = selection;
        let interner = file.interner;
        if regular_tests.is_empty() {
            return;
        }

        let (skipped_unchanged, filtered_tests): (Vec<_>, Vec<_>) = regular_tests
            .iter()
            .filter(|test| Self::test_passes_filter(test, config, interner))
            .copied()
            .partition(|test| skippable.contains(&test.name));
        for test in skipped_unchanged {
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

        ori_llvm::install_fatal_error_handler();
        match compile::compile_and_run(file, &filtered_tests, config) {
            Ok(results) => {
                for result in results {
                    summary.add_result(result);
                }
            }
            Err(failure) => {
                summary.add_error(failure.summary);
                summary.llvm_compile_error = true;
                Self::add_compile_fail_results(
                    summary,
                    &filtered_tests,
                    &failure.test_result,
                    interner,
                    config,
                );
            }
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
