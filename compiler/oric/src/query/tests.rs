//! Tests for Salsa queries.

use super::*;
use crate::CompilerDb;
use salsa::Setter;
use ori_ir::TypeId;
use std::path::PathBuf;

#[test]
fn test_line_count() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "line1\nline2\nline3".to_string(),
    );

    assert_eq!(line_count(&db, file), 3);
}

#[test]
fn test_non_empty_line_count() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
        "@main () -> int = 42".to_string(),
    );

    assert_eq!(first_line(&db, file), "@main () -> int = 42");
}

#[test]
fn test_incremental_recomputation() {
    let mut db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/a.ori"),
        "one\ntwo".to_string(),
    );

    let file2 = SourceFile::new(
        &db,
        PathBuf::from("/b.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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

#[test]
fn test_tokens_basic() {
    use crate::ir::TokenKind;

    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
        "map(over: items, transform: fn)".to_string(),
    );

    let toks = tokens(&db, file);

    // map is now an identifier (library function), not a keyword
    // map ( over : items , transform : fn )
    assert!(matches!(toks[0].kind, TokenKind::Ident(_)));
    assert!(matches!(toks[1].kind, TokenKind::LParen));
    assert!(matches!(toks[2].kind, TokenKind::Ident(_)));
}

#[test]
fn test_parsed_basic() {
    use crate::ir::ExprKind;

    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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

#[test]
fn test_typed_basic() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42".to_string(),
    );

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert_eq!(result.function_types.len(), 1);
    assert_eq!(result.function_types[0].return_type, TypeId::INT);
}

#[test]
fn test_typed_caching() {
    let db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
    let mut db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42".to_string(),
    );

    // Initial type check
    let result1 = typed(&db, file);
    assert_eq!(result1.function_types[0].return_type, TypeId::INT);

    // Modify source to return bool
    file.set_text(&mut db).to("@main () -> bool = true".to_string());

    // Should re-type-check with new return type
    let result2 = typed(&db, file);
    assert_eq!(result2.function_types[0].return_type, TypeId::BOOL);
}

#[test]
fn test_typed_with_error() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = if 42 then 1 else 2".to_string(),
    );

    let result = typed(&db, file);

    // Should have type error: condition must be bool
    assert!(result.has_errors());
}

#[test]
fn test_evaluated_basic() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
        PathBuf::from("/test.ori"),
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
fn test_evaluated_recurse_pattern() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    // Test basic recurse pattern - simplest case: always return base
    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        r#"@main () -> int = recurse(
            condition: true,
            base: 42,
            step: self()
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
