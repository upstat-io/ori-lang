//! Command handlers for the Ori compiler CLI.
//!
//! Each submodule implements a specific CLI command (run, test, check, etc.).
//! Shared utilities like `read_file` and `report_frontend_errors` live here
//! in the module root.

use ori_diagnostic::emitter::DiagnosticEmitter;
use ori_diagnostic::queue::DiagnosticQueue;
use ori_diagnostic::Diagnostic;
use ori_diagnostic::Severity;
use ori_types::{Pool, TypeCheckResult, TypeCheckWarning, TypeCheckWarningKind};
use oric::parser::ParseOutput;
use oric::problem::lex::{render_lex_error, LexProblem};
use oric::problem::semantic::{check_test_coverage, pattern_problem_to_diagnostic};
use oric::query::{
    canonicalize_cached, lex_errors, parsed, tokens_with_metadata, typed, typed_pool,
};
use oric::reporting::typeck::TypeErrorRenderer;
use oric::{CompilerDb, Db, SourceFile};

pub mod build;
pub mod build_options;
mod check;
#[cfg(feature = "llvm")]
mod codegen_pipeline;
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
mod watch;

/// Test enforcement level — controls whether missing tests are errors, warnings, or ignored.
///
/// Configurable via `--test-enforcement=off|warn|error` CLI flag.
/// Default is `Off` (tests optional). See spec §19.2.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum TestEnforcement {
    /// No enforcement. Missing tests produce no diagnostic.
    #[default]
    Off,
    /// Missing tests produce a warning (E3010).
    Warn,
    /// Missing tests produce a compile-time error (E3010).
    Error,
}

impl TestEnforcement {
    /// Parse from a CLI flag value.
    ///
    /// Returns `None` for unrecognized values.
    pub fn parse_flag(s: &str) -> Option<Self> {
        match s {
            "off" => Some(TestEnforcement::Off),
            "warn" => Some(TestEnforcement::Warn),
            "error" => Some(TestEnforcement::Error),
            _ => None,
        }
    }
}

// Public types and functions for external use (tests, library consumers)
pub use build_options::{
    accumulate_build_options, parse_build_options, BuildOptions, DebugLevel, EmitType, LinkMode,
    LtoMode, OptLevel,
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
pub use watch::watch_file;

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

    // Phase dump: AST after parse (gated behind ORI_DUMP_AFTER_PARSE=1)
    crate::dbg_do!(crate::debug_flags::ORI_DUMP_AFTER_PARSE, {
        let path_str = file.path(db).display().to_string();
        crate::ast_dump::dump_ast(&parse_result, db.interner(), &path_str);
    });

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

    // Phase dump: Typed IR after type checking (gated behind ORI_DUMP_AFTER_TYPECK=1)
    crate::dbg_do!(crate::debug_flags::ORI_DUMP_AFTER_TYPECK, {
        let path_str = file.path(db).display().to_string();
        crate::ir_dump::dump_typed_ir(
            &parse_result,
            &type_result.typed,
            &pool,
            db.interner(),
            &path_str,
        );
    });

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

/// Result of the post-frontend check pipeline (pattern exhaustiveness + test coverage).
pub(super) struct CheckPipelineResult {
    /// Whether any hard errors occurred (frontend + pattern + coverage in error mode).
    pub has_errors: bool,
    /// Whether test coverage issues were found (relevant for success message).
    pub has_coverage_issues: bool,
    /// Number of user-defined functions (for success message).
    pub func_count: usize,
    /// Number of test functions (for success message).
    pub test_count: usize,
}

/// Run the post-frontend check pipeline: pattern exhaustiveness + test coverage.
///
/// Shared between `check_file` (exits on error) and watch mode's `run_check`
/// (returns on error). Callers decide how to handle errors via [`CheckPipelineResult`].
pub(super) fn run_post_frontend_checks(
    db: &CompilerDb,
    file: SourceFile,
    frontend: &FrontendResult,
    enforcement: TestEnforcement,
    emitter: &mut dyn DiagnosticEmitter,
) -> CheckPipelineResult {
    let mut has_errors = frontend.has_errors();

    // Check pattern exhaustiveness via canonicalization.
    // Skip if parse errors exist (AST may be malformed), but run even with
    // type errors — pattern problems are independent of type mismatches.
    if !frontend.parse_result.has_errors() {
        let shared_canon = canonicalize_cached(
            db,
            file,
            &frontend.parse_result,
            &frontend.type_result,
            &frontend.pool,
        );
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
        for problem in check_test_coverage(&frontend.parse_result.module, interner) {
            let diag = problem.into_diagnostic(interner).with_severity(severity);
            emitter.emit(&diag);
            has_coverage_issues = true;
            if enforcement == TestEnforcement::Error {
                has_errors = true;
            }
        }
    }

    if has_errors {
        emitter.flush();
    }

    CheckPipelineResult {
        has_errors,
        has_coverage_issues,
        func_count: frontend.parse_result.module.functions.len(),
        test_count: frontend.parse_result.module.tests.len(),
    }
}

/// Print the success message for a completed check.
///
/// Format varies by enforcement level:
/// - `Off`: function and test counts only
/// - `Warn` with issues: counts + number uncovered
/// - `Warn`/`Error` clean: counts + "100% coverage"
pub(super) fn print_check_success(
    path: &str,
    enforcement: TestEnforcement,
    result: &CheckPipelineResult,
) {
    let func_count = result.func_count;
    let test_count = result.test_count;
    match enforcement {
        TestEnforcement::Off => {
            println!("OK: {path} ({func_count} functions, {test_count} tests)");
        }
        TestEnforcement::Warn if result.has_coverage_issues => {
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
