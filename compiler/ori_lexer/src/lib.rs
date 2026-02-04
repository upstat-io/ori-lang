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
use ori_ir::{
    Comment, CommentList, ModuleExtra, Span, StringInterner, Token, TokenKind, TokenList,
};
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

/// Output from lexing with comment capture and metadata.
///
/// Contains both the token stream (for parsing) and formatting metadata.
#[derive(Clone, Default)]
pub struct LexOutput {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Comments captured during lexing.
    pub comments: CommentList,
    /// Byte positions of blank lines (consecutive newlines).
    pub blank_lines: Vec<u32>,
    /// Byte positions of all newlines.
    pub newlines: Vec<u32>,
}

impl LexOutput {
    /// Create a new empty lex output.
    pub fn new() -> Self {
        LexOutput {
            tokens: TokenList::new(),
            comments: CommentList::new(),
            blank_lines: Vec::new(),
            newlines: Vec::new(),
        }
    }

    /// Create with pre-allocated capacity based on source length.
    pub fn with_capacity(source_len: usize) -> Self {
        LexOutput {
            tokens: TokenList::new(),
            comments: CommentList::new(),
            blank_lines: Vec::with_capacity(source_len / 400),
            newlines: Vec::with_capacity(source_len / 40),
        }
    }

    /// Convert the lexer output into a `ModuleExtra` for the parser.
    ///
    /// This transfers ownership of comments and positions into a format
    /// suitable for `ParseOutput`.
    pub fn into_metadata(self) -> ModuleExtra {
        let mut metadata = ModuleExtra::new();
        metadata.comments = self.comments;
        metadata.blank_lines = self.blank_lines;
        metadata.newlines = self.newlines;
        // trailing_commas will be filled in by the parser
        metadata
    }

    /// Decompose into tokens and metadata.
    ///
    /// This is the preferred way to use `LexOutput` with `parse_with_metadata`:
    ///
    /// ```ignore
    /// let lex_output = lex_with_comments(source, &interner);
    /// let (tokens, metadata) = lex_output.into_parts();
    /// let parse_output = parse_with_metadata(&tokens, metadata, &interner);
    /// ```
    pub fn into_parts(self) -> (TokenList, ModuleExtra) {
        let metadata = ModuleExtra {
            comments: self.comments,
            blank_lines: self.blank_lines,
            newlines: self.newlines,
            trailing_commas: Vec::new(),
        };
        (self.tokens, metadata)
    }
}

