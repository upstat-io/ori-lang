//! The `check` command: type-check an Ori source file and verify test coverage.

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use oric::problem::semantic::pattern_problem_to_diagnostic;
use oric::problem::SemanticProblem;
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

use super::read_file;
use super::report_frontend_errors;

/// Type-check a file and verify that every function has test coverage.
///
/// Accumulates all errors (parse, type, and coverage) before exiting, giving
/// the user a complete picture of issues rather than stopping at the first error.
pub fn check_file(path: &str) {
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

    // Check test coverage: every function (except @main) must have at least one test.
    // Emit structured diagnostics with source spans pointing to untested functions.
    let interner = db.interner();
    let main_name = interner.intern("main");

    let mut tested_functions: std::collections::HashSet<oric::Name> =
        std::collections::HashSet::new();
    for test in &parse_result.module.tests {
        for target in &test.targets {
            tested_functions.insert(*target);
        }
    }

    for func in &parse_result.module.functions {
        if func.name != main_name && !tested_functions.contains(&func.name) {
            let func_name = interner.lookup(func.name).to_string();
            let diag = SemanticProblem::MissingTest {
                span: func.span,
                func_name,
            }
            .into_diagnostic(interner);
            emitter.emit(&diag);
            has_errors = true;
        }
    }

    // Exit if any errors occurred
    if has_errors {
        std::process::exit(1);
    }

    let func_count = parse_result.module.functions.len();
    let test_count = parse_result.module.tests.len();
    println!("OK: {path} ({func_count} functions, {test_count} tests, 100% coverage)");
}
