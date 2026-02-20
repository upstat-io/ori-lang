//! Tests for Salsa queries.

use super::*;
use crate::CompilerDb;
use ori_types::Idx;
use salsa::Setter;
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

    assert_eq!(line_count(&db, file), 3); // "line1\n\nline3\n" = 3 lines
    assert_eq!(non_empty_line_count(&db, file), 2);
}

#[test]
fn test_first_line() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
    );

    assert_eq!(first_line(&db, file), "@main () -> int = 42;");
}

#[test]
fn test_incremental_recomputation() {
    let mut db = CompilerDb::new();

    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), "line1\nline2".to_string());

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

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
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
        "@main () -> int = 42;".to_string(),
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
        "@main () -> int = 1 + 2;".to_string(),
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
        "@main () -> int = 1;".to_string(),
    );

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
    file.set_text(&mut db)
        .to("@main () -> int = 2;".to_string());

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
    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
    );

    // First call
    let result1 = parsed(&db, file);
    let _ = db.take_logs();

    // Add whitespace (tokens should be identical after lexing)
    // Note: This depends on lexer behavior with whitespace
    file.set_text(&mut db)
        .to("@main () -> int = 42;".to_string());

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
        "@calc () -> int = 1 + 2 * 3;".to_string(),
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

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
    );

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert!(!result.typed.functions.is_empty());
    assert_eq!(result.typed.functions[0].return_type, Idx::INT);
}

#[test]
fn test_typed_caching() {
    let db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 1 + 2;".to_string(),
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
        "@main () -> int = 42;".to_string(),
    );

    // Initial type check
    let result1 = typed(&db, file);
    assert_eq!(result1.typed.functions[0].return_type, Idx::INT);

    // Modify source to return bool
    file.set_text(&mut db)
        .to("@main () -> bool = true;".to_string());

    // Should re-type-check with new return type
    let result2 = typed(&db, file);
    assert_eq!(result2.typed.functions[0].return_type, Idx::BOOL);
}

#[test]
fn test_typed_with_error() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = if 42 then 1 else 2;".to_string(),
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
        "@main () -> int = 42;".to_string(),
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
        "@main () -> int = 1 + 2 * 3;".to_string(),
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
        "@main () -> bool = true && false;".to_string(),
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
        "@main () -> int = if true then 1 else 2;".to_string(),
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
        "@main () -> [int] = [1, 2, 3];".to_string(),
    );

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

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
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
        "@main () -> int = 1;".to_string(),
    );

    // Initial evaluation
    let result1 = evaluated(&db, file);
    assert_eq!(result1.result, Some(EvalOutput::Int(1)));

    // Modify source
    file.set_text(&mut db)
        .to("@main () -> int = 2;".to_string());

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
        "@main () -> int =;".to_string(), // Missing expression
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
        "@foo () -> int = 100;".to_string(),
    );

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
        r"@main () -> int = {
            let x: int = 1;
            let y: int = 2;
            x + y
        }"
        .to_string(),
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
#[ignore = "recurse pattern self() not yet supported (needs Section 07)"]
fn test_evaluated_recurse_pattern() {
    use crate::eval::EvalOutput;

    let db = CompilerDb::new();

    // Test basic recurse pattern - simplest case: always return base
    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        r"@main () -> int = recurse(;
            condition: true,
            base: 42,
            step: self()
        )"
        .to_string(),
    );

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

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@add (a: int, b: int) -> int = a + b;".to_string(),
    );

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert_eq!(
        result.typed.functions.len(),
        1,
        "Should have exactly 1 function signature"
    );

    let sig = &result.typed.functions[0];
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
    assert!(
        result.typed.functions.is_empty(),
        "Empty module has no functions"
    );
}

#[test]
fn test_typed_multiple_functions() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@foo () -> int = 1;\n@bar () -> bool = true;".to_string(),
    );

    let result = typed(&db, file);

    assert!(!result.has_errors());
    assert_eq!(
        result.typed.functions.len(),
        2,
        "Should have 2 function signatures"
    );
}

#[test]
fn test_typed_determinism() {
    let db = CompilerDb::new();

    let source = "@add (x: int, y: int) -> int = x + y;\n@mul (a: int, b: int) -> int = a * b;";
    let file = SourceFile::new(&db, PathBuf::from("/test.ori"), source.to_string());

    // Call twice — should produce identical results
    let result1 = typed(&db, file);
    let result2 = typed(&db, file);

    assert_eq!(result1, result2, "must produce deterministic results");

    // Verify function order is stable (sorted by name)
    if result1.typed.functions.len() >= 2 {
        assert!(
            result1.typed.functions[0].name < result1.typed.functions[1].name,
            "Functions should be sorted by name for determinism"
        );
    }
}

// ========================================================================
// Field Access, Index Access, and Coalesce Tests
// ========================================================================

#[test]
fn test_typed_list_indexing() {
    let db = CompilerDb::new();

    let source = "@main () -> int = [10, 20, 30][0];";
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
    assert_eq!(result.typed.functions[0].return_type, Idx::INT);
}

#[test]
fn test_typed_map_indexing_with_coalesce() {
    let db = CompilerDb::new();

    let source = r#"@main () -> int = {"a": 1}["a"] ?? 0;"#;
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
    assert_eq!(result.typed.functions[0].return_type, Idx::INT);
}

#[test]
fn test_typed_struct_field_access() {
    let db = CompilerDb::new();

    let source = "type Point = { x: int, y: int }\n@main () -> int = {\n    let p = Point { x: 10, y: 20 };\n    p.x\n}";
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
    assert_eq!(result.typed.functions[0].return_type, Idx::INT);
}

