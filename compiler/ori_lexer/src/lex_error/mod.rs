//! Lexer error types for the V2 cooking layer.
//!
//! Errors follow the WHERE+WHAT+WHY+HOW shape (v2-conventions §5):
//! - WHERE: `span` locating the error in source
//! - WHAT: `kind` describing what went wrong
//! - WHY: `context` explaining what the lexer was doing
//! - HOW: `suggestions` providing actionable fixes
//!
//! All types derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility.

mod factories;

use ori_ir::Span;

/// Detail about what went wrong in a `\u{...}` Unicode escape sequence.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum UnicodeEscapeDetail {
    /// Missing `{` after `\u`.
    MissingOpenBrace,
    /// `\u{}` — no hex digits between braces.
    EmptyDigits,
    /// More than 6 hex digits in `\u{...}`.
    TooManyDigits,
    /// Non-hex character inside `\u{...}`.
    InvalidHexDigit { ch: char },
    /// Missing closing `}` after hex digits.
    MissingCloseBrace,
    /// Codepoint is a UTF-16 surrogate (U+D800–U+DFFF).
    SurrogateCodepoint { codepoint: u32 },
    /// Codepoint exceeds U+10FFFF.
    OutOfRange { codepoint: u32 },
}

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
    /// Invalid Unicode escape sequence `\u{...}` in any literal context.
    InvalidUnicodeEscape { detail: UnicodeEscapeDetail },
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test assertions")]
mod tests;
