//! Lex-time problem definitions.
//!
//! Lex errors (`LexError`) are rendered directly via [`render_lex_error()`].
//! This module defines `LexProblem` for lex-time warnings (detached doc
//! comments) that flow through the `oric` diagnostic pipeline.

use crate::diagnostic::{Diagnostic, ErrorCode, Suggestion};
use crate::ir::Span;
use ori_lexer::lex_error::{LexError, LexErrorKind, UnicodeEscapeDetail};

/// Lex-time warnings detected during tokenization.
///
/// Lex *errors* are rendered directly via [`render_lex_error()`] from
/// `&LexError` references. This enum covers lex-time *warnings* that
/// need structured representation for the diagnostic pipeline.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum LexProblem {
    /// A detached doc comment warning.
    DetachedDocComment {
        span: Span,
        marker: ori_lexer::lex_error::DocMarker,
    },
}

impl LexProblem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        match self {
            LexProblem::DetachedDocComment { span, .. } => *span,
        }
    }

    /// Convert this problem into a diagnostic.
    #[cold]
    pub fn into_diagnostic(&self) -> Diagnostic {
        match self {
            LexProblem::DetachedDocComment { span, .. } => Diagnostic::warning(ErrorCode::E0012)
                .with_message("detached doc comment")
                .with_label(*span, "this doc comment is not attached to any declaration")
                .with_suggestion(
                    "doc comments should appear immediately before a function (`@name`), \
                         `type`, `trait`, or other declaration",
                ),
        }
    }
}

/// Render a [`LexError`] and its structured suggestions.
#[cold]
pub fn render_lex_error(err: &LexError) -> Diagnostic {
    let mut diag = diagnostic_for_lex_kind(&err.kind, err.span);
    for suggestion in &err.suggestions {
        if let Some(ref replacement) = suggestion.replacement {
            diag = diag.with_structured_suggestion(Suggestion::machine_applicable(
                &suggestion.message,
                replacement.span,
                &replacement.text,
            ));
        } else {
            diag = diag.with_suggestion(&suggestion.message);
        }
    }

    diag
}

fn diagnostic_for_lex_kind(kind: &LexErrorKind, span: Span) -> Diagnostic {
    match kind {
        LexErrorKind::UnterminatedString => simple_lex_error(
            ErrorCode::E0001,
            span,
            "unterminated string literal",
            "string not closed",
        ),
        LexErrorKind::UnterminatedChar => simple_lex_error(
            ErrorCode::E0004,
            span,
            "unterminated character literal",
            "character literal not closed",
        ),
        LexErrorKind::UnterminatedTemplate => simple_lex_error(
            ErrorCode::E0006,
            span,
            "unterminated template literal",
            "template literal not closed",
        ),
        LexErrorKind::InvalidStringEscape { escape_char } => {
            invalid_escape(span, *escape_char, "string")
        }
        LexErrorKind::InvalidCharEscape { escape_char } => {
            invalid_escape(span, *escape_char, "character literal")
        }
        LexErrorKind::InvalidTemplateEscape { escape_char } => {
            invalid_escape(span, *escape_char, "template literal")
        }
        LexErrorKind::InvalidUnicodeEscape { detail } => unicode_escape_error(span, detail),
        LexErrorKind::SingleQuoteEscapeInString => simple_lex_error(
            ErrorCode::E0005,
            span,
            r"`\'` is not a valid escape in string literals",
            "not valid in strings",
        ),
        LexErrorKind::DoubleQuoteEscapeInChar => simple_lex_error(
            ErrorCode::E0005,
            span,
            r#"`\"` is not a valid escape in character literals"#,
            "not valid in char literals",
        ),
        LexErrorKind::IntOverflow => integer_overflow_error(span, "integer"),
        LexErrorKind::HexIntOverflow => integer_overflow_error(span, "hexadecimal integer"),
        LexErrorKind::BinIntOverflow => integer_overflow_error(span, "binary integer"),
        LexErrorKind::FloatParseError => simple_lex_error(
            ErrorCode::E0003,
            span,
            "invalid float literal",
            "could not parse as a float",
        ),
        LexErrorKind::InvalidByte { byte } => invalid_byte_error(span, *byte),
        LexErrorKind::StrictEqualityOperator { operator } => strict_equality_error(span, operator),
        LexErrorKind::SingleQuoteString => single_quote_string_error(span),
        LexErrorKind::IncrementOperator { operator } => increment_operator_error(span, operator),
        LexErrorKind::StandaloneBackslash => simple_lex_error(
            ErrorCode::E0013,
            span,
            "standalone `\\` is not a valid token",
            "unexpected backslash",
        ),
        LexErrorKind::UnicodeConfusable {
            found,
            suggested,
            name,
        } => unicode_confusable_error(span, *found, *suggested, name),
        LexErrorKind::InvalidNullByte => simple_lex_error(
            ErrorCode::E0002,
            span,
            "null byte in source",
            "unexpected null byte",
        ),
        LexErrorKind::Utf8Bom => encoding_error(
            span,
            "source file starts with a UTF-8 BOM",
            "byte order mark not allowed",
            "Ori source files must be UTF-8 without a byte order mark",
        ),
        LexErrorKind::Utf16LeBom => encoding_error(
            span,
            "source file appears to be UTF-16 LE encoded",
            "UTF-16 LE byte order mark detected",
            "Ori source files must be UTF-8 encoded",
        ),
        LexErrorKind::Utf16BeBom => encoding_error(
            span,
            "source file appears to be UTF-16 BE encoded",
            "UTF-16 BE byte order mark detected",
            "Ori source files must be UTF-8 encoded",
        ),
        LexErrorKind::DecimalNotRepresentable => decimal_not_representable_error(span),
        LexErrorKind::ReservedFutureKeyword { keyword } => simple_lex_error(
            ErrorCode::E0015,
            span,
            format!("`{keyword}` is reserved for future use"),
            "reserved keyword",
        ),
    }
}

