//! Factory methods for constructing [`LexError`] instances.
//!
//! All constructors are `#[cold]` — errors are never on the hot path.

use ori_ir::Span;

use super::{LexError, LexErrorContext, LexErrorKind, LexSuggestion, UnicodeEscapeDetail};

impl LexError {
    /// Create an unterminated string error.
    #[cold]
    pub fn unterminated_string(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::UnterminatedString,
            context: LexErrorContext::InsideString { start: span.start },
            suggestions: vec![LexSuggestion::text("add closing `\"`", 0)],
        }
    }

    /// Create an unterminated char error.
    #[cold]
    pub fn unterminated_char(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::UnterminatedChar,
            context: LexErrorContext::InsideChar,
            suggestions: vec![LexSuggestion::text("add closing `'`", 0)],
        }
    }

    /// Create an unterminated template error.
    #[cold]
    pub fn unterminated_template(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::UnterminatedTemplate,
            context: LexErrorContext::InsideTemplate {
                start: span.start,
                nesting: 0,
            },
            suggestions: vec![LexSuggestion::text("add closing `` ` ``", 0)],
        }
    }

    /// Create an invalid string escape error.
    #[cold]
    pub fn invalid_string_escape(span: Span, escape_char: char) -> Self {
        Self {
            span,
            kind: LexErrorKind::InvalidStringEscape { escape_char },
            context: LexErrorContext::InsideString { start: span.start },
            suggestions: vec![LexSuggestion::text(
                r#"valid escapes are: \n, \t, \r, \", \\, \0, \u{...}"#,
                1,
            )],
        }
    }

    /// Create an invalid char escape error.
    #[cold]
    pub fn invalid_char_escape(span: Span, escape_char: char) -> Self {
        Self {
            span,
            kind: LexErrorKind::InvalidCharEscape { escape_char },
            context: LexErrorContext::InsideChar,
            suggestions: vec![LexSuggestion::text(
                r"valid escapes are: \n, \t, \r, \', \\, \0, \u{...}",
                1,
            )],
        }
    }

    /// Create an invalid template escape error.
    #[cold]
    pub fn invalid_template_escape(span: Span, escape_char: char) -> Self {
        Self {
            span,
            kind: LexErrorKind::InvalidTemplateEscape { escape_char },
            context: LexErrorContext::InsideTemplate {
                start: span.start,
                nesting: 0,
            },
            suggestions: vec![LexSuggestion::text(
                r"valid escapes are: \n, \t, \r, \`, \\, \0, \u{...}",
                1,
            )],
        }
    }

    /// Create an invalid Unicode escape error with explicit context.
    #[cold]
    pub fn invalid_unicode_escape(
        span: Span,
        detail: UnicodeEscapeDetail,
        context: LexErrorContext,
    ) -> Self {
        let suggestion = match &detail {
            UnicodeEscapeDetail::MissingOpenBrace => {
                r"Unicode escapes use the syntax `\u{XXXX}`".to_owned()
            }
            UnicodeEscapeDetail::EmptyDigits => {
                "provide at least one hex digit: `\\u{0}` to `\\u{10FFFF}`".to_owned()
            }
            UnicodeEscapeDetail::TooManyDigits => {
                "use at most 6 hex digits (maximum codepoint is `\\u{10FFFF}`)".to_owned()
            }
            UnicodeEscapeDetail::InvalidHexDigit { ch } => {
                format!("`{ch}` is not a hex digit — use 0-9, a-f, or A-F")
            }
            UnicodeEscapeDetail::MissingCloseBrace => {
                r"add closing `}` to complete the Unicode escape".to_owned()
            }
            UnicodeEscapeDetail::SurrogateCodepoint { codepoint } => {
                format!(
                    "U+{codepoint:04X} is a UTF-16 surrogate — \
                     use the actual codepoint instead"
                )
            }
            UnicodeEscapeDetail::OutOfRange { codepoint } => {
                format!("U+{codepoint:X} exceeds the maximum Unicode codepoint (U+10FFFF)")
            }
        };
        Self {
            span,
            kind: LexErrorKind::InvalidUnicodeEscape { detail },
            context,
            suggestions: vec![LexSuggestion::text(suggestion, 0)],
        }
    }

    /// Create a single-quote-in-string error.
    #[cold]
    pub fn single_quote_escape_in_string(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::SingleQuoteEscapeInString,
            context: LexErrorContext::InsideString { start: span.start },
            suggestions: vec![LexSuggestion::replace(
                r"use literal `'` without escaping",
                span,
                "'",
            )],
        }
    }

    /// Create a double-quote-in-char error.
    #[cold]
    pub fn double_quote_escape_in_char(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::DoubleQuoteEscapeInChar,
            context: LexErrorContext::InsideChar,
            suggestions: vec![LexSuggestion::replace(
                r#"use literal `"` without escaping"#,
                span,
                "\"",
            )],
        }
    }

    /// Create an integer overflow error.
    #[cold]
    pub fn int_overflow(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::IntOverflow,
            context: LexErrorContext::NumberLiteral,
            suggestions: vec![LexSuggestion::text(
                "use a smaller value (maximum is 18446744073709551615)",
                1,
            )],
        }
    }

    /// Create a hex integer overflow error.
    #[cold]
    pub fn hex_int_overflow(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::HexIntOverflow,
            context: LexErrorContext::NumberLiteral,
            suggestions: vec![LexSuggestion::text(
                "use a smaller value (maximum is 0xFFFFFFFFFFFFFFFF)",
                1,
            )],
        }
    }

    /// Create a binary integer overflow error.
    #[cold]
    pub fn bin_int_overflow(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::BinIntOverflow,
            context: LexErrorContext::NumberLiteral,
            suggestions: vec![LexSuggestion::text("use at most 64 binary digits", 1)],
        }
    }

    /// Create a float parse error.
    #[cold]
    pub fn float_parse_error(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::FloatParseError,
            context: LexErrorContext::NumberLiteral,
            suggestions: vec![LexSuggestion::text(
                "check the number format (e.g., `3.14`, `1.5e10`)",
                1,
            )],
        }
    }

    /// Create an invalid byte error.
    #[cold]
    pub fn invalid_byte(span: Span, byte: u8) -> Self {
        Self {
            span,
            kind: LexErrorKind::InvalidByte { byte },
            context: LexErrorContext::TopLevel,
            suggestions: Vec::new(),
        }
    }

    /// Create an interior null byte error (from `SourceBuffer` encoding detection).
    #[cold]
    pub fn interior_null(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::InvalidNullByte,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::text(
                "remove the null byte — null bytes are not allowed in Ori source",
                0,
            )],
        }
    }

    /// Create a UTF-8 BOM error (from `SourceBuffer` encoding detection).
    #[cold]
    pub fn utf8_bom(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::Utf8Bom,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::removal(
                "remove the UTF-8 BOM — Ori source must not start with a byte order mark",
                span,
            )],
        }
    }

    /// Create a UTF-16 LE BOM error (from `SourceBuffer` encoding detection).
    #[cold]
    pub fn utf16_le_bom(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::Utf16LeBom,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::text(
                "re-encode the file as UTF-8 — Ori does not support UTF-16",
                0,
            )],
        }
    }

    /// Create a UTF-16 BE BOM error (from `SourceBuffer` encoding detection).
    #[cold]
    pub fn utf16_be_bom(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::Utf16BeBom,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::text(
                "re-encode the file as UTF-8 — Ori does not support UTF-16",
                0,
            )],
        }
    }

    /// Create a standalone backslash error.
    #[cold]
    pub fn standalone_backslash(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::StandaloneBackslash,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::removal("remove the backslash", span)],
        }
    }

    /// Create a decimal-not-representable error for duration/size literals.
    #[cold]
    pub fn decimal_not_representable(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::DecimalNotRepresentable,
            context: LexErrorContext::NumberLiteral,
            suggestions: vec![LexSuggestion::text(
                "use a value that divides evenly into base units (nanoseconds or bytes)",
                1,
            )],
        }
    }

    /// Create a Unicode confusable error.
    #[cold]
    pub fn unicode_confusable(
        span: Span,
        found: char,
        suggested: char,
        name: &'static str,
    ) -> Self {
        Self {
            span,
            kind: LexErrorKind::UnicodeConfusable {
                found,
                suggested,
                name,
            },
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::replace(
                format!("replace with ASCII `{suggested}`"),
                span,
                suggested.to_string(),
            )],
        }
    }

    /// Create a reserved-future keyword error.
    #[cold]
    pub fn reserved_future_keyword(span: Span, keyword: &'static str) -> Self {
        Self {
            span,
            kind: LexErrorKind::ReservedFutureKeyword { keyword },
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::text(
                format!("`{keyword}` is reserved for future use; choose a different name"),
                0,
            )],
        }
    }

    /// Create a triple-equals error with replacement suggestion.
    #[cold]
    pub fn triple_equal(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::TripleEqual,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::replace(
                "use `==` for equality in Ori",
                span,
                "==",
            )],
        }
    }

    /// Create a single-quote string error.
    #[cold]
    pub fn single_quote_string(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::SingleQuoteString,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::text(
                r#"use double quotes for strings: "hello""#,
                0,
            )],
        }
    }

    /// Create an increment/decrement error.
    #[cold]
    pub fn increment_decrement(span: Span, op: &'static str) -> Self {
        let alt = if op == "++" { "x + 1" } else { "x - 1" };
        Self {
            span,
            kind: LexErrorKind::IncrementDecrement { op },
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::text(format!("use `{alt}` instead"), 0)],
        }
    }

    /// Add a context to this error.
    #[must_use]
    pub fn with_context(mut self, ctx: LexErrorContext) -> Self {
        self.context = ctx;
        self
    }

    /// Add a suggestion to this error.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: LexSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }
}
