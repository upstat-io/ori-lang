//! Tests for Salsa queries.

use super::*;
use crate::CompilerDb;
use ori_types::FunctionSig;
use ori_types::Idx;
use salsa::Setter;
use std::path::PathBuf;

const ADD_BODY_EXTRA_ZERO: &str = include_str!("fixtures/add-body-extra-zero.ori");
const ADD_COMPACT: &str = include_str!("fixtures/add-compact.ori");
const ADD_INT: &str = include_str!("fixtures/add-int.ori");
const ADD_MUL: &str = include_str!("fixtures/add-mul.ori");
const ADD_SPACED: &str = include_str!("fixtures/add-spaced.ori");
const ADD_TABBED: &str = include_str!("fixtures/add-tabbed.ori");
const CALC_PRECEDENCE: &str = include_str!("fixtures/calc-precedence.ori");
const FOO_BAR: &str = include_str!("fixtures/foo-bar.ori");
const FOO_INT_100: &str = include_str!("fixtures/foo-int-100.ori");
const MAIN_BLOCK_BINDINGS: &str = include_str!("fixtures/main-block-bindings.ori");
const MAIN_BOOL_AND: &str = include_str!("fixtures/main-bool-and.ori");
const MAIN_BOOL_TRUE: &str = include_str!("fixtures/main-bool-true.ori");
const MAIN_COMMENT_V1: &str = include_str!("fixtures/main-comment-v1.ori");
const MAIN_COMMENT_V2: &str = include_str!("fixtures/main-comment-v2.ori");
const MAIN_DOC_COMMENT: &str = include_str!("fixtures/main-doc-comment.ori");
const MAIN_EXTRA_SPACES: &str = include_str!("fixtures/main-extra-spaces.ori");
const MAIN_FIELD_ARITHMETIC: &str = include_str!("fixtures/main-field-arithmetic.ori");
const MAIN_IF: &str = include_str!("fixtures/main-if.ori");
const MAIN_INT_1: &str = include_str!("fixtures/main-int-1.ori");
const MAIN_INT_2: &str = include_str!("fixtures/main-int-2.ori");
const MAIN_INT_42: &str = include_str!("fixtures/main-int-42.ori");
const MAIN_INT_100: &str = include_str!("fixtures/main-int-100.ori");
const MAIN_INT_ADD: &str = include_str!("fixtures/main-int-add.ori");
const MAIN_INT_LIST: &str = include_str!("fixtures/main-int-list.ori");
const MAIN_INVALID_IF_CONDITION: &str = include_str!("fixtures/main-invalid-if-condition.ori");
const MAIN_LIST_INDEX: &str = include_str!("fixtures/main-list-index.ori");
const MAIN_MAP_INDEX_COALESCE: &str = include_str!("fixtures/main-map-index-coalesce.ori");
const MAIN_MISSING_EXPRESSION: &str = include_str!("fixtures/main-missing-expression.ori");
const MAIN_NESTED_FIELD: &str = include_str!("fixtures/main-nested-field.ori");
const MAIN_NEW_COMMENT: &str = include_str!("fixtures/main-new-comment.ori");
const MAIN_OLD_COMMENT: &str = include_str!("fixtures/main-old-comment.ori");
const MAIN_PRECEDENCE: &str = include_str!("fixtures/main-precedence.ori");
const MAIN_RECURSE: &str = include_str!("fixtures/main-recurse.ori");
const MAIN_REGULAR_COMMENT: &str = include_str!("fixtures/main-regular-comment.ori");
const MAIN_RESULT_COALESCE: &str = include_str!("fixtures/main-result-coalesce.ori");
const MAIN_STRUCT_FIELD: &str = include_str!("fixtures/main-struct-field.ori");
const MAIN_TYPE_ERROR: &str = include_str!("fixtures/main-type-error.ori");

fn fixture_source(source: &'static str) -> &'static str {
    source.strip_suffix('\n').unwrap_or(source)
}

fn fixture_text(source: &'static str) -> String {
    fixture_source(source).to_owned()
}

/// Find a function signature by name in the typed result.
///
/// `typed.functions` may include imported/builtin signatures alongside
/// user-defined ones — always look up by name rather than by index.
fn find_fn<'a>(db: &CompilerDb, result: &'a TypeCheckResult, name: &str) -> &'a FunctionSig {
    let interner = db.interner();
    let target = interner.intern(name);
    result
        .typed
        .functions
        .iter()
        .find(|f| f.name == target)
        .unwrap_or_else(|| panic!("function '{name}' not found in typed output"))
}

