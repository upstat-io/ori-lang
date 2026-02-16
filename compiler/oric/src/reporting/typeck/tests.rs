use super::*;
use crate::ir::{Name, Span};
use ori_diagnostic::ErrorCode;
use ori_types::{ArityMismatchKind, ContextKind, TypeProblem};

/// Create a test `Pool` and `StringInterner`.
fn test_env() -> (Pool, StringInterner) {
    (Pool::new(), StringInterner::new())
}

#[test]
fn mismatch_with_primitives() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );

    let diag = renderer.render(&error);
    assert_eq!(diag.code, ErrorCode::E2001);
    assert!(
        diag.message.contains("int"),
        "message should contain 'int': {}",
        diag.message
    );
    assert!(
        diag.message.contains("str"),
        "message should contain 'str': {}",
        diag.message
    );
}

#[test]
fn mismatch_with_complex_types() {
    let (mut pool, interner) = test_env();

    // Create a [int] list type in the Pool
    let list_int = pool.list(Idx::INT);

    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::mismatch(
        Span::new(5, 15),
        list_int,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );

    let diag = renderer.render(&error);
    // Should show "[int]" not "<type>"
    assert!(
        diag.message.contains("[int]"),
        "message should contain '[int]' not '<type>': {}",
        diag.message
    );
    assert!(diag.message.contains("str"));
}

#[test]
fn unknown_ident_shows_name() {
    let (pool, interner) = test_env();
    let name = interner.intern("my_variable");
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::unknown_ident(Span::new(0, 11), name, vec![]);

    let diag = renderer.render(&error);
    assert_eq!(diag.code, ErrorCode::E2003);
    assert!(
        diag.message.contains("my_variable"),
        "message should contain identifier name: {}",
        diag.message
    );
}

#[test]
fn undefined_field_shows_field_and_type() {
    let (pool, interner) = test_env();
    let field_name = interner.intern("length");
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::undefined_field(Span::new(0, 10), Idx::INT, field_name, vec![]);

    let diag = renderer.render(&error);
    assert!(
        diag.message.contains("length"),
        "message should contain field name: {}",
        diag.message
    );
    assert!(
        diag.message.contains("int"),
        "message should contain type name: {}",
        diag.message
    );
}

#[test]
fn arity_mismatch_correct_counts() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::arity_mismatch(Span::new(0, 20), 2, 4, ArityMismatchKind::Function);

    let diag = renderer.render(&error);
    assert_eq!(diag.code, ErrorCode::E2004);
    assert!(
        diag.message.contains('2') && diag.message.contains('4'),
        "message should contain expected and found counts: {}",
        diag.message
    );
}

#[test]
fn arity_mismatch_with_func_name() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::arity_mismatch_named(Span::new(0, 20), "add".to_string(), 2, 3);

    let diag = renderer.render(&error);
    assert!(
        diag.message.contains("add"),
        "message should contain function name: {}",
        diag.message
    );
}

#[test]
fn error_context_produces_notes() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let context = ErrorContext::new(ContextKind::IfCondition).with_note("conditions must be bool");

    let error = TypeCheckError::mismatch(Span::new(0, 10), Idx::BOOL, Idx::INT, vec![], context);

    let diag = renderer.render(&error);
    assert!(
        !diag.notes.is_empty(),
        "diagnostic should have notes from context"
    );
    // Should contain the context description
    assert!(
        diag.notes.iter().any(|n| n.contains("if expression")),
        "notes should contain context description: {:?}",
        diag.notes
    );
    // Should contain the explicit note
    assert!(
        diag.notes
            .iter()
            .any(|n| n.contains("conditions must be bool")),
        "notes should contain explicit note: {:?}",
        diag.notes
    );
}