/// Lex source code into tokens, comments, and formatting metadata.
///
/// This is the metadata-preserving lexer entry point used by the formatter and IDE.
/// Returns the token stream, comments, and position information for:
/// - Comments (classified by type)
/// - Blank lines (for formatting preservation)
/// - Newlines (for line counting)
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
/// # Blank Line Detection
///
/// A blank line is detected when two newlines occur with only whitespace
/// or comments between them. The position of the second newline is recorded.
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
/// assert_eq!(output.newlines.len(), 1);
/// ```
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    let mut output = LexOutput::with_capacity(source.len());
    let mut logos = RawToken::lexer(source);

    // Track the previous token kind to detect blank lines
    // A blank line occurs when we see Newline -> (Comment|Whitespace)* -> Newline
    let mut last_significant_was_newline = false;

    while let Some(token_result) = logos.next() {
        // Use try_from_range with saturated fallback to avoid panic on huge files
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            // File exceeds u32::MAX - use saturated position
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        if let Ok(raw) = token_result {
            match raw {
                RawToken::LineComment => {
                    // Capture comment - strip the leading "//"
                    let content_str = if slice.len() > 2 { &slice[2..] } else { "" };
                    let (kind, normalized) = classify_and_normalize_comment(content_str);
                    let content = interner.intern(&normalized);
                    output.comments.push(Comment::new(content, span, kind));
                    // Comments ARE content on a line, so reset blank line detection
                    // A line with "// comment" is NOT a blank line
                    last_significant_was_newline = false;
                }
                RawToken::LineContinuation => {
                    // Line continuation doesn't affect blank line detection
                }
                RawToken::Newline => {
                    // Record newline position
                    output.newlines.push(span.start);

                    // Check for blank line: newline followed by newline
                    if last_significant_was_newline {
                        // This is a blank line - record the position of this (second) newline
                        output.blank_lines.push(span.start);
                    }

                    output.tokens.push(Token::new(TokenKind::Newline, span));
                    last_significant_was_newline = true;
                }
                _ => {
                    // Any non-trivia token resets blank line detection
                    last_significant_was_newline = false;
                    let kind = convert_token(raw, slice, interner);
                    output.tokens.push(Token::new(kind, span));
                }
            }
        } else {
            last_significant_was_newline = false;
            output.tokens.push(Token::new(TokenKind::Error, span));
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

#[cfg(test)]
mod tests {
    use super::*;

    // === LexOutput Tests ===

    #[test]
    fn test_lex_output_new() {
        let output = LexOutput::new();
        assert!(output.tokens.is_empty());
        assert!(output.comments.is_empty());
        assert!(output.blank_lines.is_empty());
        assert!(output.newlines.is_empty());
    }

    #[test]
    fn test_lex_output_with_capacity() {
        let output = LexOutput::with_capacity(1000);
        assert!(output.tokens.is_empty());
        assert!(output.comments.is_empty());
        // Capacity is allocated but contents are empty
    }

    #[test]
    fn test_lex_output_into_metadata() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// comment\n\nlet x = 1", &interner);
        let metadata = output.into_metadata();

        assert_eq!(metadata.comments.len(), 1);
        assert!(!metadata.blank_lines.is_empty());
        assert!(!metadata.newlines.is_empty());
    }

    // === Newline Tracking Tests ===

    #[test]
    fn test_newline_tracking_single() {
        let interner = StringInterner::new();
        let output = lex_with_comments("x\ny", &interner);

        assert_eq!(output.newlines.len(), 1);
        assert_eq!(output.newlines[0], 1); // newline at position 1
    }

    #[test]
    fn test_newline_tracking_multiple() {
        let interner = StringInterner::new();
        let output = lex_with_comments("a\nb\nc", &interner);

        assert_eq!(output.newlines.len(), 2);
        assert_eq!(output.newlines[0], 1); // after 'a'
        assert_eq!(output.newlines[1], 3); // after 'b'
    }

    #[test]
    fn test_newline_tracking_none() {
        let interner = StringInterner::new();
        let output = lex_with_comments("let x = 42", &interner);

        assert!(output.newlines.is_empty());
    }

    // === Blank Line Detection Tests ===

    #[test]
    fn test_blank_line_single() {
        let interner = StringInterner::new();
        // "a\n\nb" has a blank line between the two newlines
        let output = lex_with_comments("a\n\nb", &interner);

        assert_eq!(output.blank_lines.len(), 1);
        // The blank line is at position 2 (the second newline)
        assert_eq!(output.blank_lines[0], 2);
    }

    #[test]
    fn test_blank_line_multiple() {
        let interner = StringInterner::new();
        // "a\n\n\nb" has two blank lines
        let output = lex_with_comments("a\n\n\nb", &interner);

        assert_eq!(output.blank_lines.len(), 2);
    }

    #[test]
    fn test_blank_line_none() {
        let interner = StringInterner::new();
        // "a\nb\nc" has no blank lines
        let output = lex_with_comments("a\nb\nc", &interner);

        assert!(output.blank_lines.is_empty());
    }

    #[test]
    fn test_blank_line_at_start() {
        let interner = StringInterner::new();
        // "\n\nlet" starts with a blank line
        let output = lex_with_comments("\n\nlet", &interner);

        assert_eq!(output.blank_lines.len(), 1);
        assert_eq!(output.blank_lines[0], 1); // second newline at position 1
    }

    #[test]
    fn test_blank_line_at_end() {
        let interner = StringInterner::new();
        // "let\n\n" ends with a blank line
        let output = lex_with_comments("let\n\n", &interner);

        assert_eq!(output.blank_lines.len(), 1);
    }

    #[test]
    fn test_blank_line_with_comment_between() {
        let interner = StringInterner::new();
        // Comment between newlines should NOT create a blank line
        // because content exists on the line
        let output = lex_with_comments("a\n// comment\nb", &interner);

        // There's a newline after 'a' and 'comment', but no blank line
        assert!(output.blank_lines.is_empty());
        assert_eq!(output.comments.len(), 1);
    }

    #[test]
    fn test_blank_line_after_comment() {
        let interner = StringInterner::new();
        // Comment followed by blank line
        // "a\n// comment\n\nb"
        // Line 1: a
        // Line 2: // comment
        // Line 3: (blank)
        // Line 4: b
        let output = lex_with_comments("a\n// comment\n\nb", &interner);

        // There should be a blank line (the line after the comment)
        assert_eq!(output.blank_lines.len(), 1);
    }

    // === Comment Tracking Tests ===

    #[test]
    fn test_comment_tracking_single() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// hello\nlet x = 1", &interner);

        assert_eq!(output.comments.len(), 1);
    }

    #[test]
    fn test_comment_tracking_multiple() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// first\n// second\nlet x = 1", &interner);

        assert_eq!(output.comments.len(), 2);
    }

    #[test]
    fn test_comment_tracking_with_blank_lines() {
        let interner = StringInterner::new();
        let source = r"// comment 1

// comment 2
let x = 1";
        let output = lex_with_comments(source, &interner);

        assert_eq!(output.comments.len(), 2);
        assert_eq!(output.blank_lines.len(), 1);
    }

    // === Integration Tests ===

    #[test]
    fn test_realistic_source() {
        let interner = StringInterner::new();
        let source = r"// #Description
// This is a doc comment

@main () -> void
    let x = 42

    // Regular comment
    x
";
        let output = lex_with_comments(source, &interner);

        // Two doc comments at the top
        assert_eq!(output.comments.len(), 3);

        // Blank line after "doc comment" and after "x = 42"
        assert!(!output.blank_lines.is_empty());

        // Multiple newlines throughout
        assert!(output.newlines.len() >= 5);
    }

    #[test]
    fn test_empty_source() {
        let interner = StringInterner::new();
        let output = lex_with_comments("", &interner);

        assert!(output.comments.is_empty());
        assert!(output.blank_lines.is_empty());
        assert!(output.newlines.is_empty());
        assert_eq!(output.tokens.len(), 1); // Just EOF
    }

    #[test]
    fn test_only_newlines() {
        let interner = StringInterner::new();
        let output = lex_with_comments("\n\n\n", &interner);

        assert_eq!(output.newlines.len(), 3);
        assert_eq!(output.blank_lines.len(), 2); // Two consecutive pairs
    }

    #[test]
    fn test_only_comments() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// a\n// b\n// c", &interner);

        assert_eq!(output.comments.len(), 3);
        assert_eq!(output.newlines.len(), 2);
        assert!(output.blank_lines.is_empty());
    }

    // === Metadata Conversion Tests ===

    #[test]
    fn test_metadata_preserves_comments() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// #Description\nfn foo()", &interner);
        let metadata = output.into_metadata();

        assert_eq!(metadata.comments.len(), 1);
        assert!(metadata.comments.get(0).is_some_and(|c| c.kind.is_doc()));
    }

    #[test]
    fn test_metadata_preserves_blank_lines() {
        let interner = StringInterner::new();
        let output = lex_with_comments("a\n\nb", &interner);
        let metadata = output.into_metadata();

        assert_eq!(metadata.blank_lines.len(), 1);
        assert!(metadata.has_blank_line_between(1, 3));
    }

    #[test]
    fn test_metadata_line_number() {
        let interner = StringInterner::new();
        let output = lex_with_comments("line1\nline2\nline3", &interner);
        let metadata = output.into_metadata();

        // Position 0 is line 1
        assert_eq!(metadata.line_number(0), 1);
        // Position 7 (after first newline + "line2") is line 2
        assert_eq!(metadata.line_number(7), 2);
    }
}
