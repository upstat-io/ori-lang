use super::*;

#[test]
fn error_construction() {
    let span = Span::new(10, 15);
    let err = LexError::unterminated_string(span);
    assert_eq!(err.span, span);
    assert_eq!(err.kind, LexErrorKind::UnterminatedString);
    assert_eq!(err.context, LexErrorContext::InsideString { start: 10 });
    assert!(!err.suggestions.is_empty());
}

#[test]
fn escape_error_with_char() {
    let span = Span::new(5, 7);
    let err = LexError::invalid_string_escape(span, 'q');
    assert_eq!(
        err.kind,
        LexErrorKind::InvalidStringEscape { escape_char: 'q' }
    );
    assert!(!err.suggestions.is_empty());
}

#[test]
fn invalid_byte_error() {
    let span = Span::new(0, 1);
    let err = LexError::invalid_byte(span, 0x80);
    assert_eq!(err.kind, LexErrorKind::InvalidByte { byte: 0x80 });
    assert_eq!(err.context, LexErrorContext::TopLevel);
}

#[test]
fn error_equality() {
    let a = LexError::int_overflow(Span::new(0, 5));
    let b = LexError::int_overflow(Span::new(0, 5));
    let c = LexError::hex_int_overflow(Span::new(0, 5));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn semicolon_error_has_removal_suggestion() {
    let span = Span::new(10, 11);
    let err = LexError::semicolon(span);
    assert_eq!(err.kind, LexErrorKind::Semicolon);
    assert_eq!(err.suggestions.len(), 1);
    let suggestion = &err.suggestions[0];
    assert!(suggestion.replacement.is_some());
    let replacement = suggestion.replacement.as_ref().unwrap();
    assert_eq!(replacement.span, span);
    assert_eq!(replacement.text, "");
}

#[test]
fn triple_equal_error_has_replacement() {
    let span = Span::new(5, 8);
    let err = LexError::triple_equal(span);
    assert_eq!(err.kind, LexErrorKind::TripleEqual);
    let replacement = err.suggestions[0].replacement.as_ref().unwrap();
    assert_eq!(replacement.text, "==");
}

#[test]
fn unicode_confusable_error() {
    let span = Span::new(0, 3);
    let err = LexError::unicode_confusable(span, '\u{201C}', '"', "Left Double Quotation Mark");
    match &err.kind {
        LexErrorKind::UnicodeConfusable {
            found,
            suggested,
            name,
        } => {
            assert_eq!(*found, '\u{201C}');
            assert_eq!(*suggested, '"');
            assert_eq!(*name, "Left Double Quotation Mark");
        }
        other => panic!("expected UnicodeConfusable, got {other:?}"),
    }
}

#[test]
fn with_context_fluent_builder() {
    let err = LexError::invalid_byte(Span::new(0, 1), 0x80)
        .with_context(LexErrorContext::InsideString { start: 0 });
    assert_eq!(err.context, LexErrorContext::InsideString { start: 0 });
}

#[test]
fn with_suggestion_fluent_builder() {
    let err = LexError::invalid_byte(Span::new(0, 1), 0x80)
        .with_suggestion(LexSuggestion::text("try this", 0));
    assert_eq!(err.suggestions.len(), 1);
}

#[test]
fn all_factory_methods_compile() {
    let s = Span::new(0, 1);
    let _ = LexError::unterminated_string(s);
    let _ = LexError::unterminated_char(s);
    let _ = LexError::unterminated_template(s);
    let _ = LexError::invalid_string_escape(s, 'q');
    let _ = LexError::invalid_char_escape(s, 'q');
    let _ = LexError::invalid_template_escape(s, 'q');
    let _ = LexError::single_quote_escape_in_string(s);
    let _ = LexError::double_quote_escape_in_char(s);
    let _ = LexError::int_overflow(s);
    let _ = LexError::hex_int_overflow(s);
    let _ = LexError::bin_int_overflow(s);
    let _ = LexError::float_parse_error(s);
    let _ = LexError::invalid_byte(s, 0xFF);
    let _ = LexError::interior_null(s);
    let _ = LexError::utf8_bom(Span::new(0, 3));
    let _ = LexError::utf16_le_bom(Span::new(0, 2));
    let _ = LexError::utf16_be_bom(Span::new(0, 2));
    let _ = LexError::standalone_backslash(s);
    let _ = LexError::decimal_not_representable(s);
    let _ = LexError::unicode_confusable(s, '\u{201C}', '"', "Left Double Quotation Mark");
    let _ = LexError::semicolon(s);
    let _ = LexError::triple_equal(s);
    let _ = LexError::single_quote_string(s);
    let _ = LexError::increment_decrement(s, "++");
}

#[test]
fn error_hash_compatible() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    let e1 = LexError::semicolon(Span::new(0, 1));
    let e2 = LexError::semicolon(Span::new(0, 1));
    let e3 = LexError::triple_equal(Span::new(0, 3));
    set.insert(e1);
    set.insert(e2); // duplicate
    set.insert(e3);
    assert_eq!(set.len(), 2);
}

#[test]
fn detached_doc_warning_structure() {
    let w = DetachedDocWarning {
        span: Span::new(0, 10),
        marker: DocMarker::Description,
    };
    assert_eq!(w.span, Span::new(0, 10));
    assert_eq!(w.marker, DocMarker::Description);
}

// Encoding issue factory tests

#[test]
fn utf8_bom_error() {
    let span = Span::new(0, 3);
    let err = LexError::utf8_bom(span);
    assert_eq!(err.kind, LexErrorKind::Utf8Bom);
    assert_eq!(err.span, span);
    // Has a removal suggestion
    assert_eq!(err.suggestions.len(), 1);
    assert!(err.suggestions[0].replacement.is_some());
}

#[test]
fn utf16_le_bom_error() {
    let span = Span::new(0, 2);
    let err = LexError::utf16_le_bom(span);
    assert_eq!(err.kind, LexErrorKind::Utf16LeBom);
    assert_eq!(err.span, span);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn utf16_be_bom_error() {
    let span = Span::new(0, 2);
    let err = LexError::utf16_be_bom(span);
    assert_eq!(err.kind, LexErrorKind::Utf16BeBom);
    assert_eq!(err.span, span);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn interior_null_error() {
    let span = Span::new(5, 6);
    let err = LexError::interior_null(span);
    assert_eq!(err.kind, LexErrorKind::InvalidNullByte);
    assert_eq!(err.span, span);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn lex_suggestion_constructors() {
    let text = LexSuggestion::text("try this", 1);
    assert!(text.replacement.is_none());
    assert_eq!(text.priority, 1);

    let removal = LexSuggestion::removal("remove it", Span::new(0, 1));
    assert!(removal.replacement.is_some());
    assert_eq!(removal.replacement.as_ref().unwrap().text, "");

    let replace = LexSuggestion::replace("change it", Span::new(0, 3), "==");
    assert_eq!(replace.replacement.as_ref().unwrap().text, "==");
}
