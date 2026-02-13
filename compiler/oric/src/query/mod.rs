//! Salsa Queries — Computed values that are cached and incrementally revalidated.
//!
//! Queries are functions that compute values from inputs or other queries.
//! Salsa automatically caches results and invalidates when dependencies change.
//!
//! # Query Pipeline & Early Cutoff
//!
//! ```text
//! SourceFile (#[salsa::input])
//!     ↓ file.text(db)
//! lex_result(db, file)     — tokens + errors; early cutoff on LexResult equality
//!     ↓
//! tokens(db, file)         — position-independent equality enables cutoff
//!     ↓                      even when spans shift (whitespace edits)
//! parsed(db, file)         — early cutoff on AST equality
//!     ↓
//! typed(db, file)          — early cutoff on TypeCheckResult equality
//!     ↓                      (also caches Pool in session-scoped PoolCache)
//!     ├── [codegen boundary — NOT a Salsa query]
//!     │   ↓
//!     │   ARC analysis → LLVM emission → object file (ArtifactCache)
//!     │
//!     └── evaluated(db, file) — depends on typed() for TypeCheckResult + Pool
//!         ↓
//!         canonicalize → evaluate
//! ```
//!
//! Codegen is not a Salsa query because LLVM types are lifetime-bound to an
//! LLVM `Context` and cannot satisfy `Clone + Eq + Hash`. The Salsa/ArtifactCache
//! boundary is at `typed()`: function content hashes are computed from the
//! `TypeCheckResult` and used as cache keys for ARC IR and object artifacts.
//! See `commands/compile_common.rs` for the back-end caching strategy.
//!
//! # Side-Cache Invariant
//!
//! Three session-scoped caches (`PoolCache`, `CanonCache`, `ImportsCache`) live
//! **outside** Salsa's dependency graph because their values can't satisfy
//! `Clone + Eq + Hash`. The `typed()` query calls [`invalidate_file_caches()`]
//! before re-type-checking to clear stale entries. Any future code path that
//! triggers re-type-checking MUST also call `invalidate_file_caches()` — failing
//! to do so will cause silent correctness bugs from stale cache reads.

use crate::db::Db;
use crate::eval::{EvalOutput, Evaluator, ModuleEvalResult};
use crate::input::SourceFile;
use crate::ir::TokenList;
use crate::parser::{self, ParseOutput};
use crate::typeck;
use ori_ir::canon::SharedCanonResult;
use ori_types::TypeCheckResult;

#[cfg(test)]
mod tests;

/// Lex a source file into tokens and errors.
///
/// Derives from [`tokens_with_metadata()`], projecting out just the tokens
/// and errors. This ensures the lexer runs **at most once** per file version,
/// even when both `lex_result()` and `tokens_with_metadata()` are queried
/// (which happens in every `report_frontend_errors()` call).
///
/// # Caching Behavior
///
/// - First call: triggers `tokens_with_metadata()`, projects result
/// - Subsequent calls (same input): returns cached `LexResult`
/// - After `file.set_text()`: re-derives on next call
///
/// # Early Cutoff
///
/// Even if the source text changes, if the resulting `LexResult` is
/// identical (same hash), downstream queries won't recompute.
#[salsa::tracked]
pub fn lex_result(db: &dyn Db, file: SourceFile) -> ori_lexer::LexResult {
    let output = tokens_with_metadata(db, file);
    ori_lexer::LexResult {
        tokens: output.tokens,
        errors: output.errors,
    }
}

/// Tokenize a source file.
///
/// This is the first real compilation query. It converts source text
/// into a list of tokens that can be consumed by the parser.
///
/// # Caching Behavior
///
/// Derives from [`lex_result()`] — the lexer only runs once per file version.
/// If the tokens are unchanged (same hash), downstream queries won't recompute.
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    lex_result(db, file).tokens
}

/// Get lexer errors for a source file.
///
/// Returns the accumulated errors from lexing (unterminated strings, semicolons,
/// `===`, unicode confusables, etc.). These are surfaced in `check` and `run`
/// commands before parse errors.
///
/// Derives from [`lex_result()`] — the lexer only runs once per file version.
#[salsa::tracked]
pub fn lex_errors(db: &dyn Db, file: SourceFile) -> Vec<ori_lexer::lex_error::LexError> {
    lex_result(db, file).errors
}

