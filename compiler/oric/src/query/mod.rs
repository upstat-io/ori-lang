//! Salsa Queries - Computed values that are cached
//!
//! Queries are functions that compute values from inputs or other queries.
//! Salsa automatically caches results and invalidates when dependencies change.

use crate::db::Db;
use crate::eval::{EvalOutput, Evaluator, ModuleEvalResult};
use crate::input::SourceFile;
use crate::ir::TokenList;
use crate::parser::{self, ParseOutput};
use crate::typeck;
use ori_types::TypeCheckResult;
use std::path::Path;

#[cfg(test)]
mod tests;

/// Parse a file by path, loading it through the Salsa input system.
///
/// This is the proper way to parse imported files - it creates a `SourceFile`
/// input if needed, ensuring that changes to the file are tracked by Salsa.
///
/// Returns None if the file cannot be read or has parse errors.
pub fn parsed_path(db: &dyn Db, path: &Path) -> Option<ParseOutput> {
    let file = db.load_file(path)?;
    let result = parsed(db, file);
    if result.has_errors() {
        None
    } else {
        Some(result)
    }
}

/// Tokenize a source file.
///
/// This is the first real compilation query. It converts source text
/// into a list of tokens that can be consumed by the parser.
///
/// # Caching Behavior
///
/// - First call: executes the lexer, caches result
/// - Subsequent calls (same input): returns cached `TokenList`
/// - After `file.set_text()`: re-lexes on next call
///
/// # Early Cutoff
///
/// Even if the source text changes, if the resulting tokens are
/// identical (same hash), downstream queries won't recompute.
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let text = file.text(db);
    ori_lexer::lex(text, db.interner())
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
    let toks = tokens(db, file);
    parser::parse(&toks, db.interner())
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
/// The Pool is created per-module and discarded after checking — only
/// the `TypeCheckResult` is cached by Salsa.
#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypeCheckResult {
    let parse_result = parsed(db, file);
    let file_path = file.path(db);
    typeck::type_check_with_imports(db, &parse_result, file_path)
}

/// Evaluate a source file.
///
/// This query evaluates the module's main function (if present) or
/// returns the result of evaluating all top-level expressions.
///
/// - Depends on `parsed` query
/// - Returns a Salsa-compatible `ModuleEvalResult`
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
    let parse_result = parsed(db, file);

    // Check for parse errors
    if parse_result.has_errors() {
        return ModuleEvalResult::failure("parse errors".to_string());
    }

    let interner = db.interner();
    let file_path = file.path(db);

    // Type check (returns result + pool)
    let (type_result, _pool) =
        typeck::type_check_with_imports_and_pool(db, &parse_result, file_path);

    // Check for type errors using the error guarantee
    if type_result.has_errors() {
        let error_count = type_result.errors().len();
        return ModuleEvalResult::failure(format!(
            "{error_count} type error{} found",
            if error_count == 1 { "" } else { "s" }
        ));
    }

    // Create evaluator with type information (Idx-based)
    let mut evaluator = Evaluator::builder(interner, &parse_result.arena, db)
        .expr_types(&type_result.typed.expr_types)
        .build();
    evaluator.register_prelude();

    if let Err(e) = evaluator.load_module(&parse_result, file_path) {
        return ModuleEvalResult::failure(format!("module error: {e}"));
    }

    // Look for a main function
    let main_name = interner.intern("main");
    if let Some(main_func) = evaluator.env().lookup(main_name) {
        // Call main with no arguments
        match evaluator.eval_call_value(&main_func, &[]) {
            Ok(value) => ModuleEvalResult::success(EvalOutput::from_value(&value, interner)),
            Err(e) => ModuleEvalResult::failure(e.message),
        }
    } else {
        // No main function - try to evaluate first function only if it has no parameters
        if let Some(func) = parse_result.module.functions.first() {
            let params = parse_result.arena.get_params(func.params);
            if params.is_empty() {
                // Zero-argument function - safe to call
                match evaluator.eval(func.body) {
                    Ok(value) => {
                        ModuleEvalResult::success(EvalOutput::from_value(&value, interner))
                    }
                    Err(e) => ModuleEvalResult::failure(e.message),
                }
            } else {
                // Function requires arguments - can't run without @main
                // Type checking passed, return void result
                ModuleEvalResult::success(EvalOutput::Void)
            }
        } else {
            // Empty module
            ModuleEvalResult::default()
        }
    }
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
