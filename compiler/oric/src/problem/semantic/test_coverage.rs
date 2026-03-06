//! Test coverage analysis and pattern problem conversion.
//!
//! Free functions consumed by `check.rs` and `watch.rs` for test coverage
//! verification and pattern exhaustiveness diagnostics.

use crate::diagnostic::Diagnostic;
use crate::ir::{Name, StringInterner};

use super::SemanticProblem;

/// Check that every function (except `@main`) has at least one test targeting it.
///
/// Returns a `SemanticProblem::MissingTest` for each untested function. This
/// centralizes test coverage analysis so all consumers (check command, test runner,
/// future `ori lint`) use the same logic.
pub fn check_test_coverage(
    module: &crate::ir::Module,
    interner: &StringInterner,
) -> Vec<SemanticProblem> {
    let main_name = interner.intern("main");

    let mut tested: rustc_hash::FxHashSet<Name> = rustc_hash::FxHashSet::default();
    for test in &module.tests {
        for target in &test.targets {
            tested.insert(*target);
        }
    }

    module
        .functions
        .iter()
        .filter(|f| f.name != main_name && !tested.contains(&f.name))
        .map(|f| SemanticProblem::MissingTest {
            span: f.span,
            func_name: f.name,
        })
        .collect()
}

/// Convert an [`ori_canon::PatternProblem`] into a [`Diagnostic`] via [`SemanticProblem`].
///
/// Pattern problems originate from the canonicalizer's exhaustiveness/redundancy
/// checker. This function centralizes the mapping so all consumers (check command,
/// test runner, future commands) use the same conversion.
#[cold]
pub fn pattern_problem_to_diagnostic(
    problem: &ori_canon::PatternProblem,
    interner: &StringInterner,
) -> Diagnostic {
    let semantic = match problem {
        ori_canon::PatternProblem::NonExhaustive {
            match_span,
            missing,
        } => SemanticProblem::NonExhaustiveMatch {
            span: *match_span,
            missing_patterns: missing.clone(),
        },
        ori_canon::PatternProblem::RedundantArm {
            arm_span,
            match_span,
            ..
        } => SemanticProblem::RedundantPattern {
            span: *arm_span,
            covered_by_span: *match_span,
        },
    };
    semantic.into_diagnostic(interner)
}