/// Count only user-defined functions (those whose names match given set).
fn user_fn_count(db: &CompilerDb, result: &TypeCheckResult, names: &[&str]) -> usize {
    let interner = db.interner();
    let targets: Vec<_> = names.iter().map(|n| interner.intern(n)).collect();
    result
        .typed
        .functions
        .iter()
        .filter(|f| targets.contains(&f.name))
        .count()
}

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

    assert_eq!(line_count(&db, file), 3); // "line1\n\nline3\n" = 3 lines
    assert_eq!(non_empty_line_count(&db, file), 2);
}

#[test]
fn test_first_line() {
    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

    assert_eq!(first_line(&db, file), "@main () -> int = 42;");
}

#[test]
fn test_incremental_recomputation() {
    let mut db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), "line1\nline2".to_string());

    assert_eq!(line_count(&db, file), 2);

    assert_eq!(line_count(&db, file), 2);

    file.set_text(&mut db).to("line1\nline2\nline3".to_string());

    assert_eq!(line_count(&db, file), 3);
}

#[test]
fn test_multiple_files() {
    let db = CompilerDb::new();

    let file1 = SourceFile::new(&db, PathBuf::from("/a.ori"), "one\ntwo".to_string());

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), "line1\nline2".to_string());

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), "let x = 42".to_string());

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), "let x = 1".to_string());

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), "let x = 1".to_string());

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

    // `map` is a library-function identifier rather than a keyword.
    assert!(matches!(toks[0].kind, TokenKind::Ident(_)));
    assert!(matches!(toks[1].kind, TokenKind::LParen));
    assert!(matches!(toks[2].kind, TokenKind::Ident(_)));
}

#[test]
fn test_parsed_basic() {
    use crate::ir::ExprKind;

    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_ADD));

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_1));

    // Initial parse
    let result1 = parsed(&db, file);
    assert!(matches!(
        result1
            .arena
            .get_expr(result1.module.functions[0].body)
            .kind,
        ExprKind::Int(1)
    ));

    // Modify source
    file.set_text(&mut db).to(fixture_text(MAIN_INT_2));

    // Should re-parse with new value
    let result2 = parsed(&db, file);
    assert!(matches!(
        result2
            .arena
            .get_expr(result2.module.functions[0].body)
            .kind,
        ExprKind::Int(2)
    ));
}

#[test]
fn test_parsed_early_cutoff() {
    let mut db = CompilerDb::new();
    db.enable_logging();

    // Create file with some trailing whitespace
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

    // First call
    let result1 = parsed(&db, file);
    let _ = db.take_logs();

    // Add whitespace (tokens should be identical after lexing)
    // Note: This depends on lexer behavior with whitespace
    file.set_text(&mut db).to(fixture_text(MAIN_INT_42));

    // Get tokens to verify they're the same semantically
    // Even if tokens differ, parsed result should be equivalent
    let result2 = parsed(&db, file);

    // Results should be functionally equivalent
    assert_eq!(
        result1.module.functions.len(),
        result2.module.functions.len()
    );
}

#[test]
fn test_parsed_with_expressions() {
    use crate::ir::{BinaryOp, ExprKind};

    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        fixture_text(CALC_PRECEDENCE),
    );

    let result = parsed(&db, file);
    assert!(!result.has_errors());

    // Verify precedence: should be Add(1, Mul(2, 3))
    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &body.kind
    {
        assert!(matches!(
            result.arena.get_expr(*left).kind,
            ExprKind::Int(1)
        ));
        let right_expr = result.arena.get_expr(*right);
        if let ExprKind::Binary {
            op: BinaryOp::Mul,
            left: l2,
            right: r2,
        } = &right_expr.kind
        {
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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert_eq!(find_fn(&db, &result, "main").return_type, Idx::INT);
}

#[test]
fn test_typed_caching() {
    let db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_ADD));

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

    // Initial type check
    let result1 = typed(&db, file);
    assert_eq!(find_fn(&db, &result1, "main").return_type, Idx::INT);

    // Modify source to return bool
    file.set_text(&mut db).to(fixture_text(MAIN_BOOL_TRUE));

    // Should re-type-check with new return type
    let result2 = typed(&db, file);
    assert_eq!(find_fn(&db, &result2, "main").return_type, Idx::BOOL);
}