#[test]
fn test_typed_nested_field_access() {
    let db = CompilerDb::new();

    let source = "type Inner = { value: int }\ntype Outer = { inner: Inner }\n@main () -> int = {\n    let o = Outer { inner: Inner { value: 42 } };\n    o.inner.value\n}";
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

    let source = "type Point = { x: int, y: int }\n@main () -> int = {\n    let p = Point { x: 5, y: 10 };\n    p.x + p.y\n}";
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
    assert_eq!(result.typed.functions[0].return_type, Idx::INT);
}

#[test]
fn test_typed_whitespace_invariance() {
    // Different horizontal whitespace should produce identical TypeCheckResult.
    // The type checker output depends on semantic content (tokens), not formatting.
    let compact = "@add (x: int, y: int) -> int = x + y;";
    let spaced = "@add  ( x : int ,  y : int )  ->  int  =  x  +  y;";
    let tabbed = "@add\t(x:\tint,\ty:\tint)\t->\tint\t=\tx\t+\ty;";

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
    assert_eq!(result_compact.typed.functions.len(), 1);
    assert_eq!(result_spaced.typed.functions.len(), 1);
    assert_eq!(result_tabbed.typed.functions.len(), 1);

    let sig_compact = &result_compact.typed.functions[0];
    let sig_spaced = &result_spaced.typed.functions[0];
    let sig_tabbed = &result_tabbed.typed.functions[0];

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

    let source = "@main () -> int = Ok(42) ?? 0;";
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
    assert_eq!(result.typed.functions[0].return_type, Idx::INT);
}

// ========================================================================
// tokens_with_metadata() Tests
// ========================================================================

#[test]
fn test_tokens_with_metadata_returns_comments() {
    use ori_ir::CommentKind;

    let db = CompilerDb::new();

    let source = "// a regular comment\n@main () -> int = 42";
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
        "// old comment\n@main () -> int = 42".to_string(),
    );

    let output1 = tokens_with_metadata(&db, file);
    assert_eq!(output1.comments.len(), 1);
    assert_eq!(output1.comments[0].kind, CommentKind::Regular);

    // Version 2: different regular comment text (same comment kind)
    file.set_text(&mut db)
        .to("// new comment\n@main () -> int = 42".to_string());

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
    file.set_text(&mut db)
        .to("// * x: param doc\n@main () -> int = 42".to_string());

    let output3 = tokens_with_metadata(&db, file);
    assert_eq!(output3.comments.len(), 1);
    assert_eq!(
        output3.comments[0].kind,
        CommentKind::DocMember,
        "comment kind should update after edit"
    );

    // Token flags differ: @main now has IS_DOC set
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
    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
    );

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
    file.set_text(&mut db)
        .to("@main  ()  ->  int  =  42;".to_string());

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

// --- Section 12.4: Salsa early cutoff verification tests ---

#[test]
fn test_comment_only_change_triggers_early_cutoff_for_parsed() {
    // Changing a regular comment's text does NOT change code tokens (same kind,
    // same flags). This means parsed() should use early cutoff and NOT re-execute.
    let mut db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "// old comment\n@main () -> int = 42".to_string(),
    );

    // First call — executes lex_result + tokens + parsed
    let _ = parsed(&db, file);
    let _ = db.take_logs(); // Clear initial logs

    // Change only the comment text (regular → regular, different text)
    file.set_text(&mut db)
        .to("// new comment\n@main () -> int = 42".to_string());

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
        "// comment v1\n@main () -> int = 42".to_string(),
    );

    // First call — execute the full pipeline
    let result1 = typed(&db, file);
    let _ = db.take_logs();

    // Change only comment text
    file.set_text(&mut db)
        .to("// comment v2\n@main () -> int = 42".to_string());

    let result2 = typed(&db, file);
    let logs = db.take_logs();

    // typed() should produce identical results
    assert_eq!(
        result1.typed.functions.len(),
        result2.typed.functions.len(),
        "typed results should be identical after comment-only change"
    );

    // typed() should NOT be in the re-executed queries.
    // We expect at most lex_result + tokens to re-execute (2 events).
    // If parsed() also re-executes that's 3, but typed() should not.
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
    let file1 = SourceFile::new(
        &db,
        PathBuf::from("/test1.ori"),
        "add (a: int, b: int) -> int = a + b".to_string(),
    );
    let type1 = typed(&db, file1);
    let hashes1 = extract_function_hashes(&type1.typed.functions, &type1.typed.expr_types);

    // Version 2: same signature, different body (extra operation)
    let file2 = SourceFile::new(
        &db,
        PathBuf::from("/test2.ori"),
        "add (a: int, b: int) -> int = a + b + 0".to_string(),
    );
    let type2 = typed(&db, file2);
    let hashes2 = extract_function_hashes(&type2.typed.functions, &type2.typed.expr_types);

    assert_eq!(hashes1.len(), 1, "should have 1 function hash");
    assert_eq!(hashes2.len(), 1, "should have 1 function hash");

    // Module hash should differ (body expression types changed)
    let mh1 = compute_module_hash(&hashes1);
    let mh2 = compute_module_hash(&hashes2);
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
    // Salsa detects the change, and downstream function hashing sees it.
    let mut db = CompilerDb::new();
    db.enable_logging();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test.ori"),
        "@main () -> int = 42;".to_string(),
    );

    let result1 = typed(&db, file);
    let _ = db.take_logs();

    // Change body only (same signature: () -> int)
    file.set_text(&mut db)
        .to("@main () -> int = 100;".to_string());

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
        result1.typed.functions[0].return_type, result2.typed.functions[0].return_type,
        "return type should be unchanged"
    );
}
