//! The `check` command: type-check an Ori source file and verify test coverage.

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_diagnostic::queue::DiagnosticQueue;
use oric::problem::LexProblem;
use oric::query::{lex_errors, parsed};
use oric::reporting::typeck::TypeErrorRenderer;
use oric::typeck;
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

use super::read_file;

/// Type-check a file and verify that every function has test coverage.
///
/// Accumulates all errors (parse, type, and coverage) before exiting, giving
/// the user a complete picture of issues rather than stopping at the first error.
pub fn check_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let mut has_errors = false;

    // Create emitter once with source context for rich snippet rendering
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(path);

    // Report lexer errors first (unterminated strings, semicolons, confusables, etc.)
    let lex_errs = lex_errors(&db, file);
    if !lex_errs.is_empty() {
        for err in &lex_errs {
            let diag = LexProblem::Error(err.clone()).into_diagnostic(db.interner());
            emitter.emit(&diag);
        }
        has_errors = true;
    }

    // Check for parse errors — route through DiagnosticQueue for
    // deduplication and soft-error suppression after hard errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        let source = file.text(&db);
        let mut queue = DiagnosticQueue::new();
        for error in &parse_result.errors {
            queue.add_with_source_and_severity(
                error.to_diagnostic(),
                source.as_str(),
                error.severity,
            );
        }
        for diag in queue.flush() {
            emitter.emit(&diag);
        }
        has_errors = true;
    }

    // Check for type errors using direct call to get Pool for rich rendering
    let (type_result, pool) =
        typeck::type_check_with_imports_and_pool(&db, &parse_result, file.path(&db));
    if type_result.has_errors() {
        let renderer = TypeErrorRenderer::new(&pool, db.interner());

        for error in type_result.errors() {
            emitter.emit(&renderer.render(error));
        }
        has_errors = true;
    }

    if has_errors {
        emitter.flush();
    }

    // Check test coverage: every function (except @main) must have at least one test
    let interner = db.interner();
    let main_name = interner.intern("main");

    // Collect all tested function names
    let mut tested_functions: std::collections::HashSet<oric::Name> =
        std::collections::HashSet::new();
    for test in &parse_result.module.tests {
        for target in &test.targets {
            tested_functions.insert(*target);
        }
    }

    // Find functions without tests
    let mut untested: Vec<String> = Vec::new();
    for func in &parse_result.module.functions {
        if func.name != main_name && !tested_functions.contains(&func.name) {
            untested.push(interner.lookup(func.name).to_string());
        }
    }

    if !untested.is_empty() {
        eprintln!("Coverage error in '{path}':");
        eprintln!("  The following functions have no tests:");
        for name in &untested {
            eprintln!("    @{name}");
        }
        eprintln!();
        eprintln!("  Every function (except @main) requires at least one test.");
        eprintln!(
            "  Add tests using: @test_name tests @{} () -> void = ...",
            untested[0]
        );
        has_errors = true;
    }

    // Exit if any errors occurred
    if has_errors {
        std::process::exit(1);
    }

    let func_count = parse_result.module.functions.len();
    let test_count = parse_result.module.tests.len();
    println!("OK: {path} ({func_count} functions, {test_count} tests, 100% coverage)");
}
