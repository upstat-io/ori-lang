//! Tests for `#free(symbol)` block attribute on extern blocks.
//!
//! Spec: Annex E §FFI. Verifies the parser accepts
//! well-formed `#free(fn)` annotations and rejects malformed shapes.

use ori_ir::StringInterner;

/// Parse source and return the parser output.
fn parse_source(source: &str) -> crate::ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    parser.parse_module()
}

#[test]
fn test_extern_block_without_free_parses_with_none() {
    let source = r#"extern "c" from "m" { @sin (x: float) -> float as "sin" }"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "expected clean parse, got: {:?}",
        output.errors
    );
    assert_eq!(output.module.extern_blocks.len(), 1);
    assert!(output.module.extern_blocks[0].free_fn.is_none());
}

#[test]
fn test_extern_block_with_free_annotation_captures_symbol_name() {
    let source = r#"extern "c" from "sqlite" #free(sqlite3_close) { @open (path: str) -> int }"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "expected clean parse, got: {:?}",
        output.errors
    );
    assert_eq!(output.module.extern_blocks.len(), 1);
    let Some(free_fn) = output.module.extern_blocks[0].free_fn else {
        panic!("free_fn should be populated for #free(sqlite3_close)");
    };
    // Symbol parses to a Name; the test asserts the field is populated
    // with a non-zero (i.e. non-EMPTY) interned identifier.
    assert_ne!(free_fn.raw(), 0, "interned name should be non-zero");
}

#[test]
fn test_extern_block_with_free_no_parens_rejected() {
    // `#free` without `(symbol)` — missing required parenthesised argument.
    let source = r#"extern "c" #free { @noop () -> void }"#;
    let output = parse_source(source);
    assert!(
        !output.errors.is_empty(),
        "expected parse error for `#free` without parens"
    );
}

#[test]
fn test_extern_block_with_free_empty_parens_rejected() {
    // `#free()` — empty argument list rejected (E1004 expected identifier).
    let source = r#"extern "c" #free() { @noop () -> void }"#;
    let output = parse_source(source);
    assert!(
        !output.errors.is_empty(),
        "expected parse error for empty `#free()`"
    );
}

#[test]
fn test_extern_block_with_free_integer_arg_rejected() {
    // `#free(123)` — non-identifier argument rejected.
    let source = r#"extern "c" #free(123) { @noop () -> void }"#;
    let output = parse_source(source);
    assert!(
        !output.errors.is_empty(),
        "expected parse error for non-identifier `#free(123)`"
    );
}

#[test]
fn test_extern_block_free_annotation_unknown_attr_rejected() {
    // `#foo(bar)` — unknown attribute name on extern block.
    let source = r#"extern "c" #foo(bar) { @noop () -> void }"#;
    let output = parse_source(source);
    assert!(
        !output.errors.is_empty(),
        "expected parse error for unknown extern attribute"
    );
}

#[test]
fn test_extern_block_free_annotation_without_from_clause() {
    // `#free(...)` works without an intervening `from` clause.
    let source = r#"extern "c" #free(my_free) { @noop () -> void }"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "expected clean parse for `#free` without `from`, got: {:?}",
        output.errors
    );
    assert_eq!(output.module.extern_blocks.len(), 1);
    assert!(output.module.extern_blocks[0].free_fn.is_some());
}