/// Tokenize a source file with full metadata (comments, blank lines, errors).
///
/// Unlike `tokens()` which returns only the `TokenList` for parsing,
/// this query preserves the complete `LexOutput` including comments,
/// blank line positions, and lex errors. This is used by the formatter
/// and IDE features that need comment information.
///
/// # Caching Behavior
///
/// Uses position-independent `TokenList` hashing so whitespace-only edits
/// that shift token positions (but don't change token kinds) still enable
/// early cutoff for downstream queries that depend only on `tokens()`.
#[salsa::tracked]
pub fn tokens_with_metadata(db: &dyn Db, file: SourceFile) -> ori_lexer::LexOutput {
    tracing::debug!(path = %file.path(db).display(), "lexing with metadata");
    let text = file.text(db);
    ori_lexer::lex_with_comments(text, db.interner())
}

/// Parse a source file into a module.
///
/// This query demonstrates incremental parsing with early cutoff:
/// - Depends on `tokens` query (not source text directly)
/// - If tokens are unchanged (same hash), parsing is skipped
/// - `ParseOutput` includes Module, `ExprArena`, and errors
///
/// # Early Cutoff
///
/// Even if source text changes (e.g., adding whitespace), if the
/// resulting tokens are identical, this query returns cached result.
#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseOutput {
    tracing::debug!(path = %file.path(db).display(), "parsing");
    let toks = tokens(db, file);
    parser::parse(&toks, db.interner())
}

/// Proof that side-caches have been invalidated or are not applicable.
///
/// `type_check_with_imports_and_pool()` requires this token to prevent callers
/// from accidentally skipping `invalidate_file_caches()`. The token can only
/// be created by:
/// - [`invalidate_file_caches()`] — for Salsa-tracked files
/// - [`CacheGuard::untracked()`] — for synthetic/imported modules not in Salsa
pub(crate) struct CacheGuard(());

impl CacheGuard {
    /// For modules not tracked by Salsa (no cache entries to invalidate).
    ///
    /// These modules are type-checked directly without going through `typed()`,
    /// so they have no entries in `PoolCache`/`CanonCache`/`ImportsCache`.
    pub(crate) fn untracked() -> Self {
        CacheGuard(())
    }
}

/// Invalidate all session-scoped side-caches for a file.
///
/// Must be called whenever a file is (re-)type-checked, since these caches
/// are not tracked by Salsa's automatic dependency system.
///
/// Returns a [`CacheGuard`] token proving invalidation was performed.
fn invalidate_file_caches(db: &dyn Db, path: &std::path::Path) -> CacheGuard {
    db.pool_cache().invalidate(path);
    db.canon_cache().invalidate(path);
    db.imports_cache().invalidate(path);
    CacheGuard(())
}

/// Type check a source file.
///
/// This query performs type inference and checking on a parsed module
/// using the Pool-based type representation with unified interning.
///
/// - Depends on `parsed` query (not tokens directly)
/// - If parsed result is unchanged, type checking is skipped
/// - `TypeCheckResult` includes inferred types (`Idx`) and any type errors
///
/// # Caching Behavior
///
/// - First call: performs type checking, caches result
/// - Subsequent calls (same input): returns cached `TypeCheckResult`
/// - After source changes: re-checks only if parsed result changed
///
/// # Import Resolution
///
/// Resolves imports before type checking, making imported functions
/// available to the type checker.
///
/// # Pool Side-Cache
///
/// The Pool can't satisfy Salsa's `Clone + Eq + Hash` requirements, so it's
/// stored in a session-scoped [`PoolCache`](crate::db::PoolCache) as a side
/// effect. Callers that need the Pool (error rendering, canonicalization,
/// codegen) should call `typed()` first, then [`typed_pool()`].
#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypeCheckResult {
    tracing::debug!(path = %file.path(db).display(), "type checking");
    let parse_result = parsed(db, file);
    let file_path = file.path(db);

    // Invalidate side-caches before re-type-checking. The returned CacheGuard
    // proves to type_check_with_imports_and_pool() that invalidation was performed.
    let guard = invalidate_file_caches(db, file_path);

    let (result, pool) =
        typeck::type_check_with_imports_and_pool(db, &parse_result, file_path, guard);

    // Cache the Pool for callers that need it alongside the TypeCheckResult.
    db.pool_cache().store(file_path, pool);

    result
}

/// Retrieve the Pool cached during the most recent `typed()` call for this file.
///
/// The Pool is stored as a side-channel during `typed()` execution because it
/// can't satisfy Salsa's `Clone + Eq + Hash` requirements. Callers that need
/// both the `TypeCheckResult` and Pool (for error rendering, canonicalization,
/// or codegen) should call `typed()` first, then `typed_pool()`.
///
/// Returns `None` if `typed()` hasn't been called for this file yet.
pub fn typed_pool(db: &dyn Db, file: SourceFile) -> Option<std::sync::Arc<ori_types::Pool>> {
    db.pool_cache().get(file.path(db))
}

