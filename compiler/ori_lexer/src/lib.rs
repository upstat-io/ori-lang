#![deny(unsafe_code)]
//! Lexer for Ori with string interning.
//!
//! Produces `TokenList` for Salsa queries.
//!
//! # Specification
//!
//! - Lexical grammar: `docs/ori_lang/v2026/spec/grammar.ebnf` § LEXICAL GRAMMAR
//! - Prose: `docs/ori_lang/v2026/spec/03-lexical-elements.md`
//!
//! # Lexing
//!
//! The main entry point is [`lex()`], which converts source code into a [`TokenList`].
//! Uses the hand-written `RawScanner` from `ori_lexer_core` with a `TokenCooker`
//! that resolves keywords, parses literals, and processes escape sequences.
//!
//! # Token Types
//!
//! - **Literals**: integers (decimal, hex, binary), floats, strings, chars, durations, sizes
//! - **Keywords**: reserved words (`if`, `else`, `let`, etc.), type names, pattern keywords
//! - **Symbols**: operators, delimiters, punctuation
//! - **Identifiers**: user-defined names (interned for efficient comparison)
//!
//! # Error Handling
//!
//! Invalid tokens produce `TokenKind::Error`. The lexer continues processing after errors.
//!
//! # File Size Limits
//!
//! Source files larger than `u32::MAX` bytes (~4GB) will emit an error token.
//! Spans use `u32` for positions to keep tokens compact.
//!
//! # Modules
//!
//! - [`comments`]: Comment classification and normalization
//! - [`parse_helpers`]: Numeric literal parsing utilities
//! - [`cooker`]: Token cooking layer
//! - [`keywords`]: Keyword resolution
//! - [`cook_escape`]: Spec-strict escape processing
//! - [`lex_error`]: Lexer error types
//! - [`output`]: Output types (`LexOutput`, `LexResult`)
//! - [`driver`]: Lexer driver loop

mod comments;
mod cook_escape;
mod cooker;
mod driver;
mod keywords;
pub mod lex_error;
mod output;
mod parse_helpers;
mod trivial;
mod unicode_confusables;
mod what_is_next;

pub use output::{LexOutput, LexResult};

use driver::lex_driver;
use ori_ir::{StringInterner, TokenList};

// Re-export types needed by tests (accessed via `use super::*` in tests.rs).
#[cfg(test)]
use ori_ir::{Span, TokenKind};

/// Lex source code into tokens and accumulated errors.
///
/// Collects metadata internally (comments, newlines) but returns only
/// the token stream and errors. For metadata (comments, formatting info),
/// use [`lex_with_comments()`].
#[must_use]
pub fn lex_full(source: &str, interner: &StringInterner) -> LexResult {
    let output = lex_driver::<false>(source, interner);
    LexResult {
        tokens: output.tokens,
        errors: output.errors,
    }
}

/// Lex source code into a [`TokenList`].
///
/// Wraps [`lex_full()`] and discards errors, returning only the token
/// stream. For the full pipeline (tokens + errors), use [`lex_full()`].
#[must_use]
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    lex_full(source, interner).tokens
}

/// Lex source code into tokens, comments, and formatting metadata.
///
/// This is the metadata-preserving lexer entry point used by the formatter and IDE.
/// Returns the token stream, comments, and position information for:
/// - Comments (classified by type)
/// - Blank lines (for formatting preservation)
/// - Newlines (for line counting)
///
/// Each token carries [`TokenFlags`] metadata capturing whitespace/trivia context.
#[must_use]
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    lex_driver::<true>(source, interner)
}

#[cfg(test)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "test code: source lengths always fit u32"
)]
mod tests;
