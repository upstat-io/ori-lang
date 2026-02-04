//! Lexer for Ori using logos with string interning.
//!
//! Produces `TokenList` for Salsa queries.
//!
//! # Specification
//!
//! - Lexical grammar: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § LEXICAL GRAMMAR
//! - Prose: `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md`
//!
//! # Lexing
//!
//! The main entry point is [`lex()`], which converts source code into a [`TokenList`].
//!
//! # Token Types
//!
//! - **Literals**: integers (decimal, hex, binary), floats, strings, chars, durations, sizes
//! - **Keywords**: reserved words (`if`, `else`, `let`, etc.), type names, pattern keywords
//! - **Symbols**: operators, delimiters, punctuation
//! - **Identifiers**: user-defined names (interned for efficient comparison)
//!
//! # Escape Sequences
//!
//! String and char literals support: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`
//! Invalid escapes are preserved literally (e.g., `\q` becomes `\q`).
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
//! - [`raw_token`]: Logos-derived tokenizer definition
//! - [`convert`]: Token conversion with string interning
//! - [`escape`]: Escape sequence processing for strings and chars
//! - [`comments`]: Comment classification and normalization
//! - [`parse_helpers`]: Numeric literal parsing utilities

mod comments;
mod convert;
mod escape;
mod parse_helpers;
mod raw_token;

use comments::classify_and_normalize_comment;
use convert::convert_token;
use logos::Logos;
use ori_ir::{Comment, CommentList, Span, StringInterner, Token, TokenKind, TokenList};
use raw_token::RawToken;

/// Lex source code into a [`TokenList`].
///
/// This is the core lexing function used by the `tokens` query.
///
/// # Token Types Produced
///
/// - **Literals**: `Int`, `Float`, `String`, `Char`, `Duration`, `Size`
/// - **Keywords**: `If`, `Else`, `Let`, `For`, etc. (see [`TokenKind`])
/// - **Identifiers**: User-defined names (interned via `interner`)
/// - **Symbols**: Operators, delimiters, punctuation
/// - **Trivia**: `Newline` tokens (comments and line continuations are skipped)
/// - **Special**: `Eof` at end, `Error` for invalid tokens
///
/// # String/Char Escape Handling
///
/// String and char literals support escape sequences: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`.
/// Invalid escape sequences are preserved literally (e.g., `\q` becomes `\q`).
///
/// # Error Tokens
///
/// Invalid input produces `TokenKind::Error` tokens. The lexer continues past errors,
/// allowing partial parsing of malformed source code.
///
/// # File Size Limits
///
/// Source files larger than `u32::MAX` bytes (~4GB) will produce an error token.
/// Positions are stored as `u32` to keep tokens compact (24 bytes each).
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    let mut result = TokenList::new();
    let mut logos = RawToken::lexer(source);

    while let Some(token_result) = logos.next() {
        // Use try_from_range with saturated fallback to avoid panic on huge files
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            // File exceeds u32::MAX - use saturated position
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        match token_result {
            Ok(raw) => {
                // Skip trivia (comments, newlines, continuations)
                match raw {
                    RawToken::LineComment | RawToken::LineContinuation => {}
                    RawToken::Newline => {
                        result.push(Token::new(TokenKind::Newline, span));
                    }
                    _ => {
                        let kind = convert_token(raw, slice, interner);
                        result.push(Token::new(kind, span));
                    }
                }
            }
            Err(()) => {
                result.push(Token::new(TokenKind::Error, span));
            }
        }
    }

    // Add EOF token
    // If source exceeds u32::MAX bytes, emit error token and use max position
    let eof_pos = u32::try_from(source.len()).unwrap_or_else(|_| {
        // File too large - emit error token at end
        let error_span = Span::new(u32::MAX - 1, u32::MAX);
        result.push(Token::new(TokenKind::Error, error_span));
        u32::MAX
    });
    let eof_span = Span::point(eof_pos);
    result.push(Token::new(TokenKind::Eof, eof_span));

    result
}

/// Output from lexing with comment capture.
///
/// Contains both the token stream (for parsing) and the comment list (for formatting).
#[derive(Clone, Default)]
pub struct LexOutput {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Comments captured during lexing.
    pub comments: CommentList,
}

impl LexOutput {
    /// Create a new empty lex output.
    pub fn new() -> Self {
        LexOutput {
            tokens: TokenList::new(),
            comments: CommentList::new(),
        }
    }
}

/// Lex source code into tokens and comments.
///
/// This is the comment-preserving lexer entry point used by the formatter.
/// Returns both the token stream and a list of all comments in source order.
///
/// # Comment Classification
///
/// Comments are classified by their content:
/// - `// #...` → `DocDescription`
/// - `// @param ...` → `DocParam`
/// - `// @field ...` → `DocField`
/// - `// !...` → `DocWarning`
/// - `// >...` → `DocExample`
/// - `// ...` (anything else) → `Regular`
///
/// # Example
///
/// ```
/// use ori_lexer::lex_with_comments;
/// use ori_ir::StringInterner;
///
/// let interner = StringInterner::new();
/// let output = lex_with_comments("// comment\nlet x = 42", &interner);
/// assert_eq!(output.comments.len(), 1);
/// assert_eq!(output.tokens.len(), 6); // newline, let, x, =, 42, EOF
/// ```
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    let mut output = LexOutput::new();
    let mut logos = RawToken::lexer(source);

    while let Some(token_result) = logos.next() {
        // Use try_from_range with saturated fallback to avoid panic on huge files
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            // File exceeds u32::MAX - use saturated position
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        match token_result {
            Ok(raw) => {
                match raw {
                    RawToken::LineComment => {
                        // Capture comment - strip the leading "//"
                        let content_str = if slice.len() > 2 { &slice[2..] } else { "" };
                        let (kind, normalized) = classify_and_normalize_comment(content_str);
                        let content = interner.intern(&normalized);
                        output.comments.push(Comment::new(content, span, kind));
                    }
                    RawToken::LineContinuation => {}
                    RawToken::Newline => {
                        output.tokens.push(Token::new(TokenKind::Newline, span));
                    }
                    _ => {
                        let kind = convert_token(raw, slice, interner);
                        output.tokens.push(Token::new(kind, span));
                    }
                }
            }
            Err(()) => {
                output.tokens.push(Token::new(TokenKind::Error, span));
            }
        }
    }

    // Add EOF token
    let eof_pos = u32::try_from(source.len()).unwrap_or_else(|_| {
        let error_span = Span::new(u32::MAX - 1, u32::MAX);
        output.tokens.push(Token::new(TokenKind::Error, error_span));
        u32::MAX
    });
    let eof_span = Span::point(eof_pos);
    output.tokens.push(Token::new(TokenKind::Eof, eof_span));

    output
}
