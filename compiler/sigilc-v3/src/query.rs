//! Salsa Queries - Computed values that are cached
//!
//! Queries are functions that compute values from inputs or other queries.
//! Salsa automatically caches results and invalidates when dependencies change.

use crate::db::Db;
use crate::input::SourceFile;
use crate::ir::TokenList;
use crate::lexer;
use crate::parser::{self, ParseResult};
use crate::typeck::{self, TypedModule};
use crate::eval::{Evaluator, ModuleEvalResult, EvalOutput};

/// Tokenize a source file.
///
/// This is the first real compilation query. It converts source text
/// into a list of tokens that can be consumed by the parser.
///
/// # Caching Behavior
///
/// - First call: executes the lexer, caches result
/// - Subsequent calls (same input): returns cached TokenList
/// - After `file.set_text()`: re-lexes on next call
///
/// # Early Cutoff
///
/// Even if the source text changes, if the resulting tokens are
/// identical (same hash), downstream queries won't recompute.
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let text = file.text(db);
    lexer::lex(text, db.interner())
}

/// Parse a source file into a module.
///
/// This query demonstrates incremental parsing with early cutoff:
/// - Depends on `tokens` query (not source text directly)
/// - If tokens are unchanged (same hash), parsing is skipped
/// - ParseResult includes Module, ExprArena, and errors
///
/// # Early Cutoff
///
/// Even if source text changes (e.g., adding whitespace), if the
/// resulting tokens are identical, this query returns cached result.
#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult {
    let toks = tokens(db, file);
    parser::parse(&toks, db.interner())
}

