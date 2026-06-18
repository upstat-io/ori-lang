//! Evaluation queries: module evaluation pipeline.

use super::{canonicalize_cached, lex_errors, parsed, typed, typed_pool};
use crate::db::Db;
use crate::eval::{EvalOutput, Evaluator, ModuleEvalResult};
use crate::input::SourceFile;
use crate::parser::ParseOutput;

/// Evaluate a source file.
///
/// This query evaluates the module's main function (if present) or
/// returns the result of evaluating all top-level expressions.
///
/// - Depends on `parsed` query
/// - Returns a Salsa-compatible `ModuleEvalResult`
///
/// # Error Layering (Success/Fail Gate)
///
/// This query converts pre-runtime phase errors (lex, parse, type) into opaque
/// failure strings (e.g., `"parse errors"`, `"3 type errors found"`). This is
/// intentional — `evaluated()` serves as a **success/fail gate**, not as the
/// primary error rendering path.
///
/// Consumers that need detailed error diagnostics (spans, suggestions, error codes)
/// should call `lex_errors()`, `parsed()`, and `typed()` separately for structured
/// error access. This is exactly what `report_frontend_errors()` does in the `check`
/// and `run` commands — they render errors with full diagnostic quality before ever
/// checking `evaluated()`.
///
/// Only runtime eval errors carry structured information in `ModuleEvalResult::eval_error`
/// (via `EvalErrorSnapshot`), since those cannot be obtained from earlier queries.
///
/// # Caching Behavior
///
/// - First call: evaluates the module, caches result
/// - Subsequent calls (same input): returns cached result
/// - After source changes: re-evaluates only if parsed result changed
///
/// # Intentional Impurity
///
/// This query is **intentionally impure** because evaluation may:
/// - Execute side effects (I/O, printing, etc.)
/// - Run tests that have observable behavior
/// - Interact with external systems via capabilities
///
/// Salsa caches the *first* evaluation result. For deterministic results,
/// ensure evaluated code is pure or uses capability injection for effects.
///
/// # Invalidation
///
/// This query invalidates when:
/// - Source file content changes (via `SourceFile.set_text()`)
/// - Parsed tokens change (triggers re-parse)
/// - Typed module changes (triggers re-typecheck)
///
/// The cached result persists until one of these conditions triggers
/// re-evaluation. For fresh evaluation, create a new `SourceFile` input.
#[salsa::tracked]
pub fn evaluated(db: &dyn Db, file: SourceFile) -> ModuleEvalResult {
    tracing::debug!(path = %file.path(db).display(), "evaluating");

    // Check for lexer errors first — the parser silently skips TokenKind::Error
    // tokens without emitting parse errors, so a file of pure lexer errors
    // (e.g., `"unterminated`) would pass parse_result.has_errors() and proceed
    // to evaluation with an empty module.
    let lex_errs = lex_errors(db, file);
    if !lex_errs.is_empty() {
        let error_count = lex_errs.len();
        let message = format!(
            "{error_count} lexer error{} found",
            if error_count == 1 { "" } else { "s" }
        );
        // Lex errors are pre-runtime failures — use `failure()` (no snapshot),
        // matching the pattern for parse errors and type errors below.
        // `eval_error` should only be populated for actual runtime eval errors.
        return ModuleEvalResult::failure(message);
    }

    let parse_result = parsed(db, file);

    // Check for parse errors
    if parse_result.has_errors() {
        return ModuleEvalResult::failure("parse errors".to_string());
    }

    // Type check via Salsa query (caches Pool as side effect).
    // This establishes a Salsa dependency: if typed() changes, evaluated()
    // is invalidated. The Pool is retrieved from the session-scoped cache.
    let type_result = typed(db, file);

    if type_result.has_errors() {
        let error_count = type_result.errors().len();
        return ModuleEvalResult::failure(format!(
            "{error_count} type error{} found",
            if error_count == 1 { "" } else { "s" }
        ));
    }

    let Some(pool) = typed_pool(db, file) else {
        return ModuleEvalResult::failure(
            "internal error: Pool not cached after type checking".to_string(),
        );
    };

    // Canonicalize and evaluate via shared helper
    let (result, _) = run_evaluation(
        db,
        file,
        &parse_result,
        &type_result,
        &pool,
        EvalRunMode::Normal,
    );
    result
}

/// How to run the evaluation pipeline.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) enum EvalRunMode {
    /// Normal evaluation without profiling.
    #[default]
    Normal,
    /// Evaluation with performance counters enabled.
    Profile,
}

/// Core evaluation pipeline: canonicalize → create evaluator → load → run.
///
/// Shared by [`evaluated()`] (Salsa query) and `eval_with_profile()` (direct call).
///
/// Returns the evaluation result and an optional counters report string.
pub(crate) fn run_evaluation(
    db: &dyn Db,
    file: SourceFile,
    parse_result: &ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    pool: &ori_types::Pool,
    mode: EvalRunMode,
) -> (ModuleEvalResult, Option<String>) {
    let interner = db.interner();
    let file_path = file.path(db);

    // Canonicalize: AST + types → self-contained canonical IR.
    // Uses session-scoped CanonCache for reuse across consumers.
    let shared_canon = canonicalize_cached(db, file, parse_result, type_result, pool);

    // Create evaluator with type information, canonical IR, and source info for Traceable
    let source_path = std::sync::Arc::new(file_path.to_string_lossy().to_string());
    let source_text = std::sync::Arc::new(file.text(db).clone());
    let mut evaluator = Evaluator::builder(interner, &parse_result.arena, db)
        .source_file_path(source_path)
        .source_text(source_text)
        .canon(shared_canon.clone())
        .build();
    evaluator.register_prelude();

    let enable_counters = matches!(mode, EvalRunMode::Profile);
    if enable_counters {
        evaluator.enable_counters();
    }

    if let Err(errors) = evaluator.load_module(parse_result, file_path, Some(&shared_canon)) {
        use std::fmt::Write;
        let mut msg = String::from("module error: ");
        for (i, e) in errors.iter().enumerate() {
            if i > 0 {
                msg.push_str("; ");
            }
            let _ = write!(msg, "{}", e.message);
        }
        return (ModuleEvalResult::failure(msg), None);
    }

    // Look for a main function
    let main_name = interner.intern("main");
    let result = if let Some(main_func) = evaluator.env().lookup(main_name) {
        // Call main with no arguments
        match evaluator.eval_call_value(&main_func, &[]) {
            Ok(value) => ModuleEvalResult::success(EvalOutput::from_value(&value, interner)),
            Err(e) => ModuleEvalResult::runtime_error(&e.into_eval_error()),
        }
    } else if let Some(func) = parse_result.module.functions.first() {
        // No main function - try to evaluate first function only if it has no parameters
        let params = parse_result.arena.get_params(func.params);
        if params.is_empty() {
            // Zero-argument function - safe to call.
            let Some(can_id) = shared_canon.root_for(func.name) else {
                return (
                    ModuleEvalResult::failure(
                        "internal error: function has no canonical root".to_string(),
                    ),
                    None,
                );
            };
            match evaluator.eval_can(can_id) {
                Ok(value) => ModuleEvalResult::success(EvalOutput::from_value(&value, interner)),
                Err(e) => ModuleEvalResult::runtime_error(&e.into_eval_error()),
            }
        } else {
            // Function requires arguments - can't run without @main
            ModuleEvalResult::success(EvalOutput::Void)
        }
    } else {
        // Empty module
        ModuleEvalResult::default()
    };

    let counters = if enable_counters {
        evaluator.counters_report()
    } else {
        None
    };

    (result, counters)
}
