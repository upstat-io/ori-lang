//! Shared frontend diagnostics and post-check processing.

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

use super::provenance;

/// Test enforcement level — controls whether missing tests are errors, warnings, or ignored.
///
/// Configurable via `--test-enforcement=off|warn|error`; the default is `Off`.
/// Spec: Clause 19.2.
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
    /// Returns the enforcement level named by `s`, or `None` for an unsupported value.
    pub fn parse_flag(s: &str) -> Option<Self> {
        match s {
            "off" => Some(TestEnforcement::Off),
            "warn" => Some(TestEnforcement::Warn),
            "error" => Some(TestEnforcement::Error),
            _ => None,
        }
    }
}

/// Result of running the frontend pipeline (lex → parse → typecheck).
#[derive(Debug)]
pub(crate) struct FrontendResult {
    pub(crate) parse_result: ParseOutput,
    pub(crate) type_result: TypeCheckResult,
    pub(crate) pool: std::sync::Arc<Pool>,
    /// Count kept separately from the parse and type-check results.
    lex_error_count: usize,
}

impl FrontendResult {
    /// Reports whether lexing, parsing, or type checking produced an error.
    pub(super) fn has_errors(&self) -> bool {
        self.lex_error_count > 0 || self.parse_result.has_errors() || self.type_result.has_errors()
    }
}

/// Emit every Canon-owned module-constant failure through the stable E2058
/// renderer. Returns whether the caller must stop before a consumer phase.
pub(crate) fn emit_const_eval_problems(
    problems: &[ori_ir::canon::ConstEvalProblem],
    interner: &oric::ir::StringInterner,
    emitter: &mut dyn DiagnosticEmitter,
) -> bool {
    let diagnostics =
        oric::problem::semantic::const_eval_problems_to_diagnostics(problems, interner);
    if diagnostics.is_empty() {
        return false;
    }
    emitter.emit_all(&diagnostics);
    true
}

/// Emits all frontend diagnostics and returns the cached pipeline state.
///
/// Returns `None` only when type checking fails to cache its [`Pool`].
pub(crate) fn report_frontend_errors(
    db: &CompilerDb,
    file: SourceFile,
    emitter: &mut dyn DiagnosticEmitter,
) -> Option<FrontendResult> {
    let lex_errs = lex_errors(db, file);
    let lex_error_count = lex_errs.len();
    for err in &lex_errs {
        let diag = render_lex_error(err);
        emitter.emit(&diag);
    }

    let lex_output = tokens_with_metadata(db, file);
    for warning in &lex_output.warnings {
        let diag = LexProblem::DetachedDocComment {
            span: warning.span,
            marker: warning.marker,
        }
        .into_diagnostic();
        emitter.emit(&diag);
    }

    let parse_result = parsed(db, file);

    crate::dump_orchestrator::dump_parse(
        &parse_result,
        db.interner(),
        &file.path(db).display().to_string(),
    );

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

    for warning in &parse_result.warnings {
        emitter.emit(&warning.to_diagnostic());
    }

    let type_result = typed(db, file);
    let Some(pool) = typed_pool(db, file) else {
        let diag = ori_diagnostic::Diagnostic::error(ori_diagnostic::ErrorCode::E9001)
            .with_message("Pool not cached after type checking");
        emitter.emit(&diag);
        emitter.flush();
        return None;
    };

    crate::dump_orchestrator::dump_typeck(
        &parse_result,
        &type_result.typed,
        &pool,
        db.interner(),
        &file.path(db).display().to_string(),
    );

    provenance::emit_provenance_trace(&pool, &type_result.typed.mono_instances, db.interner());

    if type_result.has_errors() {
        let renderer = TypeErrorRenderer::new(&pool, db.interner());
        for error in type_result.errors() {
            emitter.emit(&renderer.render(error));
        }
    }

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
#[derive(Debug)]
pub(crate) struct CheckPipelineResult {
    /// Whether any hard errors occurred (frontend + pattern + coverage in error mode).
    pub has_errors: bool,
    /// Whether test coverage issues were found (relevant for success message).
    pub has_coverage_issues: bool,
    /// Number of user-defined functions (for success message).
    pub func_count: usize,
    /// Number of test functions (for success message).
    pub test_count: usize,
}

/// Applies exhaustiveness and test-coverage checks to a completed frontend result.
///
/// Shared between `check_file` (exits on error) and watch mode's `run_check`
/// (returns on error). Callers decide how to handle errors via [`CheckPipelineResult`].
pub(crate) fn run_post_frontend_checks(
    db: &CompilerDb,
    file: SourceFile,
    frontend: &FrontendResult,
    enforcement: TestEnforcement,
    emitter: &mut dyn DiagnosticEmitter,
) -> CheckPipelineResult {
    let mut has_errors = frontend.has_errors();

    // Why: A malformed AST invalidates canonicalization; type mismatches do not.
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
        if emit_const_eval_problems(&shared_canon.const_problems, db.interner(), emitter) {
            has_errors = true;
        }
    }

    if has_errors {
        emitter.flush();
    }

    // Spec: Clause 19.2.
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
pub(crate) fn print_check_success(
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
pub(crate) fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("{}", read_file_error_message(path, &e));
            std::process::exit(1);
        }
    }
}

fn read_file_error_message(path: &str, e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => format!(
            "cannot find source file '{path}'. Check the path and try again, or run 'ori help' \
             for command usage."
        ),
        std::io::ErrorKind::PermissionDenied => format!("permission denied reading '{path}'"),
        std::io::ErrorKind::InvalidData => format!("'{path}' contains invalid UTF-8 data"),
        _ => format!("error reading '{path}': {e}"),
    }
}

#[cfg(test)]
mod tests;