#[test]
fn text_suggestions_go_to_suggestions() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::FLOAT,
        vec![ori_types::TypeProblem::IntFloat {
            expected: "int",
            found: "float",
        }],
        ErrorContext::default(),
    );

    let diag = renderer.render(&error);
    // IntFloat suggestions are text-only, should appear in suggestions
    assert!(
        !diag.suggestions.is_empty(),
        "text-only suggestions should be in diag.suggestions"
    );
    assert!(
        diag.suggestions.iter().any(|s| s.contains("int(x)")),
        "should suggest int(x): {:?}",
        diag.suggestions
    );
}

#[test]
fn span_suggestions_go_to_structured() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    // Create an error with a span-bearing suggestion
    let structured_suggestion =
        Suggestion::text_with_replacement("replace with correct type", 0, Span::new(5, 10), "int");

    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    )
    .with_suggestion(structured_suggestion);

    let diag = renderer.render(&error);
    assert!(
        !diag.structured_suggestions.is_empty(),
        "span-bearing suggestions should be in diag.structured_suggestions"
    );
}

#[test]
fn error_codes_map_correctly() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    // Mismatch -> E2001
    let mismatch = TypeCheckError::mismatch(
        Span::new(0, 5),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );
    assert_eq!(renderer.render(&mismatch).code, ErrorCode::E2001);

    // UnknownIdent -> E2003
    let ident = TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![]);
    assert_eq!(renderer.render(&ident).code, ErrorCode::E2003);

    // ArityMismatch -> E2004
    let arity = TypeCheckError::arity_mismatch(Span::new(0, 5), 2, 3, ArityMismatchKind::Function);
    assert_eq!(renderer.render(&arity).code, ErrorCode::E2004);

    // InfiniteType -> E2008
    let infinite = TypeCheckError::infinite_type(Span::new(0, 5), None);
    assert_eq!(renderer.render(&infinite).code, ErrorCode::E2008);

    // AmbiguousType -> E2005
    let ambiguous = TypeCheckError::ambiguous_type(Span::new(0, 5), 1, "expression".into());
    assert_eq!(renderer.render(&ambiguous).code, ErrorCode::E2005);
}

#[test]
fn render_type_errors_helper() {
    let (pool, interner) = test_env();

    let errors = vec![
        TypeCheckError::mismatch(
            Span::new(0, 5),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        ),
        TypeCheckError::unknown_ident(Span::new(10, 15), interner.intern("foo"), vec![]),
    ];

    let diagnostics = render_type_errors(&errors, &pool, &interner);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code, ErrorCode::E2001);
    assert_eq!(diagnostics[1].code, ErrorCode::E2003);
    assert!(diagnostics[1].message.contains("foo"));
}

#[test]
fn closure_self_capture_label_not_error_types() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    // Closure self-capture uses Idx::ERROR for both expected and found
    let error = TypeCheckError::closure_self_capture(Span::new(5, 6));

    let diag = renderer.render(&error);
    assert_eq!(diag.code, ErrorCode::E2001);
    assert!(
        diag.message.contains("closure cannot capture itself"),
        "message: {}",
        diag.message
    );
    // Label should NOT contain "<error>" - it should be problem-specific
    let label_text = &diag.labels[0].message;
    assert!(
        !label_text.contains("<error>"),
        "label should not show raw error types, got: {label_text}"
    );
    assert!(
        label_text.contains("self-referential"),
        "label should describe the problem, got: {label_text}"
    );
}

#[test]
fn bad_operand_label_is_specific() {
    let (pool, interner) = test_env();
    let renderer = TypeErrorRenderer::new(&pool, &interner);

    let error = TypeCheckError::mismatch(
        Span::new(0, 5),
        Idx::INT,
        Idx::FLOAT,
        vec![TypeProblem::BadOperandType {
            op: "-",
            op_category: "unary",
            found_type: "float",
            required_type: "int",
        }],
        ErrorContext::default(),
    );

    let diag = renderer.render(&error);
    let label_text = &diag.labels[0].message;
    assert!(
        label_text.contains("cannot apply"),
        "label should describe the operator problem, got: {label_text}"
    );
}
