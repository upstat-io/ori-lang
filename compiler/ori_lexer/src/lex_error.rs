//! Lexer error types for the V2 cooking layer.
//!
//! Errors follow the WHERE+WHAT+WHY+HOW shape (v2-conventions §5):
//! - WHERE: `span` locating the error in source
//! - WHAT: `kind` describing what went wrong
//! - WHY: `context` explaining what the lexer was doing
//! - HOW: `suggestions` providing actionable fixes
//!
//! All types derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility.

use ori_ir::Span;

/// A lexer error with full context for diagnostic rendering.
///
/// Follows the cross-system error shape from `v2-conventions.md` §5.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LexError {
    /// WHERE the error occurred.
    pub span: Span,
    /// WHAT went wrong.
    pub kind: LexErrorKind,
    /// WHY we were checking (lexing context at the point of error).
    pub context: LexErrorContext,
    /// HOW to fix (actionable suggestions).
    pub suggestions: Vec<LexSuggestion>,
}

/// What kind of lexer error occurred.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LexErrorKind {
    // === String/Char Errors ===
    /// Missing closing `"` for string literal.
    UnterminatedString,
    /// Missing closing `'` for char literal.
    UnterminatedChar,
    /// Missing closing `` ` `` for template literal.
    UnterminatedTemplate,
    /// Invalid escape in a string literal (e.g., `\q`).
    InvalidStringEscape { escape_char: char },
    /// Invalid escape in a char literal.
    InvalidCharEscape { escape_char: char },
    /// Invalid escape in a template literal.
    InvalidTemplateEscape { escape_char: char },
    /// `\'` used in a string literal — not valid per grammar line 102.
    SingleQuoteEscapeInString,
    /// `\"` used in a char literal — not valid per grammar line 127.
    DoubleQuoteEscapeInChar,
    /// Empty char literal `''`.
    EmptyCharLiteral,
    /// Multiple characters in char literal `'ab'`.
    MultiCharLiteral,

    // === Numeric Errors ===
    /// Integer literal overflowed `u64`.
    IntOverflow,
    /// Hex integer literal overflowed `u64`.
    HexIntOverflow,
    /// Binary integer literal overflowed `u64`.
    BinIntOverflow,
    /// Float literal could not be parsed.
    FloatParseError,
    /// Invalid digit for the given radix (e.g., `0xGG`).
    InvalidDigitForRadix { digit: char, radix: u8 },
    /// Empty exponent in float literal (e.g., `1.5e`).
    EmptyExponent,
    /// Leading zero in decimal literal (e.g., `007`).
    LeadingZero,
    /// Trailing underscore in numeric literal (e.g., `100_`).
    TrailingUnderscore,
    /// Consecutive underscores in numeric literal (e.g., `1__000`).
    ConsecutiveUnderscores,

    // === Character Errors ===
    /// Non-printable or invalid byte in source.
    InvalidByte { byte: u8 },
    /// Standalone `\` outside of escape context.
    StandaloneBackslash,
    /// Unicode character visually similar to an ASCII character.
    UnicodeConfusable {
        found: char,
        suggested: char,
        name: &'static str,
    },
    /// Interior null byte in source.
    InvalidNullByte,
    /// ASCII control character (0x01-0x1F except `\t`, `\n`, `\r`).
    InvalidControlChar { byte: u8 },

    // === Unit Literal Errors ===
    /// Decimal duration/size literal cannot be represented as a whole number
    /// of base units (nanoseconds for duration, bytes for size).
    DecimalNotRepresentable,

    // === Reserved-Future Keyword Errors ===
    /// A keyword reserved for future use (`asm`, `inline`, `static`, `union`, `view`).
    ReservedFutureKeyword { keyword: &'static str },

    // === Cross-Language Pattern Errors ===
    /// `;` used (C/JavaScript/Rust habit).
    Semicolon,
    /// `===` or `!==` used (JavaScript habit).
    TripleEqual,
    /// `'string'` used instead of `"string"` (Python/JS habit).
    SingleQuoteString,
    /// `++` or `--` used (C/JavaScript habit).
    IncrementDecrement { op: &'static str },
    /// `? :` ternary operator pattern (C habit).
    TernaryOperator,
}

/// Lexing context at the point of error — the WHY.
///
/// Describes what the lexer was doing when the error occurred,
/// matching the `ErrorContext` pattern from types V2's `TypeCheckError`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum LexErrorContext {
    /// Top-level scanning (not inside any literal).
    #[default]
    TopLevel,
    /// Inside a string literal.
    InsideString { start: u32 },
    /// Inside a char literal.
    InsideChar,
    /// Inside a template literal.
    InsideTemplate { start: u32, nesting: u32 },
    /// Inside a numeric literal.
    NumberLiteral,
}

/// Suggestion for fixing a lexical error — the HOW.
///
/// Internal type; final rendering in `oric` maps to
/// `ori_diagnostic::Suggestion` (with `Applicability`).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LexSuggestion {
    /// Human-readable message describing the fix.
    pub message: String,
    /// Concrete text replacement for auto-fix, if applicable.
    pub replacement: Option<LexReplacement>,
    /// Priority (lower = more likely relevant). 0 = most likely.
    pub priority: u8,
}

/// A concrete text replacement for an auto-fix.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LexReplacement {
    /// The span to replace.
    pub span: Span,
    /// The replacement text.
    pub text: String,
}

/// A warning about a detached doc comment.
///
/// A doc comment not immediately followed by a declaration is "detached"
/// and likely not attached to what the author intended.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DetachedDocWarning {
    /// Location of the detached doc comment.
    pub span: Span,
    /// What kind of doc marker was used.
    pub marker: DocMarker,
}

/// The kind of doc comment marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DocMarker {
    /// `#` description marker.
    Description,
    /// `* name:` member marker (also used for legacy `@param`/`@field`).
    Member,
    /// `!` warning marker.
    Warning,
    /// `>` example marker.
    Example,
    /// No special marker (regular doc).
    Plain,
}

impl LexSuggestion {
    /// Create a text-only suggestion (no code replacement).
    pub fn text(message: impl Into<String>, priority: u8) -> Self {
        Self {
            message: message.into(),
            replacement: None,
            priority,
        }
    }

    /// Create a suggestion with a removal (replace span with empty string).
    pub fn removal(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            replacement: Some(LexReplacement {
                span,
                text: String::new(),
            }),
            priority: 0,
        }
    }

    /// Create a suggestion with a replacement.
    pub fn replace(message: impl Into<String>, span: Span, text: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            replacement: Some(LexReplacement {
                span,
                text: text.into(),
            }),
            priority: 0,
        }
    }
}

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
                r#"valid escapes are: \n, \t, \r, \", \\, \0"#,
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
                r"valid escapes are: \n, \t, \r, \', \\, \0",
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
                r"valid escapes are: \n, \t, \r, \`, \\, \0",
                1,
            )],
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

    /// Create a semicolon error with removal suggestion.
    #[cold]
    pub fn semicolon(span: Span) -> Self {
        Self {
            span,
            kind: LexErrorKind::Semicolon,
            context: LexErrorContext::TopLevel,
            suggestions: vec![LexSuggestion::removal("remove the semicolon", span)],
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
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
}
