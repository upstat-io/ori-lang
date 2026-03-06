//! The `check` command: type-check an Ori source file and optionally verify test coverage.

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_diagnostic::Severity;
use oric::problem::semantic::{check_test_coverage, pattern_problem_to_diagnostic};
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

use super::read_file;
use super::report_frontend_errors;
use super::TestEnforcement;

/// Type-check a file and verify test coverage based on enforcement level.
///
/// Accumulates all errors (parse, type, and coverage) before exiting, giving
/// the user a complete picture of issues rather than stopping at the first error.
/// Test coverage enforcement is controlled by the `enforcement` parameter:
/// - `Off`: skip test coverage check entirely
/// - `Warn`: emit missing-test diagnostics as warnings
/// - `Error`: emit missing-test diagnostics as errors (strict mode)
pub fn check_file(path: &str, enforcement: TestEnforcement) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Create emitter once with source context for rich snippet rendering
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(path);

    // Run frontend pipeline: lex → parse → typecheck, reporting all errors
    let Some(frontend) = report_frontend_errors(&db, file, &mut emitter) else {
        std::process::exit(1);
    };
    let mut has_errors = frontend.has_errors();
    let parse_result = frontend.parse_result;
    let type_result = frontend.type_result;
    let pool = frontend.pool;

    // Check pattern exhaustiveness via canonicalization.
    // Skip if parse errors exist (AST may be malformed), but run even with
    // type errors — pattern problems are independent of type mismatches.
    // Store in CanonCache for session-scoped reuse by downstream consumers.
    if !parse_result.has_errors() {
        let shared_canon =
            oric::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);
        for problem in &shared_canon.problems {
            let diag = pattern_problem_to_diagnostic(problem, db.interner());
            emitter.emit(&diag);
            has_errors = true;
        }
    }

    if has_errors {
        emitter.flush();
    }

    // Check test coverage — severity controlled by enforcement level.
    // Spec: Clause 19.2 — configurable test enforcement (off/warn/error).
    let mut has_coverage_issues = false;
    if enforcement != TestEnforcement::Off {
        let interner = db.interner();
        let severity = match enforcement {
            TestEnforcement::Warn => Severity::Warning,
            TestEnforcement::Error => Severity::Error,
            TestEnforcement::Off => unreachable!(),
        };
        for problem in check_test_coverage(&parse_result.module, interner) {
            let diag = problem.into_diagnostic(interner).with_severity(severity);
            emitter.emit(&diag);
            has_coverage_issues = true;
            if enforcement == TestEnforcement::Error {
                has_errors = true;
            }
        }
    }

    // Exit if any errors occurred
    if has_errors {
        std::process::exit(1);
    }

    // Success message — varies by enforcement level
    let func_count = parse_result.module.functions.len();
    let test_count = parse_result.module.tests.len();
    match enforcement {
        TestEnforcement::Off => {
            println!("OK: {path} ({func_count} functions, {test_count} tests)");
        }
        TestEnforcement::Warn if has_coverage_issues => {
            let uncovered = func_count.saturating_sub(test_count);
            println!(
                "OK: {path} ({func_count} functions, {test_count} tests, {uncovered} uncovered)"
            );
        }
        TestEnforcement::Warn | TestEnforcement::Error => {
            println!("OK: {path} ({func_count} functions, {test_count} tests, 100% coverage)");
        }
    }
}
