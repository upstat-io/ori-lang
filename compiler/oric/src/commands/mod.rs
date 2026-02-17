//! Command handlers for the Ori compiler CLI.
//!
//! Each submodule implements a specific CLI command (run, test, check, etc.).
//! Shared utilities like `read_file` and `report_frontend_errors` live here
//! in the module root.

use ori_diagnostic::emitter::DiagnosticEmitter;
use ori_diagnostic::queue::DiagnosticQueue;
use ori_diagnostic::Diagnostic;
use ori_types::{Pool, TypeCheckResult, TypeCheckWarning, TypeCheckWarningKind};
use oric::parser::ParseOutput;
use oric::problem::lex::{render_lex_error, LexProblem};
use oric::query::{lex_errors, parsed, tokens_with_metadata, typed, typed_pool};
use oric::reporting::typeck::TypeErrorRenderer;
use oric::{CompilerDb, Db, SourceFile};

pub mod build;
mod check;
#[cfg(feature = "llvm")]
mod compile_common;
mod debug;
mod demangle;
mod explain;
mod fmt;
mod run;
mod target;
mod targets;
mod test;

// Public types and functions for external use (tests, library consumers)
pub use build::{
    parse_build_options, BuildOptions, DebugLevel, EmitType, LinkMode, LtoMode, OptLevel,
};

// Internal re-exports for use by the CLI binary via oric::commands::*
// These use paths like `oric::commands::build_file` from main.rs
pub use build::build_file;
pub use check::check_file;
pub use debug::{lex_file, parse_file};
pub use demangle::demangle_symbol;
pub use explain::explain_error;
pub use fmt::run_format;
pub use run::{run_file, run_file_compiled};
pub use target::{add_target, list_installed_targets, remove_target, TargetSubcommand};
pub use targets::{list_targets, TargetFilter};
pub use test::run_tests;

/// Result of running the frontend pipeline (lex → parse → typecheck).
pub(super) struct FrontendResult {
    pub parse_result: ParseOutput,
    pub type_result: TypeCheckResult,
    pub pool: std::sync::Arc<Pool>,
    /// Number of lex errors found (not tracked by parse/type results).
    lex_error_count: usize,
}

impl FrontendResult {
    /// Whether any phase produced errors.
    ///
    /// Checks all three sources: lex errors (counted separately since they're
    /// not part of `ParseOutput`), parse errors, and type errors.
    pub fn has_errors(&self) -> bool {
        self.lex_error_count > 0 || self.parse_result.has_errors() || self.type_result.has_errors()
    }
}

/// Run the frontend pipeline and report all errors to the emitter.
///
/// Checks lex errors, parse errors, and type errors, emitting diagnostics for
/// each. Returns `None` only if the Pool fails to cache (internal error).
/// Otherwise returns `FrontendResult` with all pipeline outputs. Use
/// `FrontendResult::has_errors()` to check whether any phase produced errors.
///
/// This is the single source of truth for frontend error reporting — used by
/// `check_file`, `run_file`, and `check_source` (LLVM path).
pub(super) fn report_frontend_errors(
    db: &CompilerDb,
    file: SourceFile,
    emitter: &mut dyn DiagnosticEmitter,
) -> Option<FrontendResult> {
    // Report lexer errors first (unterminated strings, semicolons, confusables, etc.)
    let lex_errs = lex_errors(db, file);
    let lex_error_count = lex_errs.len();
    for err in &lex_errs {
        let diag = render_lex_error(err);
        emitter.emit(&diag);
    }

    // Emit lex warnings (detached doc comments detected at the token level).
    // Uses `tokens_with_metadata()` which preserves the full `LexOutput` including warnings.
    let lex_output = tokens_with_metadata(db, file);
    for warning in &lex_output.warnings {
        let diag = LexProblem::DetachedDocComment {
            span: warning.span,
            marker: warning.marker,
        }
        .into_diagnostic();
        emitter.emit(&diag);
    }

    // Check for parse errors — route through DiagnosticQueue for
    // deduplication and soft-error suppression after hard errors
    let parse_result = parsed(db, file);
    if parse_result.has_errors() {
        let source = file.text(db);
        let mut queue = DiagnosticQueue::new();
        for error in &parse_result.errors {
            let (diag, severity) = error.to_queued_diagnostic();
            queue.add_with_source_and_severity(diag, source.as_str(), severity);
        }
        for diag in queue.flush() {
            emitter.emit(&diag);
        }
    }

    // Emit parse warnings (detached doc comments detected at the syntax level).
    for warning in &parse_result.warnings {
        emitter.emit(&warning.to_diagnostic());
    }

    // Type check via Salsa query — caches Pool for reuse downstream.
    let type_result = typed(db, file);
    let Some(pool) = typed_pool(db, file) else {
        let diag = ori_diagnostic::Diagnostic::error(ori_diagnostic::ErrorCode::E9001)
            .with_message("Pool not cached after type checking");
        emitter.emit(&diag);
        emitter.flush();
        return None;
    };

    if type_result.has_errors() {
        let renderer = TypeErrorRenderer::new(&pool, db.interner());
        for error in type_result.errors() {
            emitter.emit(&renderer.render(error));
        }
    }

    // Emit type checker warnings (e.g., infinite iterator consumption)
    for warning in &type_result.typed.warnings {
        emitter.emit(&render_type_warning(warning));
    }

    Some(FrontendResult {
        parse_result,
        type_result,
        pool,
        lex_error_count,
    })
}

/// Render a type checker warning into a `Diagnostic`.
#[cold]
fn render_type_warning(warning: &TypeCheckWarning) -> Diagnostic {
    match &warning.kind {
        TypeCheckWarningKind::InfiniteIteratorConsumed { consumer, source } => {
            Diagnostic::warning(warning.code())
                .with_message(format!(
                    "`.{consumer}()` on an infinite iterator will never terminate"
                ))
                .with_label(
                    warning.span,
                    format!("this iterator is infinite (from `{source}`)"),
                )
                .with_suggestion(format!(
                    "add `.take(n)` before `.{consumer}()` to bound the iteration"
                ))
        }
    }
}

/// Read a file from disk, exiting with a user-friendly error message on failure.
pub(super) fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            let msg = match e.kind() {
                std::io::ErrorKind::NotFound => format!("cannot find file '{path}'"),
                std::io::ErrorKind::PermissionDenied => {
                    format!("permission denied reading '{path}'")
                }
                std::io::ErrorKind::InvalidData => {
                    format!("'{path}' contains invalid UTF-8 data")
                }
                _ => format!("error reading '{path}': {e}"),
            };
            eprintln!("{msg}");
            std::process::exit(1);
        }
    }
}
