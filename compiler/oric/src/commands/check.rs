//! The `check` command: type-check an Ori source file and optionally verify test coverage.

use ori_diagnostic::emitter::{ColorMode, TerminalEmitter};
use oric::{CompilerDb, SourceFile};
use std::path::PathBuf;

use super::print_check_success;
use super::read_file;
use super::report_frontend_errors;
use super::run_post_frontend_checks;
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

    let result = run_post_frontend_checks(&db, file, &frontend, enforcement, &mut emitter);

    if result.has_errors {
        std::process::exit(1);
    }

    print_check_success(path, enforcement, &result);
}
