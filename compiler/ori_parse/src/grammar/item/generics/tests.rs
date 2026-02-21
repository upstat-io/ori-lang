use ori_ir::{StringInterner, WhereClause};

/// Parse a source string and return the where clauses from the first function.
fn parse_where_clauses(source: &str) -> Vec<WhereClause> {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    let output = parser.parse_module();
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    output.module.functions[0].where_clauses.clone()
}

#[test]
fn test_where_type_bound() {
    // Regression: T: Clone still parses as TypeBound
    let clauses = parse_where_clauses("@f<T> () -> void where T: Clone = ();");
    assert_eq!(clauses.len(), 1);
    assert!(clauses[0].is_type_bound());
}

#[test]
fn test_where_type_bound_with_projection() {
    // T.Item: Eq — associated type constraint
    let clauses = parse_where_clauses("@f<T> () -> void where T.Item: Eq = ();");
    assert_eq!(clauses.len(), 1);
    assert!(clauses[0].is_type_bound());
}

#[test]
fn test_where_multiple_type_bounds() {
    // Multiple type bounds: T: Clone, U: Default
    let clauses = parse_where_clauses("@f<T, U> () -> void where T: Clone, U: Default = ();");
    assert_eq!(clauses.len(), 2);
    assert!(clauses[0].is_type_bound());
    assert!(clauses[1].is_type_bound());
}

#[test]
fn test_where_const_bound() {
    // N > 0 — const bound expression
    let clauses = parse_where_clauses("@f<$N: int> () -> void where N > 0 = ();");
    assert_eq!(clauses.len(), 1);
    assert!(clauses[0].is_const_bound());
}

#[test]
fn test_where_mixed_type_and_const_bounds() {
    // T: Clone, N > 0 — mixed type bound + const bound
    let clauses = parse_where_clauses("@f<T, $N: int> () -> void where T: Clone, N > 0 = ();");
    assert_eq!(clauses.len(), 2);
    assert!(clauses[0].is_type_bound());
    assert!(clauses[1].is_const_bound());
}
