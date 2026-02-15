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
    // String/char errors
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

    // Numeric errors
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

    // Character errors
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
    /// UTF-8 BOM at file start. Forbidden per spec: `02-source-code.md` § Encoding.
    Utf8Bom,
    /// UTF-16 LE BOM at file start. Wrong encoding — Ori requires UTF-8.
    Utf16LeBom,
    /// UTF-16 BE BOM at file start. Wrong encoding — Ori requires UTF-8.
    Utf16BeBom,
    /// ASCII control character (0x01-0x1F except `\t`, `\n`, `\r`).
    InvalidControlChar { byte: u8 },

    // Unit literal errors
    /// Decimal duration/size literal cannot be represented as a whole number
    /// of base units (nanoseconds for duration, bytes for size).
    DecimalNotRepresentable,

    // Reserved-future keyword errors
    /// A keyword reserved for future use (`asm`, `inline`, `static`, `union`, `view`).
    ReservedFutureKeyword { keyword: &'static str },

    // Cross-language pattern errors
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
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test assertions")]
mod tests;
