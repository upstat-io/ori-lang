//! Test execution and result collection helpers.
//!
//! Handles running individual test functions, capturing output,
//! and collecting pass/fail results.

use std::time::Instant;

use ori_types::{Pool, TypeCheckResult};

use crate::eval::Evaluator;
use crate::ir::TestDef;

use super::super::error_matching::{
    format_actual, format_const_problem, format_expected, format_pattern_problem, match_all_errors,
};
use super::super::result::{TestOutcome, TestResult};
use super::TestRunner;

/// Outcome of one interpreter test run plus evaluator-state health.
pub(super) struct SingleTestRun {
    /// The test's reported result.
    pub(super) result: TestResult,
    /// Whether a caught Rust panic may have left the shared per-file
    /// evaluator inconsistent. The caller MUST rebuild the evaluator before
    /// running another test on it.
    pub(super) evaluator_poisoned: bool,
}

/// Errors and problems selected to match a `compile_fail` test's expectations
/// against — span-local when the test's own span produced any, else every
/// module-level error/problem.
struct ErrorsToMatch<'a> {
    type_errors: Vec<&'a ori_types::TypeCheckError>,
    pattern_problems: Vec<&'a ori_ir::canon::PatternProblem>,
    const_problems: Vec<&'a ori_ir::canon::ConstEvalProblem>,
}

/// Select span-filtered errors first for better isolation between tests in
/// the same file; fall back to every module-level error/problem when the
/// test's own span produced none (module-level errors, like a missing impl
/// member, are not attributable to any single test's span).
fn select_errors_to_match<'a>(
    test: &TestDef,
    type_result: &'a TypeCheckResult,
    pattern_problems: &'a [ori_ir::canon::PatternProblem],
    const_problems: &'a [ori_ir::canon::ConstEvalProblem],
) -> ErrorsToMatch<'a> {
    let test_type_errors: Vec<_> = type_result
        .errors()
        .iter()
        .filter(|e| test.span.contains_span(e.span()))
        .collect();
    let test_pattern_problems: Vec<_> = pattern_problems
        .iter()
        .filter(|p| {
            let span = match p {
                ori_ir::canon::PatternProblem::NonExhaustive { match_span, .. } => *match_span,
                ori_ir::canon::PatternProblem::RedundantArm { arm_span, .. } => *arm_span,
            };
            test.span.contains_span(span)
        })
        .collect();
    let test_const_problems: Vec<_> = const_problems
        .iter()
        .filter(|problem| test.span.contains_span(problem.span))
        .collect();
    let use_all_module_errors = test_type_errors.is_empty()
        && test_pattern_problems.is_empty()
        && test_const_problems.is_empty();
    if use_all_module_errors {
        ErrorsToMatch {
            type_errors: type_result.errors().iter().collect(),
            pattern_problems: pattern_problems.iter().collect(),
            const_problems: const_problems.iter().collect(),
        }
    } else {
        ErrorsToMatch {
            type_errors: test_type_errors,
            pattern_problems: test_pattern_problems,
            const_problems: test_const_problems,
        }
    }
}

/// Build the failure result for a `compile_fail` test whose expected errors
/// never materialized (compilation succeeded).
fn expected_failure_but_compilation_succeeded(
    test: &TestDef,
    interner: &crate::ir::StringInterner,
    start: Instant,
) -> TestResult {
    let expected_strs: Vec<String> = test
        .expected_errors
        .iter()
        .map(|e| format_expected(e, interner))
        .collect();
    let error_word = if test.expected_errors.len() == 1 {
        "error"
    } else {
        "errors"
    };
    TestResult::failed(
        test.name,
        test.targets.clone(),
        format!(
            "expected compilation to fail with {} {error_word}, but compilation succeeded. Expected: {}",
            test.expected_errors.len(),
            expected_strs.join("; ")
        ),
        start.elapsed(),
    )
}