/// Type check a source file.
///
/// This query performs type inference and checking on a parsed module.
/// - Depends on `parsed` query (not tokens directly)
/// - If parsed result is unchanged, type checking is skipped
/// - TypedModule includes inferred types and any type errors
///
/// # Caching Behavior
///
/// - First call: performs type checking, caches result
/// - Subsequent calls (same input): returns cached TypedModule
/// - After source changes: re-checks only if parsed result changed
#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypedModule {
    let parse_result = parsed(db, file);
    typeck::type_check(&parse_result, db.interner())
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
/// # Note
///
/// Evaluation results are deterministic for pure functions but may
/// differ for functions with side effects (I/O, randomness, etc.).
/// The cached result represents the first evaluation.
#[salsa::tracked]
pub fn evaluated(db: &dyn Db, file: SourceFile) -> ModuleEvalResult {
    let parse_result = parsed(db, file);

    // Check for parse errors
    if parse_result.has_errors() {
        return ModuleEvalResult::failure("parse errors".to_string());
    }

    let interner = db.interner();

    // Create evaluator
    let mut evaluator = Evaluator::new(interner, &parse_result.arena);
    evaluator.register_prelude();

    // Register all functions in the module
    for func in &parse_result.module.functions {
        let func_name = func.name;
        let params: Vec<_> = parse_result.arena.get_params(func.params)
            .iter()
            .map(|p| p.name)
            .collect();

        // Capture the current environment for closures
        let captures = evaluator.env().capture();

        let func_value = crate::eval::FunctionValue::with_captures(
            params,
            func.body,
            captures,
        );

        evaluator.env_mut().define(func_name, crate::eval::Value::Function(func_value), false);
    }

    // Look for a main function
    let main_name = interner.intern("main");
    if let Some(main_func) = evaluator.env().lookup(main_name) {
        // Call main with no arguments
        match evaluator.eval_call_value(main_func, vec![]) {
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
                    Ok(value) => ModuleEvalResult::success(EvalOutput::from_value(&value, interner)),
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
/// Depends on the same input as line_count, demonstrating
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompilerDb;
    use salsa::Setter;
    use std::path::PathBuf;

    #[test]
    fn test_line_count() {
        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "line1\nline2\nline3".to_string(),
        );

        assert_eq!(line_count(&db, file), 3);
    }

    #[test]
    fn test_non_empty_line_count() {
        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "line1\n\nline3\n".to_string(),
        );

        assert_eq!(line_count(&db, file), 3);  // "line1\n\nline3\n" = 3 lines
        assert_eq!(non_empty_line_count(&db, file), 2);
    }

    #[test]
    fn test_first_line() {
        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        assert_eq!(first_line(&db, file), "@main () -> int = 42");
    }

    #[test]
    fn test_incremental_recomputation() {
        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "line1\nline2".to_string(),
        );

        // Initial computation
        assert_eq!(line_count(&db, file), 2);

        // Cached - same result, no recomputation
        assert_eq!(line_count(&db, file), 2);

        // Mutate the input
        file.set_text(&mut db).to("line1\nline2\nline3".to_string());

        // Now it should recompute
        assert_eq!(line_count(&db, file), 3);
    }

    #[test]
    fn test_multiple_files() {
        let db = CompilerDb::new();

        let file1 = SourceFile::new(
            &db,
            PathBuf::from("/a.si"),
            "one\ntwo".to_string(),
        );

        let file2 = SourceFile::new(
            &db,
            PathBuf::from("/b.si"),
            "one\ntwo\nthree\nfour".to_string(),
        );

        assert_eq!(line_count(&db, file1), 2);
        assert_eq!(line_count(&db, file2), 4);
    }

    #[test]
    fn test_query_independence() {
        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "hello\n\nworld".to_string(),
        );

        // Both queries work
        assert_eq!(line_count(&db, file), 3);
        assert_eq!(non_empty_line_count(&db, file), 2);

        // Mutate
        file.set_text(&mut db).to("hello\nworld".to_string());

        // Both recompute correctly
        assert_eq!(line_count(&db, file), 2);
        assert_eq!(non_empty_line_count(&db, file), 2);
    }

    #[test]
    fn test_caching_verified_with_logs() {
        let db = CompilerDb::new();
        db.enable_logging();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "line1\nline2".to_string(),
        );

        // First call - should execute
        let _ = line_count(&db, file);
        let logs1 = db.take_logs();
        assert!(!logs1.is_empty(), "First call should execute query");

        // Second call - should be cached (no execution)
        let _ = line_count(&db, file);
        let logs2 = db.take_logs();
        assert!(logs2.is_empty(), "Second call should use cache");
    }

    // ===== TOKENS QUERY TESTS =====

    #[test]
    fn test_tokens_basic() {
        use crate::ir::TokenKind;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "let x = 42".to_string(),
        );

        let toks = tokens(&db, file);

        assert_eq!(toks.len(), 5); // let, x, =, 42, EOF
        assert!(matches!(toks[0].kind, TokenKind::Let));
        assert!(matches!(toks[1].kind, TokenKind::Ident(_)));
        assert!(matches!(toks[2].kind, TokenKind::Eq));
        assert!(matches!(toks[3].kind, TokenKind::Int(42)));
        assert!(matches!(toks[4].kind, TokenKind::Eof));
    }

    #[test]
    fn test_tokens_function_def() {
        use crate::ir::TokenKind;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        let toks = tokens(&db, file);

        assert!(matches!(toks[0].kind, TokenKind::At));
        assert!(matches!(toks[1].kind, TokenKind::Ident(_)));
        assert!(matches!(toks[2].kind, TokenKind::LParen));
        assert!(matches!(toks[3].kind, TokenKind::RParen));
        assert!(matches!(toks[4].kind, TokenKind::Arrow));
        assert!(matches!(toks[5].kind, TokenKind::IntType));
        assert!(matches!(toks[6].kind, TokenKind::Eq));
        assert!(matches!(toks[7].kind, TokenKind::Int(42)));
    }

    #[test]
    fn test_tokens_caching() {
        let db = CompilerDb::new();
        db.enable_logging();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "let x = 1".to_string(),
        );

        // First call - should execute
        let _ = tokens(&db, file);
        let logs1 = db.take_logs();
        assert!(!logs1.is_empty(), "First call should execute tokens query");

        // Second call - should be cached
        let _ = tokens(&db, file);
        let logs2 = db.take_logs();
        assert!(logs2.is_empty(), "Second call should use cache");
    }

    #[test]
    fn test_tokens_incremental() {
        use crate::ir::TokenKind;

        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "let x = 1".to_string(),
        );

        // Initial tokens
        let toks1 = tokens(&db, file);
        assert!(matches!(toks1[3].kind, TokenKind::Int(1)));

        // Modify the file
        file.set_text(&mut db).to("let x = 2".to_string());

        // Should get new tokens
        let toks2 = tokens(&db, file);
        assert!(matches!(toks2[3].kind, TokenKind::Int(2)));
    }

    #[test]
    fn test_tokens_with_strings() {
        use crate::ir::TokenKind;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"let s = "hello""#.to_string(),
        );

        let toks = tokens(&db, file);

        // Verify the string is correctly interned
        if let TokenKind::String(name) = toks[3].kind {
            assert_eq!(db.interner().lookup(name), "hello");
        } else {
            panic!("Expected String token");
        }
    }

    #[test]
    fn test_tokens_with_patterns() {
        use crate::ir::TokenKind;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "map(.over: items, .transform: fn)".to_string(),
        );

        let toks = tokens(&db, file);

        // map ( .over : items , .transform : fn )
        assert!(matches!(toks[0].kind, TokenKind::Map));
        assert!(matches!(toks[1].kind, TokenKind::LParen));
        assert!(matches!(toks[2].kind, TokenKind::Dot));
    }

    // ===== PARSED QUERY TESTS =====

    #[test]
    fn test_parsed_basic() {
        use crate::ir::ExprKind;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        let result = parsed(&db, file);

        assert!(!result.has_errors());
        assert_eq!(result.module.functions.len(), 1);

        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);
        assert!(matches!(body.kind, ExprKind::Int(42)));
    }

    #[test]
    fn test_parsed_caching() {
        let db = CompilerDb::new();
        db.enable_logging();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 1 + 2".to_string(),
        );

        // First call - should execute both tokens and parsed queries
        let _ = parsed(&db, file);
        let logs1 = db.take_logs();
        assert!(logs1.len() >= 2, "First call should execute queries");

        // Second call - should be fully cached
        let _ = parsed(&db, file);
        let logs2 = db.take_logs();
        assert!(logs2.is_empty(), "Second call should use cache");
    }

    #[test]
    fn test_parsed_incremental() {
        use crate::ir::ExprKind;

        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 1".to_string(),
        );

        // Initial parse
        let result1 = parsed(&db, file);
        assert!(matches!(
            result1.arena.get_expr(result1.module.functions[0].body).kind,
            ExprKind::Int(1)
        ));

        // Modify source
        file.set_text(&mut db).to("@main () -> int = 2".to_string());

        // Should re-parse with new value
        let result2 = parsed(&db, file);
        assert!(matches!(
            result2.arena.get_expr(result2.module.functions[0].body).kind,
            ExprKind::Int(2)
        ));
    }

    #[test]
    fn test_parsed_early_cutoff() {
        let mut db = CompilerDb::new();
        db.enable_logging();

        // Create file with some trailing whitespace
        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        // First call
        let result1 = parsed(&db, file);
        let _ = db.take_logs();

        // Add whitespace (tokens should be identical after lexing)
        // Note: This depends on lexer behavior with whitespace
        file.set_text(&mut db).to("@main () -> int = 42  ".to_string());

        // Get tokens to verify they're the same semantically
        // Even if tokens differ, parsed result should be equivalent
        let result2 = parsed(&db, file);

        // Results should be functionally equivalent
        assert_eq!(result1.module.functions.len(), result2.module.functions.len());
    }

    #[test]
    fn test_parsed_with_expressions() {
        use crate::ir::{ExprKind, BinaryOp};

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@calc () -> int = 1 + 2 * 3".to_string(),
        );

        let result = parsed(&db, file);
        assert!(!result.has_errors());

        // Verify precedence: should be Add(1, Mul(2, 3))
        let func = &result.module.functions[0];
        let body = result.arena.get_expr(func.body);

        if let ExprKind::Binary { op: BinaryOp::Add, left, right } = &body.kind {
            assert!(matches!(result.arena.get_expr(*left).kind, ExprKind::Int(1)));
            let right_expr = result.arena.get_expr(*right);
            if let ExprKind::Binary { op: BinaryOp::Mul, left: l2, right: r2 } = &right_expr.kind {
                assert!(matches!(result.arena.get_expr(*l2).kind, ExprKind::Int(2)));
                assert!(matches!(result.arena.get_expr(*r2).kind, ExprKind::Int(3)));
            } else {
                panic!("Expected multiplication");
            }
        } else {
            panic!("Expected addition");
        }
    }

    // ===== TYPED QUERY TESTS =====

    #[test]
    fn test_typed_basic() {
        use crate::types::Type;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        let result = typed(&db, file);

        assert!(!result.has_errors());
        assert_eq!(result.function_types.len(), 1);
        assert_eq!(result.function_types[0].return_type, Type::Int);
    }

    #[test]
    fn test_typed_caching() {
        let db = CompilerDb::new();
        db.enable_logging();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 1 + 2".to_string(),
        );

        // First call - should execute tokens, parsed, and typed queries
        let _ = typed(&db, file);
        let logs1 = db.take_logs();
        assert!(logs1.len() >= 3, "First call should execute queries");

        // Second call - should be fully cached
        let _ = typed(&db, file);
        let logs2 = db.take_logs();
        assert!(logs2.is_empty(), "Second call should use cache");
    }

    #[test]
    fn test_typed_incremental() {
        use crate::types::Type;

        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        // Initial type check
        let result1 = typed(&db, file);
        assert_eq!(result1.function_types[0].return_type, Type::Int);

        // Modify source to return bool
        file.set_text(&mut db).to("@main () -> bool = true".to_string());

        // Should re-type-check with new return type
        let result2 = typed(&db, file);
        assert_eq!(result2.function_types[0].return_type, Type::Bool);
    }

    #[test]
    fn test_typed_with_error() {
        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = if 42 then 1 else 2".to_string(),
        );

        let result = typed(&db, file);

        // Should have type error: condition must be bool
        assert!(result.has_errors());
    }

    // ===== EVALUATED QUERY TESTS =====

    #[test]
    fn test_evaluated_basic() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::Int(42)));
    }

    #[test]
    fn test_evaluated_arithmetic() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 1 + 2 * 3".to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::Int(7)));
    }

    #[test]
    fn test_evaluated_boolean() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> bool = true && false".to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::Bool(false)));
    }

    #[test]
    fn test_evaluated_if_expression() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = if true then 1 else 2".to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::Int(1)));
    }

    #[test]
    fn test_evaluated_list() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        // Note: Using simple return type since parser doesn't yet support [int]
        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () = [1, 2, 3]".to_string(),
        );

        let result = evaluated(&db, file);

        if result.is_failure() {
            panic!("Evaluation failed: {:?}", result.error);
        }

        assert_eq!(
            result.result,
            Some(EvalOutput::List(vec![
                EvalOutput::Int(1),
                EvalOutput::Int(2),
                EvalOutput::Int(3),
            ]))
        );
    }

    #[test]
    fn test_evaluated_caching() {
        let db = CompilerDb::new();
        db.enable_logging();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 42".to_string(),
        );

        // First call - should execute
        let _ = evaluated(&db, file);
        let logs1 = db.take_logs();
        assert!(!logs1.is_empty(), "First call should execute queries");

        // Second call - should be cached
        let _ = evaluated(&db, file);
        let logs2 = db.take_logs();
        assert!(logs2.is_empty(), "Second call should use cache");
    }

    #[test]
    fn test_evaluated_incremental() {
        use crate::eval::EvalOutput;

        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int = 1".to_string(),
        );

        // Initial evaluation
        let result1 = evaluated(&db, file);
        assert_eq!(result1.result, Some(EvalOutput::Int(1)));

        // Modify source
        file.set_text(&mut db).to("@main () -> int = 2".to_string());

        // Should re-evaluate with new value
        let result2 = evaluated(&db, file);
        assert_eq!(result2.result, Some(EvalOutput::Int(2)));
    }

    #[test]
    fn test_evaluated_parse_error() {
        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@main () -> int =".to_string(), // Missing expression
        );

        let result = evaluated(&db, file);

        assert!(result.is_failure());
        assert_eq!(result.error, Some("parse errors".to_string()));
    }

    #[test]
    fn test_evaluated_no_main() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            "@foo () -> int = 100".to_string(),
        );

        let result = evaluated(&db, file);

        // Should evaluate first function's body
        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::Int(100)));
    }

    #[test]
    fn test_evaluated_run_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        // Multi-line version with proper formatting
        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () -> int = run(
                let x: int = 1,
                let y: int = 2,
                x + y
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        if result.is_failure() {
            eprintln!("Error: {:?}", result.error);
        }
        assert!(result.is_success(), "Expected success, got error: {:?}", result.error);
        assert_eq!(result.result, Some(EvalOutput::Int(3)));
    }

    #[test]
    fn test_evaluated_map_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () = map(
                .over: [1, 2, 3],
                .transform: (x: int) -> x * 2
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        if result.is_failure() {
            eprintln!("Error: {:?}", result.error);
        }
        assert!(result.is_success(), "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.result,
            Some(EvalOutput::List(vec![
                EvalOutput::Int(2),
                EvalOutput::Int(4),
                EvalOutput::Int(6),
            ]))
        );
    }

    #[test]
    fn test_evaluated_filter_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () = filter(
                .over: [1, 2, 3, 4, 5],
                .predicate: (x: int) -> x > 2
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(
            result.result,
            Some(EvalOutput::List(vec![
                EvalOutput::Int(3),
                EvalOutput::Int(4),
                EvalOutput::Int(5),
            ]))
        );
    }

    #[test]
    fn test_evaluated_fold_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () -> int = fold(
                .over: [1, 2, 3, 4],
                .init: 0,
                .op: (acc: int, x: int) -> acc + x
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::Int(10)));
    }

    #[test]
    fn test_evaluated_find_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () = find(
                .over: [1, 2, 3, 4, 5],
                .where: (x: int) -> x > 3
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(
            result.result,
            Some(EvalOutput::Some(Box::new(EvalOutput::Int(4))))
        );
    }

    #[test]
    fn test_evaluated_find_pattern_not_found() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () = find(
                .over: [1, 2, 3],
                .where: (x: int) -> x > 10
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        assert!(result.is_success());
        assert_eq!(result.result, Some(EvalOutput::None));
    }

    #[test]
    fn test_evaluated_collect_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () = collect(
                .range: 1..4,
                .transform: (x: int) -> x * x
            )"#.to_string(),
        );

        let result = evaluated(&db, file);

        if !result.is_success() {
            eprintln!("Error: {:?}", result.error);
        }
        assert!(result.is_success(), "Expected success, got error: {:?}", result.error);
        assert_eq!(
            result.result,
            Some(EvalOutput::List(vec![
                EvalOutput::Int(1),
                EvalOutput::Int(4),
                EvalOutput::Int(9),
            ]))
        );
    }

    #[test]
    fn test_evaluated_recurse_pattern() {
        use crate::eval::EvalOutput;

        let db = CompilerDb::new();

        // Test basic recurse pattern - simplest case: always return base
        let file = SourceFile::new(
            &db,
            PathBuf::from("/test.si"),
            r#"@main () -> int = recurse(
                .cond: true,
                .base: 42,
                .step: self()
            )"#.to_string(),
        );

        // Debug: print parse errors
        let parsed = parsed(&db, file);
        if !parsed.errors.is_empty() {
            for err in &parsed.errors {
                eprintln!("Parse error: {:?}", err);
            }
        }

        let result = evaluated(&db, file);

        if !result.is_success() {
            eprintln!("Error: {:?}", result.error);
        }
        assert!(result.is_success(), "Expected success, got error: {:?}", result.error);
        assert_eq!(result.result, Some(EvalOutput::Int(42)));
    }
}
