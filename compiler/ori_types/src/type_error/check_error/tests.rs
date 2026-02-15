use super::*;

#[test]
fn create_mismatch_error() {
    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::STR,
        vec![TypeProblem::StringToNumber],
        ErrorContext::default(),
    );

    assert!(matches!(error.kind, TypeErrorKind::Mismatch { .. }));
    assert!(!error.suggestions.is_empty());
}

#[test]
fn create_unknown_ident_error() {
    let error =
        TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![Name::from_raw(2)]);

    assert!(matches!(error.kind, TypeErrorKind::UnknownIdent { .. }));
    assert!(!error.suggestions.is_empty());
}

#[test]
fn create_arity_mismatch_error() {
    let error = TypeCheckError::arity_mismatch(Span::new(0, 20), 2, 4, ArityMismatchKind::Function);

    assert!(matches!(error.kind, TypeErrorKind::ArityMismatch { .. }));
    assert!(!error.suggestions.is_empty());
    assert!(error.suggestions[0].message.contains("remove"));
}

#[test]
fn error_context() {
    let context =
        ErrorContext::new(ContextKind::IfCondition).with_note("conditions must evaluate to bool");

    assert!(context.describe().is_some());
    assert!(!context.notes.is_empty());
}

#[test]
fn arity_kind_descriptions() {
    assert_eq!(ArityMismatchKind::Function.description(), "arguments");
    assert_eq!(ArityMismatchKind::Tuple.description(), "tuple elements");
}

#[test]
fn message_for_mismatch() {
    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );
    assert_eq!(error.message(), "type mismatch: expected int, found str");
}

#[test]
fn message_for_unknown_ident() {
    let error = TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![]);
    assert!(error.message().contains("unknown identifier"));
}

#[test]
fn code_for_mismatch() {
    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );
    assert_eq!(error.code(), ori_diagnostic::ErrorCode::E2001);
}

#[test]
fn code_for_unknown_ident() {
    let error = TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![]);
    assert_eq!(error.code(), ori_diagnostic::ErrorCode::E2003);
}

#[test]
fn span_method_matches_field() {
    let error = TypeCheckError::mismatch(
        Span::new(10, 20),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );
    assert_eq!(error.span(), error.span);
}

// ====================================================================
// format_message_rich tests
// ====================================================================

fn identity_type(idx: Idx) -> String {
    idx.display_name().to_string()
}

fn test_name_resolver(name: Name) -> String {
    match name.raw() {
        1 => "foo".to_string(),
        2 => "bar".to_string(),
        3 => "baz".to_string(),
        10 => "MyStruct".to_string(),
        11 => "length".to_string(),
        12 => "width".to_string(),
        20 => "Http".to_string(),
        30 => "Iter".to_string(),
        31 => "Container".to_string(),
        _ => format!("<name:{}>", name.raw()),
    }
}

#[test]
fn rich_message_unknown_ident_with_name() {
    let error = TypeCheckError::unknown_ident(Span::new(0, 3), Name::from_raw(1), vec![]);
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(msg, "unknown identifier `foo`");
}

#[test]
fn rich_message_unknown_ident_with_suggestions() {
    let error = TypeCheckError::unknown_ident(
        Span::new(0, 3),
        Name::from_raw(1),
        vec![Name::from_raw(2), Name::from_raw(3)],
    );
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(
        msg,
        "unknown identifier `foo`; did you mean `bar` or `baz`?"
    );
}

#[test]
fn rich_message_mismatch_primitives() {
    let error = TypeCheckError::mismatch(
        Span::new(0, 10),
        Idx::INT,
        Idx::STR,
        vec![],
        ErrorContext::default(),
    );
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(msg, "type mismatch: expected `int`, found `str`");
}

#[test]
fn rich_message_undefined_field() {
    let error = TypeCheckError::undefined_field(
        Span::new(0, 5),
        Idx::INT,
        Name::from_raw(11),
        vec![Name::from_raw(12)],
    );
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(msg, "no such field `length` on type `int`");
}

#[test]
fn rich_message_missing_capability() {
    let error = TypeCheckError::missing_capability(Span::new(0, 5), Name::from_raw(20), &[]);
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(msg, "missing required capability `Http`");
}

#[test]
fn rich_message_missing_fields() {
    let error = TypeCheckError::missing_fields(
        Span::new(0, 10),
        Name::from_raw(10),
        vec![Name::from_raw(11), Name::from_raw(12)],
    );
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(
        msg,
        "missing 2 required fields in `MyStruct`: `length`, `width`"
    );
}

#[test]
fn rich_message_duplicate_field() {
    let error =
        TypeCheckError::duplicate_field(Span::new(0, 5), Name::from_raw(10), Name::from_raw(11));
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(msg, "duplicate field `length` in `MyStruct`");
}

#[test]
fn rich_message_not_a_struct() {
    let error = TypeCheckError::not_a_struct(Span::new(0, 5), Name::from_raw(1));
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(msg, "`foo` is not a struct type");
}

#[test]
fn rich_message_missing_assoc_type() {
    let error =
        TypeCheckError::missing_assoc_type(Span::new(0, 5), Name::from_raw(30), Name::from_raw(31));
    let msg = error.format_message_rich(&identity_type, &test_name_resolver);
    assert_eq!(
        msg,
        "missing associated type `Iter` in impl for `Container`"
    );
}

#[test]
fn format_with_uses_pool_and_interner() {
    let pool = crate::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let name = interner.intern("my_var");

    let error = TypeCheckError::unknown_ident(Span::new(0, 6), name, vec![]);
    let msg = error.format_with(&pool, &interner);
    assert_eq!(msg, "unknown identifier `my_var`");
}
