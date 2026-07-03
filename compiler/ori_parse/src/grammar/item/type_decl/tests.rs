//! Sum-type newline-significance + grammar-conformance parse pins.

use ori_diagnostic::ErrorCode;
use ori_ir::TypeDeclKind;

/// Parse source and return the module.
fn parse_module(source: &str) -> crate::ParseOutput {
    let interner = ori_ir::StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    parser.parse_module()
}

/// grammar.ebnf `sum_body = variant { "|" variant }`
/// is newline-silent, so a sum type with a newline after `=` and each
/// continuation variant on its own line (no leading pipe) must parse.
#[test]
fn test_multiline_sum_no_leading_pipe_parses_clean() {
    let output = parse_module("type Event =\n    Click(x: int)\n    | KeyPress(key: char);");
    assert!(
        output.errors.is_empty(),
        "multi-line sum (no leading pipe) should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Sum(variants) => assert_eq!(variants.len(), 2),
        other => panic!("expected Sum with 2 variants, got {other:?}"),
    }
}

/// A sum continuation variant on its own line puts a
/// newline before the `|`; the `while check(Pipe)` loop must skip it.
#[test]
fn test_multiline_sum_continuation_on_own_line_parses_clean() {
    let output = parse_module("type Event = Click(x: int)\n    | KeyPress(key: char);");
    assert!(
        output.errors.is_empty(),
        "sum continuation on its own line should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Sum(variants) => assert_eq!(variants.len(), 2),
        other => panic!("expected Sum with 2 variants, got {other:?}"),
    }
}

/// A BARE first variant
/// (no fields) on its own line before its continuation pipe forces the
/// sum-vs-newtype disambiguation checks (`check(&Pipe)` / `check(&LParen)`) to
/// run at a `Newline`; without a `skip_newlines()` there the type mis-parses as
/// a newtype and the continuation `| KeyPress` surfaces an error. Distinct from
/// a first variant that carries fields.
#[test]
fn test_multiline_sum_bare_first_variant_parses_clean() {
    let output = parse_module("type Event =\n    Click\n    | KeyPress(key: char);");
    assert!(
        output.errors.is_empty(),
        "bare first sum variant on its own line should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Sum(variants) => assert_eq!(variants.len(), 2),
        other => panic!("expected Sum with 2 variants, got {other:?}"),
    }
}

/// Negative pin: grammar.ebnf `sum_body = variant { "|" variant }`
/// makes `|` a SEPARATOR only, never a prefix on the first variant. The parser
/// must REJECT a leading pipe even inline — adding newline-skips must not make
/// the parser over-permissively accept a leading pipe. E1001 at the leading `|`.
#[test]
fn test_leading_pipe_on_first_variant_rejected() {
    let output = parse_module("type Event = | Click(x: int) | KeyPress(key: char);");
    assert!(
        !output.errors.is_empty(),
        "leading pipe on the first variant must be rejected"
    );
    assert!(
        output.errors.iter().any(|e| e.code() == ErrorCode::E1001),
        "expected E1001 for leading pipe; errors: {:?}",
        output.errors
    );
}

/// Negative pin: Same invariant as
/// `test_leading_pipe_on_first_variant_rejected`, clamped across the newline
/// dimension: the `skip_newlines()` calls added around the sum-vs-newtype
/// disambiguation must not let a leading pipe on its own line slip through.
#[test]
fn test_multiline_leading_pipe_on_first_variant_rejected() {
    let output = parse_module("type Event =\n    | Click(x: int)\n    | KeyPress(key: char);");
    assert!(
        !output.errors.is_empty(),
        "multi-line leading pipe on the first variant must be rejected"
    );
    assert!(
        output.errors.iter().any(|e| e.code() == ErrorCode::E1001),
        "expected E1001 for multi-line leading pipe; errors: {:?}",
        output.errors
    );
}

/// The `skip_newlines()` inserted right after `=`
/// (before dispatching struct/sum/newtype) is a SHARED call site — a struct
/// body pushed to the next line after `=` must parse for the same reason a
/// multi-line sum type does. Matrix companion to the sum-type pins above,
/// clamped across the `TypeDeclKind` dimension.
#[test]
fn test_struct_body_after_eq_newline_parses_clean() {
    let output = parse_module("type Point =\n    { x: int, y: int };");
    assert!(
        output.errors.is_empty(),
        "struct body after a `=` newline should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Struct(fields) => assert_eq!(fields.len(), 2),
        other => panic!("expected Struct with 2 fields, got {other:?}"),
    }
}

/// Same shared `skip_newlines()` call site as
/// `test_struct_body_after_eq_newline_parses_clean`, clamped for the newtype
/// arm of `TypeDeclKind` — a bare newtype body pushed to the next line after
/// `=` must parse.
#[test]
fn test_newtype_body_after_eq_newline_parses_clean() {
    let output = parse_module("type UserId =\n    int;");
    assert!(
        output.errors.is_empty(),
        "newtype body after a `=` newline should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Newtype(_) => {}
        other => panic!("expected Newtype, got {other:?}"),
    }
}

/// A `where` clause before `=` (the grammar.ebnf `type_def` position, and where
/// the formatter emits it) parses; the formatted shape re-parses.
#[test]
fn test_type_decl_where_clause_before_eq_parses_clean() {
    let output = parse_module("type Wrapper<T> where T: Clone = T;");
    assert!(
        output.errors.is_empty(),
        "a where clause before `=` should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    assert!(
        !output.module.types[0].where_clauses.is_empty(),
        "the where clause should be captured on the TypeDecl"
    );
}

/// Negative pin: a `where` clause AFTER `=`/body (the pre-grammar parser
/// position) is no longer accepted — grammar.ebnf `type_def` places
/// `where_clause` before `=`.
#[test]
fn test_type_decl_where_clause_after_eq_rejected() {
    let output = parse_module("type Wrapper<T> = T where T: Clone;");
    assert!(
        !output.errors.is_empty(),
        "a where clause after the body should be rejected"
    );
}