/// Build the failure result for a `compile_fail` test whose actual errors
/// did not match every expectation.
fn unmatched_expectations_result(
    test: &TestDef,
    match_result: &crate::test::error_matching::MatchResult,
    errors: &ErrorsToMatch<'_>,
    source: &str,
    pool: &Pool,
    interner: &crate::ir::StringInterner,
    start: Instant,
) -> TestResult {
    let unmatched: Vec<String> = match_result
        .unmatched_expectations
        .iter()
        .map(|&i| format_expected(&test.expected_errors[i], interner))
        .collect();
    let mut actual: Vec<String> = errors
        .type_errors
        .iter()
        .map(|e| format_actual(e, source, pool, interner))
        .collect();
    actual.extend(
        errors
            .pattern_problems
            .iter()
            .map(|p| format_pattern_problem(p, source)),
    );
    actual.extend(
        errors
            .const_problems
            .iter()
            .map(|problem| format_const_problem(problem, source, interner)),
    );
    TestResult::failed(
        test.name,
        test.targets.clone(),
        format!(
            "unmatched expectations: [{}]. Actual errors: [{}]",
            unmatched.join(", "),
            actual.join(", ")
        ),
        start.elapsed(),
    )
}

impl TestRunner {
    /// Run a `compile_fail` test.
    ///
    /// The test passes if all expected errors are matched by actual errors.
    /// Matches against both type errors and pattern problems (exhaustiveness/
    /// redundancy from canonicalization).
    ///
    /// Error matching strategy:
    /// 1. First try to match errors within this test's span (isolation for tests
    ///    that produce errors in their body, like `add("hello", 2)`)
    /// 2. If no errors in test span, fall back to matching all module errors
    ///    (for tests checking module-level errors like missing impl members)
    pub(super) fn run_compile_fail_test(
        test: &TestDef,
        type_result: &TypeCheckResult,
        pattern_problems: &[ori_ir::canon::PatternProblem],
        const_problems: &[ori_ir::canon::ConstEvalProblem],
        source: &str,
        interner: &crate::ir::StringInterner,
        pool: &Pool,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            return TestResult::skipped_for(test, reason, interner);
        }

        let start = Instant::now();

        let errors = select_errors_to_match(test, type_result, pattern_problems, const_problems);
        if errors.type_errors.is_empty()
            && errors.pattern_problems.is_empty()
            && errors.const_problems.is_empty()
        {
            return expected_failure_but_compilation_succeeded(test, interner, start);
        }

        let match_result = match_all_errors(
            &errors.type_errors,
            &errors.pattern_problems,
            &errors.const_problems,
            &test.expected_errors,
            source,
            interner,
            pool,
        );

