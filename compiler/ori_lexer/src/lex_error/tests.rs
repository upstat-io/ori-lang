use super::*;

/// Assert a constructed `LexError` carries the exact fields its factory produces.
fn assert_lex_fields(
    err: &LexError,
    span: Span,
    expected_kind: &LexErrorKind,
    expected_context: &LexErrorContext,
    expected_suggestion_count: usize,
) {
    assert_eq!(err.span, span);
    assert_eq!(&err.kind, expected_kind);
    assert_eq!(&err.context, expected_context);
    assert_eq!(err.suggestions.len(), expected_suggestion_count);
}

#[test]
fn unterminated_string_produces_kind_context_and_suggestion() {
    let span = Span::new(10, 15);
    let err = LexError::unterminated_string(span);
    assert_eq!(err.span, span);
    assert_eq!(err.kind, LexErrorKind::UnterminatedString);
    assert_eq!(err.context, LexErrorContext::InsideString { start: 10 });
    assert!(!err.suggestions.is_empty());
}

#[test]
fn invalid_string_escape_carries_escape_char_and_suggestion() {
    let span = Span::new(5, 7);
    let err = LexError::invalid_string_escape(span, 'q');
    assert_eq!(
        err.kind,
        LexErrorKind::InvalidStringEscape { escape_char: 'q' }
    );
    assert!(!err.suggestions.is_empty());
}

#[test]
fn invalid_byte_produces_top_level_context_and_no_suggestions() {
    let span = Span::new(0, 1);
    let err = LexError::invalid_byte(span, 0x80);
    assert_eq!(err.kind, LexErrorKind::InvalidByte { byte: 0x80 });
    assert_eq!(err.context, LexErrorContext::TopLevel);
    // invalid_byte is the sole factory producing no suggestions.
    assert!(err.suggestions.is_empty());
}

