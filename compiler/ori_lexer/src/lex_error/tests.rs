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
}

#[test]
fn error_hash_compatible() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    let e1 = LexError::standalone_backslash(Span::new(0, 1));
    let e2 = LexError::standalone_backslash(Span::new(0, 1));
    let e3 = LexError::standalone_backslash(Span::new(5, 6));
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

// Error code coverage

#[test]
fn every_lex_error_kind_has_error_code() {
    // Construct one of every LexErrorKind variant and verify it returns a non-empty code
    let variants: Vec<LexErrorKind> = vec![
        LexErrorKind::UnterminatedString,
        LexErrorKind::UnterminatedChar,
        LexErrorKind::UnterminatedTemplate,
        LexErrorKind::InvalidStringEscape { escape_char: 'q' },
        LexErrorKind::InvalidCharEscape { escape_char: 'q' },
        LexErrorKind::InvalidTemplateEscape { escape_char: 'q' },
        LexErrorKind::InvalidUnicodeEscape {
            detail: UnicodeEscapeDetail::EmptyDigits,
        },
        LexErrorKind::SingleQuoteEscapeInString,
        LexErrorKind::DoubleQuoteEscapeInChar,
        LexErrorKind::IntOverflow,
        LexErrorKind::HexIntOverflow,
        LexErrorKind::BinIntOverflow,
        LexErrorKind::FloatParseError,
        LexErrorKind::InvalidByte { byte: 0xFF },
        LexErrorKind::StandaloneBackslash,
        LexErrorKind::UnicodeConfusable {
            found: '\u{201C}',
            suggested: '"',
            name: "test",
        },
        LexErrorKind::InvalidNullByte,
        LexErrorKind::Utf8Bom,
        LexErrorKind::Utf16LeBom,
        LexErrorKind::Utf16BeBom,
        LexErrorKind::DecimalNotRepresentable,
        LexErrorKind::ReservedFutureKeyword { keyword: "asm" },
    ];

    for kind in &variants {
        let code = kind.error_code();
        assert!(
            !code.is_empty(),
            "LexErrorKind::{kind:?} returned empty error code"
        );
        assert!(
            code.starts_with('E'),
            "LexErrorKind::{kind:?} error code {code:?} doesn't start with 'E'"
        );
    }
}

// Unicode escape error factory tests

#[test]
fn invalid_unicode_escape_missing_open_brace() {
    let span = Span::new(5, 7);
    let err = LexError::invalid_unicode_escape(
        span,
        UnicodeEscapeDetail::MissingOpenBrace,
        LexErrorContext::InsideChar,
    );
    assert_eq!(
        err.kind,
        LexErrorKind::InvalidUnicodeEscape {
            detail: UnicodeEscapeDetail::MissingOpenBrace
        }
    );
    assert_eq!(err.context, LexErrorContext::InsideChar);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn invalid_unicode_escape_surrogate() {
    let span = Span::new(0, 10);
    let err = LexError::invalid_unicode_escape(
        span,
        UnicodeEscapeDetail::SurrogateCodepoint { codepoint: 0xD800 },
        LexErrorContext::InsideString { start: 0 },
    );
    assert_eq!(
        err.kind,
        LexErrorKind::InvalidUnicodeEscape {
            detail: UnicodeEscapeDetail::SurrogateCodepoint { codepoint: 0xD800 }
        }
    );
    assert_eq!(err.context, LexErrorContext::InsideString { start: 0 });
}

#[test]
fn invalid_unicode_escape_out_of_range() {
    let span = Span::new(0, 12);
    let err = LexError::invalid_unicode_escape(
        span,
        UnicodeEscapeDetail::OutOfRange {
            codepoint: 0x11_0000,
        },
        LexErrorContext::InsideTemplate {
            start: 0,
            nesting: 0,
        },
    );
    assert_eq!(
        err.kind,
        LexErrorKind::InvalidUnicodeEscape {
            detail: UnicodeEscapeDetail::OutOfRange {
                codepoint: 0x11_0000
            }
        }
    );
    assert!(matches!(
        err.context,
        LexErrorContext::InsideTemplate { .. }
    ));
}

#[test]
fn invalid_unicode_escape_all_factory_variants_compile() {
    let s = Span::new(0, 1);
    let ctx = LexErrorContext::InsideChar;
    let _ = LexError::invalid_unicode_escape(s, UnicodeEscapeDetail::MissingOpenBrace, ctx.clone());
    let _ = LexError::invalid_unicode_escape(s, UnicodeEscapeDetail::EmptyDigits, ctx.clone());
    let _ = LexError::invalid_unicode_escape(s, UnicodeEscapeDetail::TooManyDigits, ctx.clone());
    let _ = LexError::invalid_unicode_escape(
        s,
        UnicodeEscapeDetail::InvalidHexDigit { ch: 'G' },
        ctx.clone(),
    );
    let _ =
        LexError::invalid_unicode_escape(s, UnicodeEscapeDetail::MissingCloseBrace, ctx.clone());
    let _ = LexError::invalid_unicode_escape(
        s,
        UnicodeEscapeDetail::SurrogateCodepoint { codepoint: 0xD800 },
        ctx.clone(),
    );
    let _ = LexError::invalid_unicode_escape(
        s,
        UnicodeEscapeDetail::OutOfRange {
            codepoint: 0x11_0000,
        },
        ctx,
    );
}

#[test]
fn escape_suggestions_mention_unicode() {
    // Verify existing escape error factories now mention \u{...}
    let s = Span::new(0, 2);
    let str_err = LexError::invalid_string_escape(s, 'q');
    assert!(str_err.suggestions[0].message.contains(r"\u{...}"));

    let char_err = LexError::invalid_char_escape(s, 'q');
    assert!(char_err.suggestions[0].message.contains(r"\u{...}"));

    let tmpl_err = LexError::invalid_template_escape(s, 'q');
    assert!(tmpl_err.suggestions[0].message.contains(r"\u{...}"));
}