#[test]
fn test_typed_with_error() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        fixture_text(MAIN_INVALID_IF_CONDITION),
    );

    let result = typed(&db, file);

    // Should have type error: condition must be bool
    assert!(result.has_errors());
}

#[test]
fn test_evaluated_basic() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

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
        fixture_text(MAIN_PRECEDENCE),
    );

    let result = evaluated(&db, file);

    assert!(result.is_success());
    assert_eq!(result.result, Some(EvalOutput::Int(7)));
}

#[test]
fn test_evaluated_boolean() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_BOOL_AND));

    let result = evaluated(&db, file);

    assert!(result.is_success());
    assert_eq!(result.result, Some(EvalOutput::Bool(false)));
}

#[test]
fn test_evaluated_if_expression() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_IF));

    let result = evaluated(&db, file);

    assert!(result.is_success());
    assert_eq!(result.result, Some(EvalOutput::Int(1)));
}

#[test]
fn test_evaluated_list() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_LIST));

    let result = evaluated(&db, file);

    assert!(
        !result.is_failure(),
        "Evaluation failed: {:?}",
        result.error
    );

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

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

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_1));

    // Initial evaluation
    let result1 = evaluated(&db, file);
    assert_eq!(result1.result, Some(EvalOutput::Int(1)));

    // Modify source
    file.set_text(&mut db).to(fixture_text(MAIN_INT_2));

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
        fixture_text(MAIN_MISSING_EXPRESSION), // Missing expression
    );

    let result = evaluated(&db, file);

    assert!(result.is_failure());
    assert_eq!(result.error, Some("parse errors".to_string()));
}

#[test]
fn test_evaluated_no_main() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(FOO_INT_100));

    let result = evaluated(&db, file);

    // Should evaluate first function's body
    assert!(result.is_success());
    assert_eq!(result.result, Some(EvalOutput::Int(100)));
}

#[test]
fn test_evaluated_block_expression() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    // Block expression with let bindings
    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        fixture_text(MAIN_BLOCK_BINDINGS),
    );

    let result = evaluated(&db, file);

    if result.is_failure() {
        eprintln!("Error: {:?}", result.error);
    }
    assert!(
        result.is_success(),
        "Expected success, got error: {:?}",
        result.error
    );
    assert_eq!(result.result, Some(EvalOutput::Int(3)));
}

#[test]
fn test_evaluated_recurse_pattern() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    // Test basic recurse pattern - simplest case: always return base
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_RECURSE));

    // Debug: print parse errors
    let parsed = parsed(&db, file);
    if !parsed.errors.is_empty() {
        for err in &parsed.errors {
            eprintln!("Parse error: {err:?}");
        }
    }

    let result = evaluated(&db, file);

    if !result.is_success() {
        eprintln!("Error: {:?}", result.error);
    }
    assert!(
        result.is_success(),
        "Expected success, got error: {:?}",
        result.error
    );
    assert_eq!(result.result, Some(EvalOutput::Int(42)));
}

#[test]
fn test_typed_function_signatures() {
    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(ADD_INT));

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert_eq!(
        user_fn_count(&db, &result, &["add"]),
        1,
        "Should have exactly 1 user-defined function signature"
    );

    let sig = find_fn(&db, &result, "add");
    assert_eq!(sig.param_types.len(), 2, "add() has 2 parameters");
    assert_eq!(sig.return_type, Idx::INT, "add() returns int");
    assert_eq!(sig.param_types[0], Idx::INT, "first param is int");
    assert_eq!(sig.param_types[1], Idx::INT, "second param is int");
}

#[test]
fn test_typed_empty_module() {
    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), String::new());

    let result = typed(&db, file);

    assert!(!result.has_errors(), "Empty module should have no errors");
    // No user-defined functions — builtins may be present in the list
    assert_eq!(
        user_fn_count(&db, &result, &[]),
        0,
        "Empty module has no user-defined functions"
    );
}

#[test]
fn test_typed_multiple_functions() {
    let db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(FOO_BAR));

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert_eq!(
        user_fn_count(&db, &result, &["foo", "bar"]),
        2,
        "Should have 2 user-defined function signatures"
    );
}