#[test]
fn lex_error_equality_same_kind_equal_different_kind_distinct() {
    let a = LexError::int_overflow(Span::new(0, 5));
    let b = LexError::int_overflow(Span::new(0, 5));
    let c = LexError::hex_int_overflow(Span::new(0, 5));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn unicode_confusable_carries_found_suggested_and_name() {
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
fn string_char_template_factories_produce_expected_fields() {
    let s = Span::new(0, 1);
    let in_str = LexErrorContext::InsideString { start: 0 };
    let in_tmpl = LexErrorContext::InsideTemplate {
        start: 0,
        nesting: 0,
    };
    let cases: [(LexError, LexErrorKind, LexErrorContext); 8] = [
        (
            LexError::unterminated_string(s),
            LexErrorKind::UnterminatedString,
            in_str.clone(),
        ),
        (
            LexError::unterminated_char(s),
            LexErrorKind::UnterminatedChar,
            LexErrorContext::InsideChar,
        ),
        (
            LexError::unterminated_template(s),
            LexErrorKind::UnterminatedTemplate,
            in_tmpl.clone(),
        ),
        (
            LexError::invalid_string_escape(s, 'q'),
            LexErrorKind::InvalidStringEscape { escape_char: 'q' },
            in_str.clone(),
        ),
        (
            LexError::invalid_char_escape(s, 'q'),
            LexErrorKind::InvalidCharEscape { escape_char: 'q' },
            LexErrorContext::InsideChar,
        ),
        (
            LexError::invalid_template_escape(s, 'q'),
            LexErrorKind::InvalidTemplateEscape { escape_char: 'q' },
            in_tmpl,
        ),
        (
            LexError::single_quote_escape_in_string(s),
            LexErrorKind::SingleQuoteEscapeInString,
            in_str,
        ),
        (
            LexError::double_quote_escape_in_char(s),
            LexErrorKind::DoubleQuoteEscapeInChar,
            LexErrorContext::InsideChar,
        ),
    ];
    let mut checked = 0;
    for (err, kind, ctx) in &cases {
        assert_lex_fields(err, s, kind, ctx, 1);
        checked += 1;
    }
    assert_eq!(
        checked, 8,
        "every string/char/template factory must be field-asserted"
    );
}

#[test]
fn numeric_factories_produce_number_literal_context() {
    let s = Span::new(0, 1);
    let cases: [(LexError, LexErrorKind); 5] = [
        (LexError::int_overflow(s), LexErrorKind::IntOverflow),
        (LexError::hex_int_overflow(s), LexErrorKind::HexIntOverflow),
        (LexError::bin_int_overflow(s), LexErrorKind::BinIntOverflow),
        (
            LexError::float_parse_error(s),
            LexErrorKind::FloatParseError,
        ),
        (
            LexError::decimal_not_representable(s),
            LexErrorKind::DecimalNotRepresentable,
        ),
    ];
    let mut checked = 0;
    for (err, kind) in &cases {
        assert_lex_fields(err, s, kind, &LexErrorContext::NumberLiteral, 1);
        checked += 1;
    }
    assert_eq!(checked, 5, "every numeric factory must be field-asserted");
}

#[test]
fn encoding_and_top_level_factories_produce_expected_fields() {
    let s = Span::new(0, 1);
    let bom3 = Span::new(0, 3);
    let bom2 = Span::new(0, 2);
    let top = LexErrorContext::TopLevel;
    // span, error, kind, suggestion count. invalid_byte is the lone no-suggestion factory.
    let cases: [(Span, LexError, LexErrorKind, usize); 8] = [
        (
            s,
            LexError::invalid_byte(s, 0xFF),
            LexErrorKind::InvalidByte { byte: 0xFF },
            0,
        ),
        (
            s,
            LexError::interior_null(s),
            LexErrorKind::InvalidNullByte,
            1,
        ),
        (bom3, LexError::utf8_bom(bom3), LexErrorKind::Utf8Bom, 1),
        (
            bom2,
            LexError::utf16_le_bom(bom2),
            LexErrorKind::Utf16LeBom,
            1,
        ),
        (
            bom2,
            LexError::utf16_be_bom(bom2),
            LexErrorKind::Utf16BeBom,
            1,
        ),
        (
            s,
            LexError::standalone_backslash(s),
            LexErrorKind::StandaloneBackslash,
            1,
        ),
        (
            s,
            LexError::unicode_confusable(s, '\u{201C}', '"', "Left Double Quotation Mark"),
            LexErrorKind::UnicodeConfusable {
                found: '\u{201C}',
                suggested: '"',
                name: "Left Double Quotation Mark",
            },
            1,
        ),
        (
            s,
            LexError::reserved_future_keyword(s, "asm"),
            LexErrorKind::ReservedFutureKeyword { keyword: "asm" },
            1,
        ),
    ];
    let mut checked = 0;
    for (span, err, kind, count) in &cases {
        assert_lex_fields(err, *span, kind, &top, *count);
        checked += 1;
    }
    assert_eq!(
        checked, 8,
        "every encoding/top-level factory must be field-asserted"
    );
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
fn detached_doc_warning_carries_span_and_marker() {
    let w = DetachedDocWarning {
        span: Span::new(0, 10),
        marker: DocMarker::Description,
    };
    assert_eq!(w.span, Span::new(0, 10));
    assert_eq!(w.marker, DocMarker::Description);
}

// Encoding issue factory tests

#[test]
fn utf8_bom_produces_kind_span_and_removal_suggestion() {
    let span = Span::new(0, 3);
    let err = LexError::utf8_bom(span);
    assert_eq!(err.kind, LexErrorKind::Utf8Bom);
    assert_eq!(err.span, span);
    // Has a removal suggestion
    assert_eq!(err.suggestions.len(), 1);
    assert!(err.suggestions[0].replacement.is_some());
}

#[test]
fn utf16_le_bom_produces_kind_span_and_suggestion() {
    let span = Span::new(0, 2);
    let err = LexError::utf16_le_bom(span);
    assert_eq!(err.kind, LexErrorKind::Utf16LeBom);
    assert_eq!(err.span, span);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn utf16_be_bom_produces_kind_span_and_suggestion() {
    let span = Span::new(0, 2);
    let err = LexError::utf16_be_bom(span);
    assert_eq!(err.kind, LexErrorKind::Utf16BeBom);
    assert_eq!(err.span, span);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn interior_null_produces_invalid_null_byte_kind() {
    let span = Span::new(5, 6);
    let err = LexError::interior_null(span);
    assert_eq!(err.kind, LexErrorKind::InvalidNullByte);
    assert_eq!(err.span, span);
    assert!(!err.suggestions.is_empty());
}

#[test]
fn lex_suggestion_text_removal_replace_set_expected_fields() {
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
fn invalid_unicode_escape_every_detail_produces_expected_fields() {
    let s = Span::new(0, 1);
    let ctx = LexErrorContext::InsideChar;
    let details = [
        UnicodeEscapeDetail::MissingOpenBrace,
        UnicodeEscapeDetail::EmptyDigits,
        UnicodeEscapeDetail::TooManyDigits,
        UnicodeEscapeDetail::InvalidHexDigit { ch: 'G' },
        UnicodeEscapeDetail::MissingCloseBrace,
        UnicodeEscapeDetail::SurrogateCodepoint { codepoint: 0xD800 },
        UnicodeEscapeDetail::OutOfRange {
            codepoint: 0x11_0000,
        },
    ];

    let mut checked = 0;
    for detail in &details {
        let err = LexError::invalid_unicode_escape(s, detail.clone(), ctx.clone());
        assert_lex_fields(
            &err,
            s,
            &LexErrorKind::InvalidUnicodeEscape {
                detail: detail.clone(),
            },
            &ctx,
            1,
        );
        checked += 1;
    }
    assert_eq!(
        checked, 7,
        "every UnicodeEscapeDetail variant must be field-asserted"
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
