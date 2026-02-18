//! Lex-time problem definitions.
//!
//! Lex errors (`LexError`) are rendered directly via [`render_lex_error()`].
//! This module defines `LexProblem` for lex-time warnings (detached doc
//! comments) that flow through the `oric` diagnostic pipeline.

use crate::diagnostic::{Diagnostic, ErrorCode, Suggestion};
use crate::ir::Span;
use ori_lexer::lex_error::{LexError, LexErrorKind};

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

/// Render a `LexError` into a `Diagnostic` with appropriate error code,
/// message, labels, and suggestions.
///
/// Public so that callers can render lex errors directly from `&LexError`
/// without cloning into a `LexProblem::Error` wrapper.
#[cold]
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive LexErrorKind → diagnostic dispatch"
)]
pub fn render_lex_error(err: &LexError) -> Diagnostic {
    let span = err.span;
    let mut diag = match &err.kind {
        LexErrorKind::UnterminatedString => Diagnostic::error(ErrorCode::E0001)
            .with_message("unterminated string literal")
            .with_label(span, "string not closed"),

        LexErrorKind::UnterminatedChar => Diagnostic::error(ErrorCode::E0004)
            .with_message("unterminated character literal")
            .with_label(span, "character literal not closed"),

        LexErrorKind::UnterminatedTemplate => Diagnostic::error(ErrorCode::E0006)
            .with_message("unterminated template literal")
            .with_label(span, "template literal not closed"),

        LexErrorKind::InvalidStringEscape { escape_char } => Diagnostic::error(ErrorCode::E0005)
            .with_message(format!(
                "invalid escape sequence `\\{escape_char}` in string"
            ))
            .with_label(span, "unknown escape"),

        LexErrorKind::InvalidCharEscape { escape_char } => Diagnostic::error(ErrorCode::E0005)
            .with_message(format!(
                "invalid escape sequence `\\{escape_char}` in character literal"
            ))
            .with_label(span, "unknown escape"),

        LexErrorKind::InvalidTemplateEscape { escape_char } => Diagnostic::error(ErrorCode::E0005)
            .with_message(format!(
                "invalid escape sequence `\\{escape_char}` in template literal"
            ))
            .with_label(span, "unknown escape"),

        LexErrorKind::SingleQuoteEscapeInString => Diagnostic::error(ErrorCode::E0005)
            .with_message(r"`\'` is not a valid escape in string literals")
            .with_label(span, "not valid in strings"),

        LexErrorKind::DoubleQuoteEscapeInChar => Diagnostic::error(ErrorCode::E0005)
            .with_message(r#"`\"` is not a valid escape in character literals"#)
            .with_label(span, "not valid in char literals"),

        LexErrorKind::EmptyCharLiteral => Diagnostic::error(ErrorCode::E0004)
            .with_message("empty character literal")
            .with_label(span, "character literal must contain exactly one character"),

        LexErrorKind::MultiCharLiteral => Diagnostic::error(ErrorCode::E0004)
            .with_message("character literal contains multiple characters")
            .with_label(span, "expected a single character"),

        LexErrorKind::IntOverflow => Diagnostic::error(ErrorCode::E0003)
            .with_message("integer literal overflows `int`")
            .with_label(span, "value exceeds maximum integer"),

        LexErrorKind::HexIntOverflow => Diagnostic::error(ErrorCode::E0003)
            .with_message("hexadecimal integer literal overflows `int`")
            .with_label(span, "value exceeds maximum integer"),

        LexErrorKind::BinIntOverflow => Diagnostic::error(ErrorCode::E0003)
            .with_message("binary integer literal overflows `int`")
            .with_label(span, "value exceeds maximum integer"),

        LexErrorKind::FloatParseError => Diagnostic::error(ErrorCode::E0003)
            .with_message("invalid float literal")
            .with_label(span, "could not parse as a float"),

        LexErrorKind::InvalidDigitForRadix { digit, radix } => Diagnostic::error(ErrorCode::E0003)
            .with_message(format!("invalid digit `{digit}` for base-{radix} literal"))
            .with_label(span, format!("not valid in base {radix}")),

        LexErrorKind::EmptyExponent => Diagnostic::error(ErrorCode::E0003)
            .with_message("expected digits after exponent")
            .with_label(span, "add digits after `e`"),

        LexErrorKind::LeadingZero => Diagnostic::error(ErrorCode::E0003)
            .with_message("leading zeros are not allowed in decimal literals")
            .with_label(span, "remove leading zero"),

        LexErrorKind::TrailingUnderscore => Diagnostic::error(ErrorCode::E0003)
            .with_message("trailing underscore in numeric literal")
            .with_label(span, "remove trailing underscore"),

        LexErrorKind::ConsecutiveUnderscores => Diagnostic::error(ErrorCode::E0003)
            .with_message("consecutive underscores in numeric literal")
            .with_label(span, "use a single underscore"),

        LexErrorKind::InvalidByte { byte } => {
            let ch = *byte as char;
            if byte.is_ascii_control() {
                Diagnostic::error(ErrorCode::E0002)
                    .with_message(format!("invalid control character (0x{byte:02X})"))
                    .with_label(span, "unexpected control character")
            } else {
                Diagnostic::error(ErrorCode::E0002)
                    .with_message(format!("invalid character `{ch}`"))
                    .with_label(span, "unexpected character")
            }
        }

        LexErrorKind::StandaloneBackslash => Diagnostic::error(ErrorCode::E0013)
            .with_message("standalone `\\` is not a valid token")
            .with_label(span, "unexpected backslash"),

        LexErrorKind::UnicodeConfusable {
            found,
            suggested,
            name,
        } => Diagnostic::error(ErrorCode::E0011)
            .with_message(format!(
                "found {name} (`{found}`), expected ASCII `{suggested}`"
            ))
            .with_label(span, format!("this is `{found}`, not `{suggested}`"))
            .with_note("this often happens when copying code from a word processor or web page"),

        LexErrorKind::InvalidNullByte => Diagnostic::error(ErrorCode::E0002)
            .with_message("null byte in source")
            .with_label(span, "unexpected null byte"),

        LexErrorKind::Utf8Bom => Diagnostic::error(ErrorCode::E0002)
            .with_message("source file starts with a UTF-8 BOM")
            .with_label(span, "byte order mark not allowed")
            .with_note("Ori source files must be UTF-8 without a byte order mark"),

        LexErrorKind::Utf16LeBom => Diagnostic::error(ErrorCode::E0002)
            .with_message("source file appears to be UTF-16 LE encoded")
            .with_label(span, "UTF-16 LE byte order mark detected")
            .with_note("Ori source files must be UTF-8 encoded"),

        LexErrorKind::Utf16BeBom => Diagnostic::error(ErrorCode::E0002)
            .with_message("source file appears to be UTF-16 BE encoded")
            .with_label(span, "UTF-16 BE byte order mark detected")
            .with_note("Ori source files must be UTF-8 encoded"),

        LexErrorKind::InvalidControlChar { byte } => Diagnostic::error(ErrorCode::E0002)
            .with_message(format!("invalid control character (0x{byte:02X})"))
            .with_label(span, "unexpected control character"),

        LexErrorKind::DecimalNotRepresentable => Diagnostic::error(ErrorCode::E0014)
            .with_message("decimal literal cannot be represented as a whole number of base units")
            .with_label(span, "value is not a whole number of nanoseconds or bytes")
            .with_note("decimal duration/size values must resolve to whole numbers of base units"),

        // Reserved-future keywords
        LexErrorKind::ReservedFutureKeyword { keyword } => Diagnostic::error(ErrorCode::E0015)
            .with_message(format!("`{keyword}` is reserved for future use"))
            .with_label(span, "reserved keyword"),

        // Cross-language pattern errors
        LexErrorKind::Semicolon => Diagnostic::error(ErrorCode::E0007)
            .with_message("Ori doesn't use semicolons — expressions are separated by newlines")
            .with_label(span, "remove this semicolon"),

        LexErrorKind::TripleEqual => Diagnostic::error(ErrorCode::E0008)
            .with_message("Ori uses `==` for equality, not `===`")
            .with_label(span, "replace with `==`"),

        LexErrorKind::SingleQuoteString => Diagnostic::error(ErrorCode::E0009)
            .with_message("strings in Ori use double quotes, not single quotes")
            .with_label(span, r#"use `"..."` instead of `'...'`"#),

        LexErrorKind::IncrementDecrement { op } => {
            let alt = if *op == "++" { "x + 1" } else { "x - 1" };
            Diagnostic::error(ErrorCode::E0010)
                .with_message(format!("Ori doesn't have `{op}` — use `{alt}` instead"))
                .with_label(span, format!("use `{alt}` instead"))
        }

        LexErrorKind::TernaryOperator => Diagnostic::error(ErrorCode::E0002)
            .with_message("Ori uses `if`/`else` expressions, not ternary `? :`")
            .with_label(span, "use `if condition then a else b`"),
    };

    // Attach suggestions from the LexError
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