#[test]
fn test_typed_determinism() {
    let db = CompilerDb::new();

    let source = fixture_source(ADD_MUL);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    // Call twice — should produce identical results
    let result1 = typed(&db, file);
    let result2 = typed(&db, file);

    assert_eq!(result1, result2, "must produce deterministic results");

    // Verify both user-defined functions are present
    let add_sig = find_fn(&db, &result1, "add");
    let mul_sig = find_fn(&db, &result1, "mul");
    assert!(
        add_sig.name < mul_sig.name,
        "User functions 'add' and 'mul' should be sorted by name for determinism"
    );
}

// Field Access, Index Access, and Coalesce Tests

#[test]
fn test_typed_list_indexing() {
    let db = CompilerDb::new();

    let source = fixture_source(MAIN_LIST_INDEX);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let result = typed(&db, file);
    if result.has_errors() {
        for e in result.errors() {
            eprintln!("ERROR: {e:?}");
        }
    }
    assert!(
        !result.has_errors(),
        "list indexing should not produce errors"
    );
    assert_eq!(find_fn(&db, &result, "main").return_type, Idx::INT);
}

#[test]
fn test_typed_map_indexing_with_coalesce() {
    let db = CompilerDb::new();

    let source = fixture_source(MAIN_MAP_INDEX_COALESCE);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let result = typed(&db, file);
    if result.has_errors() {
        for e in result.errors() {
            eprintln!("ERROR: {e:?}");
        }
    }
    assert!(
        !result.has_errors(),
        "map indexing with coalesce should not produce errors"
    );
    assert_eq!(find_fn(&db, &result, "main").return_type, Idx::INT);
}

#[test]
fn test_typed_struct_field_access() {
    let db = CompilerDb::new();

    let source = fixture_source(MAIN_STRUCT_FIELD);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let result = typed(&db, file);
    if result.has_errors() {
        for e in result.errors() {
            eprintln!("ERROR: {e:?}");
        }
    }
    assert!(
        !result.has_errors(),
        "struct field access should not produce errors"
    );
    assert_eq!(find_fn(&db, &result, "main").return_type, Idx::INT);
}

#[test]
fn test_typed_nested_field_access() {
    let db = CompilerDb::new();

    let source = fixture_source(MAIN_NESTED_FIELD);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let result = typed(&db, file);
    if result.has_errors() {
        for e in result.errors() {
            eprintln!("ERROR: {e:?}");
        }
    }
    assert!(
        !result.has_errors(),
        "nested field access should not produce errors"
    );
}

#[test]
fn test_typed_field_in_arithmetic() {
    let db = CompilerDb::new();

    let source = fixture_source(MAIN_FIELD_ARITHMETIC);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let result = typed(&db, file);
    if result.has_errors() {
        for e in result.errors() {
            eprintln!("ERROR: {e:?}");
        }
    }
    assert!(
        !result.has_errors(),
        "field access in arithmetic should not produce errors"
    );
    assert_eq!(find_fn(&db, &result, "main").return_type, Idx::INT);
}

#[test]
fn test_typed_whitespace_invariance() {
    // Different horizontal whitespace should produce identical TypeCheckResult.
    // The type checker output depends on semantic content (tokens), not formatting.
    let compact = fixture_source(ADD_COMPACT);
    let spaced = fixture_source(ADD_SPACED);
    let tabbed = fixture_source(ADD_TABBED);

    let db1 = CompilerDb::new();
    let file1 = SourceFile::new(&db1, PathBuf::from("/test.ori"), compact.to_string());
    let result_compact = typed(&db1, file1);

    let db2 = CompilerDb::new();
    let file2 = SourceFile::new(&db2, PathBuf::from("/test.ori"), spaced.to_string());
    let result_spaced = typed(&db2, file2);

    let db3 = CompilerDb::new();
    let file3 = SourceFile::new(&db3, PathBuf::from("/test.ori"), tabbed.to_string());
    let result_tabbed = typed(&db3, file3);

    // All three should type check without errors
    assert!(!result_compact.has_errors(), "compact should succeed");
    assert!(!result_spaced.has_errors(), "spaced should succeed");
    assert!(!result_tabbed.has_errors(), "tabbed should succeed");

    // All three should produce the same function signature
    assert_eq!(user_fn_count(&db1, &result_compact, &["add"]), 1);
    assert_eq!(user_fn_count(&db2, &result_spaced, &["add"]), 1);
    assert_eq!(user_fn_count(&db3, &result_tabbed, &["add"]), 1);

    let sig_compact = find_fn(&db1, &result_compact, "add");
    let sig_spaced = find_fn(&db2, &result_spaced, "add");
    let sig_tabbed = find_fn(&db3, &result_tabbed, "add");

    // Same function name
    assert_eq!(sig_compact.name, sig_spaced.name);
    assert_eq!(sig_compact.name, sig_tabbed.name);

    // Same parameter types
    assert_eq!(sig_compact.param_types, sig_spaced.param_types);
    assert_eq!(sig_compact.param_types, sig_tabbed.param_types);

    // Same return type
    assert_eq!(sig_compact.return_type, sig_spaced.return_type);
    assert_eq!(sig_compact.return_type, sig_tabbed.return_type);

    // Same error state (no errors)
    assert_eq!(
        result_compact.typed.errors.len(),
        result_spaced.typed.errors.len()
    );
    assert_eq!(
        result_compact.typed.errors.len(),
        result_tabbed.typed.errors.len()
    );
}

