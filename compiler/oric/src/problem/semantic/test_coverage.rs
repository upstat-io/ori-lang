//! Test coverage analysis and pattern problem conversion.
//!
//! Free functions consumed by `check.rs` and `watch.rs` for test coverage
//! verification and pattern exhaustiveness diagnostics.

use crate::diagnostic::{Diagnostic, ErrorCode};
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

/// Convert a Canon-owned constant-evaluation problem into the stable E2058
/// diagnostic family.
#[cold]
pub fn const_eval_problem_to_diagnostic(
    problem: &ori_canon::ConstEvalProblem,
    interner: &StringInterner,
) -> Diagnostic {
    use ori_canon::ConstEvalProblemKind;

    let name = interner.lookup(problem.name);
    let base = Diagnostic::error(ErrorCode::E2058);
    match &problem.kind {
        ConstEvalProblemKind::CircularDependency { dependency } => base
            .with_message(format!(
                "constant '${name}' cannot be evaluated because its dependency graph is circular"
            ))
            .with_label(
                problem.span,
                format!(
                    "evaluation returns to '${}' before producing a value",
                    interner.lookup(*dependency)
                ),
            )
            .with_suggestion(
                "break the cycle so every constant ultimately depends only on literals or earlier evaluated constants",
            ),
        ConstEvalProblemKind::UnresolvedReference { reference } => base
            .with_message(format!(
                "constant '${name}' cannot be evaluated because '${}' has no compile-time value",
                interner.lookup(*reference)
            ))
            .with_label(problem.span, "this initializer needs an unavailable constant")
            .with_suggestion(
                "declare the referenced constant, import it explicitly, or replace it with a compile-time value",
            ),
        ConstEvalProblemKind::DivisionByZero => base
            .with_message(format!(
                "constant '${name}' cannot be evaluated because it divides by zero"
            ))
            .with_label(problem.span, "division or remainder by zero occurs here")
            .with_suggestion("use a non-zero divisor or guard the operation with a constant condition"),
        ConstEvalProblemKind::Overflow => base
            .with_message(format!(
                "constant '${name}' cannot be evaluated because its arithmetic overflows"
            ))
            .with_label(problem.span, "the result is outside the constant value range")
            .with_suggestion("use smaller operands or restructure the calculation to remain in range"),
        ConstEvalProblemKind::UnsupportedExpression { form } => base
            .with_message(format!(
                "constant '${name}' cannot yet be frozen from {form}"
            ))
            .with_label(problem.span, "this initializer is outside the current constant-value domain")
            .with_suggestion(
                "rewrite the initializer using literals, primitive arithmetic or logic, conditionals, and evaluated `$` constants",
            ),
        ConstEvalProblemKind::ImportedValueUnavailable { module } => base
            .with_message(format!(
                "imported constant '${name}' has no evaluated value from '{module}'"
            ))
            .with_label(problem.span, "the provider failed to freeze this selected constant")
            .with_suggestion(format!(
                "compile '{module}' directly and fix its constant-evaluation error before importing '${name}'"
            )),
    }
}

/// Convert every constant-evaluation failure without losing its individual
/// source span, cause, or repair suggestion.
pub fn const_eval_problems_to_diagnostics(
    problems: &[ori_canon::ConstEvalProblem],
    interner: &StringInterner,
) -> Vec<Diagnostic> {
    problems
        .iter()
        .map(|problem| const_eval_problem_to_diagnostic(problem, interner))
        .collect()
}

/// Stable text fallback for consumers that cannot render structured
/// diagnostics directly.
pub fn const_eval_problems_summary(
    problems: &[ori_canon::ConstEvalProblem],
    interner: &StringInterner,
) -> String {
    const_eval_problems_to_diagnostics(problems, interner)
        .into_iter()
        .map(|diagnostic| {
            format!(
                "error[{}]: {}",
                diagnostic.code.as_str(),
                diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}