/// Type-check a module using Salsa queries when a `SourceFile` is available,
/// falling back to direct type checking otherwise.
///
/// This consolidates the "resolve `TypeCheckResult` + Pool" logic shared by
/// `Evaluator::canonicalize_module()` and the test runner's LLVM path. Both
/// need to type-check imported modules and obtain a Pool for canonicalization.
///
/// Returns `None` only if the Salsa-based `typed()` succeeds but `typed_pool()`
/// fails to return the Pool (an internal error — the Pool is cached as a side
/// effect of `typed()`). The direct type-check fallback always succeeds.
pub(crate) fn type_check_module(
    db: &dyn Db,
    parse_output: &ParseOutput,
    module_path: &std::path::Path,
    source_file: Option<SourceFile>,
) -> Option<(TypeCheckResult, std::sync::Arc<ori_types::Pool>)> {
    if let Some(sf) = source_file {
        let tc = typed(db, sf);
        let Some(pool) = typed_pool(db, sf) else {
            tracing::warn!(
                module = %module_path.display(),
                "Pool not cached after typed() — skipping module"
            );
            return None;
        };
        Some((tc, pool))
    } else {
        // No SourceFile → imported module not tracked by Salsa. No cache
        // entries exist to invalidate, so CacheGuard::untracked() is correct.
        let guard = CacheGuard::untracked();
        let (tc, pool) =
            crate::typeck::type_check_with_imports_and_pool(db, parse_output, module_path, guard);
        Some((tc, std::sync::Arc::new(pool)))
    }
}

/// Canonicalize a module with session-scoped caching, keyed by `SourceFile`.
///
/// Thin wrapper around [`canonicalize_cached_by_path`] that derives the cache
/// key from `file.path(db)`. This is the primary entry point for callers that
/// have a `SourceFile` (Salsa queries, commands, test runner).
pub(crate) fn canonicalize_cached(
    db: &dyn Db,
    file: SourceFile,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &ori_types::Pool,
) -> SharedCanonResult {
    canonicalize_cached_by_path(db, file.path(db), parse_result, type_result, pool)
}

/// Canonicalize a module with session-scoped caching, keyed by path.
///
/// Performs `AST + types → canonical IR` via `ori_canon::lower_module()`, caching
/// the result in `CanonCache`. This is the single source of truth for the
/// cache-check → canonicalize → store pattern used by:
/// - `canonicalize_cached()` (SourceFile-keyed convenience wrapper)
/// - `Evaluator::canonicalize_module()` (imported module canonicalization)
/// - `check_file`, `run_evaluation`, `check_source` (LLVM), and the test runner
///
/// Always canonicalizes regardless of type errors — callers that need to skip
/// on type errors (like `Evaluator::canonicalize_module`) should check before calling.
pub(crate) fn canonicalize_cached_by_path(
    db: &dyn Db,
    path: &std::path::Path,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &ori_types::Pool,
) -> SharedCanonResult {
    if let Some(cached) = db.canon_cache().get(path) {
        return cached;
    }
    let canon = ori_ir::canon::SharedCanonResult::new(ori_canon::lower_module(
        &parse_result.module,
        &parse_result.arena,
        type_result,
        pool,
        db.interner(),
    ));
    db.canon_cache().store(path, canon.clone());
    canon
}

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

    // Create evaluator with type information and canonical IR
    let mut evaluator = Evaluator::builder(interner, &parse_result.arena, db)
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

/// Count the number of lines in a source file.
///
/// This is a trivial query to verify Salsa is working.
///
/// # Caching Behavior
///
/// - First call: executes the function, caches result
/// - Subsequent calls (same input): returns cached result
/// - After `file.set_text()`: re-executes on next call
#[salsa::tracked]
pub fn line_count(db: &dyn Db, file: SourceFile) -> usize {
    let text = file.text(db);
    text.lines().count()
}

/// Count the number of non-empty lines.
///
/// Depends on the same input as `line_count`, demonstrating
/// that multiple queries can depend on the same input.
#[salsa::tracked]
pub fn non_empty_line_count(db: &dyn Db, file: SourceFile) -> usize {
    let text = file.text(db);
    text.lines().filter(|line| !line.trim().is_empty()).count()
}

/// Get the first line of a file.
///
/// Returns an owned String because Salsa query results must be Clone + Eq.
#[salsa::tracked]
pub fn first_line(db: &dyn Db, file: SourceFile) -> String {
    let text = file.text(db);
    text.lines().next().unwrap_or("").to_string()
}