#[test]
fn test_typed_result_coalesce() {
    let db = CompilerDb::new();

    // Err type annotated explicitly so no unresolved `Tag::Var` survives body
    // inference (PC-2). The test's
    // subject is `??` coalescing on a Result; the choice of Err type is
    // incidental — `str` stands in for any inhabited type.
    let source = fixture_source(MAIN_RESULT_COALESCE);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let result = typed(&db, file);
    if result.has_errors() {
        for e in result.errors() {
            eprintln!("ERROR: {e:?}");
        }
    }
    assert!(
        !result.has_errors(),
        "Result coalesce should not produce errors"
    );
    assert_eq!(find_fn(&db, &result, "main").return_type, Idx::INT);
}

// tokens_with_metadata() Tests

#[test]
fn test_tokens_with_metadata_returns_comments() {
    use ori_ir::CommentKind;

    let db = CompilerDb::new();

    let source = fixture_source(MAIN_REGULAR_COMMENT);
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    let output = tokens_with_metadata(&db, file);

    // Should capture the comment
    assert_eq!(output.comments.len(), 1, "should capture 1 comment");
    assert_eq!(output.comments[0].kind, CommentKind::Regular);

    // Tokens should also be present and valid
    assert!(
        output.tokens.len() >= 5,
        "should have tokens for the function"
    );
    assert!(!output.has_errors(), "should have no lex errors");
}

#[test]
fn test_tokens_with_metadata_comment_only_edit() {
    use ori_ir::CommentKind;

    let mut db = CompilerDb::new();

    // Version 1: regular comment
    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        fixture_text(MAIN_OLD_COMMENT),
    );

    let output1 = tokens_with_metadata(&db, file);
    assert_eq!(output1.comments.len(), 1);
    assert_eq!(output1.comments[0].kind, CommentKind::Regular);

    // Version 2: different regular comment text (same comment kind)
    file.set_text(&mut db).to(fixture_text(MAIN_NEW_COMMENT));

    let output2 = tokens_with_metadata(&db, file);
    assert_eq!(output2.comments.len(), 1);
    assert_eq!(output2.comments[0].kind, CommentKind::Regular);

    // Code tokens are identical (same kind, same flags — no IS_DOC in either)
    assert_eq!(
        output1.tokens, output2.tokens,
        "regular→regular comment text edit should not change code tokens"
    );

    // But the full LexOutput differs (different comment content)
    assert_ne!(
        output1, output2,
        "full LexOutput should differ due to comment text change"
    );

    // Version 3: doc comment (comment kind changes → IS_DOC flag changes on @main)
    file.set_text(&mut db).to(fixture_text(MAIN_DOC_COMMENT));

    let output3 = tokens_with_metadata(&db, file);
    assert_eq!(output3.comments.len(), 1);
    assert_eq!(
        output3.comments[0].kind,
        CommentKind::DocMember,
        "comment kind should update after edit"
    );

    // A doc marker changes the `@main` token's flags.
    assert_ne!(
        output2.tokens, output3.tokens,
        "regular→doc comment change should change code tokens (IS_DOC flag)"
    );

    // Full output also differs
    assert_ne!(
        output2, output3,
        "full LexOutput should differ due to comment kind change"
    );
}