fn simple_lex_error(
    code: ErrorCode,
    span: Span,
    message: impl Into<String>,
    label: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(code)
        .with_message(message)
        .with_label(span, label)
}

fn invalid_escape(span: Span, escape_char: char, literal: &str) -> Diagnostic {
    simple_lex_error(
        ErrorCode::E0005,
        span,
        format!("invalid escape sequence `\\{escape_char}` in {literal}"),
        "unknown escape",
    )
}

fn unicode_escape_error(span: Span, detail: &UnicodeEscapeDetail) -> Diagnostic {
    let (message, label) = match detail {
        UnicodeEscapeDetail::MissingOpenBrace => (
            r"missing `{` after `\u` in Unicode escape".to_owned(),
            r"expected `\u{...}`",
        ),
        UnicodeEscapeDetail::EmptyDigits => (
            "empty Unicode escape `\\u{}`".to_owned(),
            "provide at least one hex digit",
        ),
        UnicodeEscapeDetail::TooManyDigits => (
            "too many digits in Unicode escape".to_owned(),
            "at most 6 hex digits allowed",
        ),
        UnicodeEscapeDetail::InvalidHexDigit { ch } => (
            format!("invalid hex digit `{ch}` in Unicode escape"),
            "not a valid hex digit",
        ),
        UnicodeEscapeDetail::MissingCloseBrace => (
            r"missing `}` in Unicode escape".to_owned(),
            "expected closing `}`",
        ),
        UnicodeEscapeDetail::SurrogateCodepoint { codepoint } => (
            format!("surrogate codepoint U+{codepoint:04X} in Unicode escape"),
            "surrogates (U+D800-U+DFFF) are not valid",
        ),
        UnicodeEscapeDetail::OutOfRange { codepoint } => (
            format!("codepoint U+{codepoint:X} out of range in Unicode escape"),
            "maximum codepoint is U+10FFFF",
        ),
    };
    simple_lex_error(ErrorCode::E0005, span, message, label)
}

fn integer_overflow_error(span: Span, literal: &str) -> Diagnostic {
    simple_lex_error(
        ErrorCode::E0003,
        span,
        format!("{literal} literal overflows `int`"),
        "value exceeds maximum integer",
    )
}

fn invalid_byte_error(span: Span, byte: u8) -> Diagnostic {
    if byte.is_ascii_control() {
        simple_lex_error(
            ErrorCode::E0002,
            span,
            format!("invalid control character (0x{byte:02X})"),
            "unexpected control character",
        )
    } else {
        simple_lex_error(
            ErrorCode::E0002,
            span,
            format!("invalid character `{}`", char::from(byte)),
            "unexpected character",
        )
    }
}

fn strict_equality_error(span: Span, operator: &str) -> Diagnostic {
    let replacement = if operator == "!==" { "!=" } else { "==" };
    Diagnostic::error(ErrorCode::E0008)
        .with_message(format!("`{operator}` is not an Ori equality operator"))
        .with_label(span, "cross-language equality habit")
        .with_note("Ori uses structural equality operators, not strict equality")
        .with_suggestion(format!("use `{replacement}` instead"))
}

fn single_quote_string_error(span: Span) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0009)
        .with_message("single-quoted strings are not valid Ori syntax")
        .with_label(span, "this looks like a string")
        .with_note("Ori uses double quotes for strings and single quotes for one character")
        .with_suggestion("use double quotes for string literals")
}

fn increment_operator_error(span: Span, operator: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0010)
        .with_message(format!("`{operator}` is not an Ori operator"))
        .with_label(span, "increment operators are not supported")
        .with_note("Ori uses explicit assignment for updates")
        .with_suggestion("write the update explicitly, such as `x = x + 1`")
}

fn unicode_confusable_error(span: Span, found: char, suggested: char, name: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0011)
        .with_message(format!(
            "found {name} (`{found}`), expected ASCII `{suggested}`"
        ))
        .with_label(span, format!("this is `{found}`, not `{suggested}`"))
        .with_note("this often happens when copying code from a word processor or web page")
}

fn encoding_error(span: Span, message: &str, label: &str, note: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0002)
        .with_message(message)
        .with_label(span, label)
        .with_note(note)
}

fn decimal_not_representable_error(span: Span) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0014)
        .with_message("decimal literal cannot be represented as a whole number of base units")
        .with_label(span, "value is not a whole number of nanoseconds or bytes")
        .with_note("decimal duration/size values must resolve to whole numbers of base units")
}
