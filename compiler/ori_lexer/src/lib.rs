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
//! - `comments`: Comment classification and normalization
//! - `parse_helpers`: Numeric literal parsing utilities
//! - `cooker`: Token cooking layer
//! - `keywords`: Keyword resolution
//! - `cook_escape`: Spec-strict escape processing
//! - [`lex_error`]: Lexer error types
//! - `output`: Output types (`LexOutput`, `LexResult`)
//! - `driver`: Lexer driver loop

mod api;
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

pub use api::{lex, lex_full, lex_with_comments};
pub use output::{LexOutput, LexResult};

// Re-export types needed by tests (accessed via `use super::*` in tests.rs).
#[cfg(test)]
use ori_ir::{Span, TokenKind};

#[cfg(test)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "test code: source lengths always fit u32"
)]
mod tests;