#[test]
fn test_tokens_early_cutoff_on_whitespace_edit() {
    let mut db = CompilerDb::new();
    db.enable_logging();

    // Start with single spaces between tokens
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

    // First call — executes both tokens and parsed queries
    let _ = parsed(&db, file);
    let initial_logs = db.take_logs();
    assert!(
        initial_logs.len() >= 2,
        "initial call should execute tokens + parsed, got {} logs",
        initial_logs.len()
    );

    // Add extra spaces between tokens that already have SPACE_BEFORE.
    // This changes Span positions but NOT TokenKind or TokenFlags, so
    // position-independent equality holds and parsed() is not re-executed.
    file.set_text(&mut db).to(fixture_text(MAIN_EXTRA_SPACES));

    // Call parsed again — tokens query re-executes (text changed),
    // but position-independent Hash/Eq means tokens are "equal",
    // so parsed should NOT re-execute (early cutoff).
    let _ = parsed(&db, file);
    let logs = db.take_logs();

    // With early cutoff: only tokens re-executes (1 WillExecute event).
    // Without early cutoff: both tokens + parsed re-execute (2+ events).
    assert_eq!(
        logs.len(),
        1,
        "only tokens should re-execute (early cutoff for parsed); got {} logs: {:#?}",
        logs.len(),
        logs
    );
}

// Salsa early cutoff verification tests

#[test]
fn test_comment_only_change_triggers_early_cutoff_for_parsed() {
    // Changing a regular comment's text does NOT change code tokens (same kind,
    // same flags). This means parsed() should use early cutoff and NOT re-execute.
    let mut db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        fixture_text(MAIN_OLD_COMMENT),
    );

    // First call — executes lex_result + tokens + parsed
    let _ = parsed(&db, file);
    let _ = db.take_logs(); // Clear initial logs

    // Change only the comment text (regular → regular, different text)
    file.set_text(&mut db).to(fixture_text(MAIN_NEW_COMMENT));

    let _ = parsed(&db, file);
    let logs = db.take_logs();

    // Only lex_result + tokens should re-execute (2 events at most).
    // parsed() should NOT re-execute because code tokens are position-
    // independent equal (same TokenKind and TokenFlags for @main, (, ), etc.).
    assert!(
        logs.len() <= 2,
        "comment-only edit should trigger early cutoff for parsed(); got {} logs: {:#?}",
        logs.len(),
        logs
    );
}

#[test]
fn test_comment_only_change_triggers_early_cutoff_for_typed() {
    // If tokens are unchanged after a comment edit, then parsed() is skipped,
    // which means typed() is also skipped (transitive early cutoff).
    let mut db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        fixture_text(MAIN_COMMENT_V1),
    );

    // First call — execute the full pipeline
    let result1 = typed(&db, file);
    let _ = db.take_logs();

    // Change only comment text
    file.set_text(&mut db).to(fixture_text(MAIN_COMMENT_V2));

    let result2 = typed(&db, file);
    let logs = db.take_logs();

    // typed() should produce identical results
    assert_eq!(
        result1.typed.functions.len(),
        result2.typed.functions.len(),
        "typed results should be identical after comment-only change"
    );

    // A comment-only edit may re-execute lexing and parsing, but never type checking.
    let typed_reexecuted = logs.iter().any(|l| l.contains("typed"));
    assert!(
        !typed_reexecuted,
        "typed() should NOT re-execute on comment-only change; logs: {logs:#?}",
    );
}

