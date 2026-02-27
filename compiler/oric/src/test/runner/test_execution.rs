//! Test execution and result collection helpers.
//!
//! Handles running individual test functions, capturing output,
//! managing timeouts, and collecting pass/fail results.

use std::time::Instant;

use ori_types::TypeCheckResult;

use crate::eval::Evaluator;
use crate::ir::TestDef;

use super::super::error_matching::{
    format_actual, format_expected, format_pattern_problem, match_all_errors,
};
use super::super::result::{TestOutcome, TestResult};
use super::TestRunner;

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
        source: &str,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test.name, test.targets.clone(), reason_str);
        }

        let start = Instant::now();

        // Try span-filtered errors first for better isolation.
        // This helps when multiple compile_fail tests exist in the same file,
        // each should only see errors from their own body.
        let test_type_errors: Vec<_> = type_result
            .errors()
            .iter()
            .filter(|e| test.span.contains_span(e.span()))
            .collect();

        // Filter pattern problems by test span too.
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

        // If no errors within test span, use all module errors.
        // This handles tests that check for module-level errors (like missing
        // associated types in impl blocks) where the error is outside the test body.
        let type_errors_to_match: Vec<&_> =
            if test_type_errors.is_empty() && test_pattern_problems.is_empty() {
                type_result.errors().iter().collect()
            } else {
                test_type_errors
            };

        let pattern_problems_to_match: Vec<&_> = if type_errors_to_match.len()
            == type_result.errors().len()
            && test_pattern_problems.is_empty()
        {
            // Fell back to all module errors — also use all pattern problems.
            pattern_problems.iter().collect()
        } else {
            test_pattern_problems
        };

        // If no errors were produced but we expected some
        if type_errors_to_match.is_empty() && pattern_problems_to_match.is_empty() {
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
            return TestResult::failed(
                test.name,
                test.targets.clone(),
                format!(
                    "expected compilation to fail with {} {error_word}, but compilation succeeded. Expected: {}",
                    test.expected_errors.len(),
                    expected_strs.join("; ")
                ),
                start.elapsed(),
            );
        }

        // Match actual errors (type + pattern) against expectations
        let match_result = match_all_errors(
            &type_errors_to_match,
            &pattern_problems_to_match,
            &test.expected_errors,
            source,
            interner,
        );

        if match_result.all_matched() {
            // All expectations matched - test passes
            TestResult::passed(test.name, test.targets.clone(), start.elapsed())
        } else {
            // Some expectations were not matched
            let unmatched: Vec<String> = match_result
                .unmatched_expectations
                .iter()
                .map(|&i| format_expected(&test.expected_errors[i], interner))
                .collect();

            let mut actual: Vec<String> = type_errors_to_match
                .iter()
                .map(|e| format_actual(e, source))
                .collect();
            actual.extend(
                pattern_problems_to_match
                    .iter()
                    .map(|p| format_pattern_problem(p, source)),
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
    }

    /// Apply the #[fail] wrapper to a test result.
    ///
    /// The #[fail] attribute expects the inner test to fail.
    /// - If inner test failed with expected message: wrapper passes
    /// - If inner test failed with different message: wrapper fails
    /// - If inner test passed: wrapper fails (expected failure didn't happen)
    /// - If inner test was skipped: remains skipped
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
                // Skipped and expected-failure tests pass through unchanged
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
                    return max_effect; // Short-circuit: can't get higher
                }
            }
        }

        max_effect
    }

    /// Run a single test.
    pub(super) fn run_single_test(
        evaluator: &mut Evaluator,
        test: &TestDef,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test.name, test.targets.clone(), reason_str);
        }

        // Time the test execution
        let start = Instant::now();

        let Some(can_id) = evaluator.canon_root_for(test.name) else {
            return TestResult::failed(
                test.name,
                test.targets.clone(),
                "internal error: test has no canonical root".to_string(),
                start.elapsed(),
            );
        };
        let result = evaluator.eval_can(can_id);
        match result {
            Ok(_) => TestResult::passed(test.name, test.targets.clone(), start.elapsed()),
            Err(e) => TestResult::failed(
                test.name,
                test.targets.clone(),
                e.into_eval_error().message,
                start.elapsed(),
            ),
        }
    }
}