        if match_result.all_matched() {
            TestResult::passed(test.name, test.targets.clone(), start.elapsed())
        } else {
            unmatched_expectations_result(
                test,
                &match_result,
                &errors,
                source,
                pool,
                interner,
                start,
            )
        }
    }

    /// Apply the #[fail] wrapper to a test result.
    ///
    /// The #[fail] attribute expects the inner test to fail.
    /// - If inner test failed with expected message: wrapper passes
    /// - If inner test failed with different message: wrapper fails
    /// - If inner test passed: wrapper fails (expected failure didn't happen)
    /// - If inner test was skipped, unchanged-skipped, or blocked by an LLVM
    ///   compile failure: it never ran, so the result passes through unchanged
    pub(super) fn apply_fail_wrapper(
        inner_result: TestResult,
        expected_failure: crate::ir::Name,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        let expected_substr = interner.lookup(expected_failure);

        match inner_result.outcome {
            TestOutcome::Skipped(_)
            | TestOutcome::SkippedUnchanged
            | TestOutcome::LlvmCompileFail(_) => {
                // The inner test never ran (skipped, unchanged, or blocked by
                // an LLVM compile failure): pass the result through unchanged
                // rather than judging it against #[fail].
                inner_result
            }
            TestOutcome::Passed => {
                // Inner test passed, but we expected it to fail
                TestResult::failed(
                    inner_result.name,
                    inner_result.targets,
                    format!("expected test to fail with '{expected_substr}', but test passed"),
                    inner_result.duration,
                )
            }
            TestOutcome::Failed(ref error) => {
                // Inner test failed - check if it's the expected failure
                if error.contains(expected_substr) {
                    // Expected failure occurred - this is a pass
                    TestResult::passed(
                        inner_result.name,
                        inner_result.targets,
                        inner_result.duration,
                    )
                } else {
                    // Wrong failure message
                    TestResult::failed(
                        inner_result.name,
                        inner_result.targets,
                        format!(
                            "expected failure containing '{expected_substr}', but got: {error}"
                        ),
                        inner_result.duration,
                    )
                }
            }
        }
    }

    /// Sort tests by effect class: effectful first, pure last.
    ///
    /// Effectful tests (targets with capabilities like `Http`, `FileSystem`) are more
    /// likely to catch real regressions because they exercise I/O paths. Pure tests
    /// (targets with no capabilities) are deterministic and cacheable, so running
    /// them last allows failures to surface sooner.
    pub(super) fn prioritize_tests(
        tests: &mut [&TestDef],
        typed: &ori_types::TypedModule,
        interner: &crate::ir::StringInterner,
    ) {
        tests.sort_by(|a, b| {
            let effect_a = Self::max_target_effect(a, typed, interner);
            let effect_b = Self::max_target_effect(b, typed, interner);
            // Reverse: HasEffects (2) > ReadsOnly (1) > Pure (0)
            // We want HasEffects first, so reverse the comparison.
            effect_b.cmp(&effect_a)
        });
    }

    /// Get the maximum effect class across a test's targets.
    ///
    /// If any target has `HasEffects`, the test is effectful.
    /// If any target has `ReadsOnly` (and none has `HasEffects`), it's read-only.
    /// Otherwise it's pure.
    fn max_target_effect(
        test: &TestDef,
        typed: &ori_types::TypedModule,
        interner: &crate::ir::StringInterner,
    ) -> ori_types::EffectClass {
        use ori_types::EffectClass;

        let mut max_effect = EffectClass::Pure;

        for &target in &test.targets {
            if let Some(sig) = typed.function(target) {
                let effect = sig.effect_class(interner);
                if effect > max_effect {
                    max_effect = effect;
                }
                if max_effect == EffectClass::HasEffects {
                    // Short-circuit: can't get higher.
                    return max_effect;
                }
            }
        }

        max_effect
    }

    /// Run a single test on the shared per-file evaluator.
    ///
    /// `evaluator_poisoned` is set when a Rust panic was caught during
    /// evaluation: the evaluator's interpreter state (scopes, environment,
    /// in-flight bookkeeping) may be inconsistent after an unwind, so the
    /// caller rebuilds a fresh evaluator before the next test.
    pub(super) fn run_single_test(
        evaluator: &mut Evaluator,
        test: &TestDef,
        interner: &crate::ir::StringInterner,
    ) -> SingleTestRun {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            return SingleTestRun {
                result: TestResult::skipped_for(test, reason, interner),
                evaluator_poisoned: false,
            };
        }

        // Time the test execution
        let start = Instant::now();

        let Some(can_id) = evaluator.canon_root_for(test.name) else {
            return SingleTestRun {
                result: TestResult::failed(
                    test.name,
                    test.targets.clone(),
                    "internal error: test has no canonical root".to_string(),
                    start.elapsed(),
                ),
                evaluator_poisoned: false,
            };
        };
        // Harness contract: a Rust panic inside evaluation is a FAILED test,
        // never a dead runner — catch it and continue with the next test.
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| evaluator.eval_can(can_id)));
        // Drain buffered print() output to stderr on every exit path (pass,
        // fail, or caught panic) before the caller drops/rebuilds the
        // evaluator; no-op for the non-capturing stdout handler (Text mode).
        if let Some(block) = stderr_drain_block(evaluator.print_output()) {
            eprint!("{block}");
            evaluator.clear_print_output();
        }
        match result {
            Ok(Ok(_)) => SingleTestRun {
                result: TestResult::passed(test.name, test.targets.clone(), start.elapsed()),
                evaluator_poisoned: false,
            },
            Ok(Err(e)) => SingleTestRun {
                result: TestResult::failed(
                    test.name,
                    test.targets.clone(),
                    e.into_eval_error().message,
                    start.elapsed(),
                ),
                evaluator_poisoned: false,
            },
            Err(payload) => SingleTestRun {
                result: TestResult::failed(
                    test.name,
                    test.targets.clone(),
                    format!(
                        "test panicked (evaluator): {}",
                        super::panic_message(payload.as_ref())
                    ),
                    start.elapsed(),
                ),
                evaluator_poisoned: true,
            },
        }
    }
}

/// Render captured per-test print output for the stderr drain.
///
/// Returns `None` for empty capture; otherwise the output with a trailing
/// newline normalized so consecutive tests' fragments never concatenate on
/// the shared stderr stream.
pub(super) fn stderr_drain_block(mut captured: String) -> Option<String> {
    if captured.is_empty() {
        return None;
    }
    if !captured.ends_with('\n') {
        captured.push('\n');
    }
    Some(captured)
}

#[cfg(test)]
mod tests;