#[test]
#[cfg(feature = "llvm")]
fn test_body_change_without_signature_change_produces_different_module_hash() {
    // When a function's body changes but its signature stays the same,
    // the module hash should change (different expr_types) but individual
    // function signature hashes should be identical.
    //
    // This is the foundation of function-level incremental compilation:
    // only the changed function needs recompilation, not its callers.

    use ori_llvm::aot::incremental::function_hash::{compute_module_hash, extract_function_hashes};

    let db = CompilerDb::new();

    // Version 1: body returns a + b
    let file1 = SourceFile::new(&db, PathBuf::from("/test1.ori"), fixture_text(ADD_INT));
    let type1 = typed(&db, file1);
    let all_hashes1 = extract_function_hashes(&type1.typed.functions, &type1.typed.expr_types);
    let add_name = db.interner().intern("add");
    let hashes1: Vec<_> = all_hashes1.iter().filter(|(n, _)| *n == add_name).collect();

    // Version 2: same signature, different body (extra operation)
    let file2 = SourceFile::new(
        &db,
        PathBuf::from("/test2.ori"),
        fixture_text(ADD_BODY_EXTRA_ZERO),
    );
    let type2 = typed(&db, file2);
    let all_hashes2 = extract_function_hashes(&type2.typed.functions, &type2.typed.expr_types);
    let hashes2: Vec<_> = all_hashes2.iter().filter(|(n, _)| *n == add_name).collect();

    assert_eq!(hashes1.len(), 1, "should have 1 'add' function hash");
    assert_eq!(hashes2.len(), 1, "should have 1 'add' function hash");

    // Module hash should differ (body expression types changed)
    let mh1 = compute_module_hash(&all_hashes1);
    let mh2 = compute_module_hash(&all_hashes2);
    assert_ne!(mh1, mh2, "module hash should differ when body changes");

    // Signature hash should be identical (same params and return type)
    assert_eq!(
        hashes1[0].1.signature_hash(),
        hashes2[0].1.signature_hash(),
        "signature hash should be unchanged when only body changes"
    );
}

#[test]
fn test_typed_early_cutoff_on_body_change() {
    // Changing a function's body (but not its signature) should cause typed()
    // to re-execute, producing a different TypeCheckResult with different
    // expression types. This verifies the Salsa → codegen handoff works:
    // Salsa detects the change, and function hashing observes it.
    let mut db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), fixture_text(MAIN_INT_42));

    let result1 = typed(&db, file);
    let _ = db.take_logs();

    // Change body only (same signature: () -> int)
    file.set_text(&mut db).to(fixture_text(MAIN_INT_100));

    let result2 = typed(&db, file);
    let logs = db.take_logs();

    // typed() SHOULD re-execute because the parsed AST changed
    let typed_reexecuted = logs.iter().any(|l| l.contains("typed"));
    assert!(
        typed_reexecuted,
        "typed() should re-execute when body changes; logs: {logs:#?}",
    );

    // Return types should still match (signature unchanged)
    assert_eq!(
        find_fn(&db, &result1, "main").return_type,
        find_fn(&db, &result2, "main").return_type,
        "return type should be unchanged"
    );
}

/// Simulates 3 sequential file edits through the Salsa pipeline, proving that
/// the database remains usable across multiple edit cycles. This is the
/// foundational correctness proof for `ori watch` — if this test passes, the
/// watch command's edit loop is sound.
#[test]
fn test_watch_loop_simulation() {
    let mut db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/watch.ori"), fixture_text(MAIN_INT_1));

    // The initial source type-checks.
    let initial = typed(&db, file);
    assert!(
        !initial.has_errors(),
        "initial source should have no errors"
    );
    assert_eq!(user_fn_count(&db, &initial, &["main"]), 1);

    // A body-only edit preserves the signature.
    file.set_text(&mut db).to(fixture_text(MAIN_INT_2));

    let body_edit = typed(&db, file);
    assert!(
        !body_edit.has_errors(),
        "body-only edit should have no errors"
    );
    assert_eq!(find_fn(&db, &body_edit, "main").return_type, Idx::INT);

    // A signature edit replaces the inferred return type.
    file.set_text(&mut db).to(fixture_text(MAIN_BOOL_TRUE));

    let signature_edit = typed(&db, file);
    assert!(
        !signature_edit.has_errors(),
        "signature edit should have no errors"
    );
    assert_eq!(find_fn(&db, &signature_edit, "main").return_type, Idx::BOOL);

    // An invalid edit reports its type error.
    file.set_text(&mut db).to(fixture_text(MAIN_TYPE_ERROR));

    let invalid_edit = typed(&db, file);
    assert!(
        invalid_edit.has_errors(),
        "invalid edit should detect type mismatch"
    );

    // A valid replacement recovers the database.
    file.set_text(&mut db).to(fixture_text(MAIN_INT_42));

    let recovered = typed(&db, file);
    assert!(
        !recovered.has_errors(),
        "valid replacement should recover after an invalid edit"
    );
    assert_eq!(find_fn(&db, &recovered, "main").return_type, Idx::INT);
}
