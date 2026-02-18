//! Lexer for Ori with string interning.
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

mod comments;
mod cook_escape;
mod cooker;
mod keywords;
pub mod lex_error;
mod parse_helpers;
mod unicode_confusables;
mod what_is_next;

use comments::classify_and_normalize_comment;
use cooker::TokenCooker;
use lex_error::{DetachedDocWarning, LexError};
use ori_ir::{
    Comment, CommentKind, CommentList, ModuleExtra, Span, StringInterner, Token, TokenFlags,
    TokenKind, TokenList,
};
use ori_lexer_core::{EncodingIssueKind, RawScanner, RawTag, SourceBuffer};

/// Output from lexing with comment capture and metadata.
///
/// Contains both the token stream (for parsing) and formatting metadata,
/// plus accumulated lexer errors and warnings.
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`, `Default`
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct LexOutput {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Comments captured during lexing.
    pub comments: CommentList,
    /// Byte positions of blank lines (consecutive newlines).
    pub blank_lines: Vec<u32>,
    /// Byte positions of all newlines.
    pub newlines: Vec<u32>,
    /// Accumulated lexer errors.
    pub errors: Vec<LexError>,
    /// Accumulated warnings (e.g., detached doc comments).
    pub warnings: Vec<DetachedDocWarning>,
}

impl std::fmt::Debug for LexOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LexOutput")
            .field("tokens", &self.tokens.len())
            .field("comments", &self.comments.len())
            .field("blank_lines", &self.blank_lines.len())
            .field("newlines", &self.newlines.len())
            .field("errors", &self.errors.len())
            .field("warnings", &self.warnings.len())
            .finish()
    }
}

impl LexOutput {
    /// Create a new empty lex output.
    pub fn new() -> Self {
        LexOutput {
            tokens: TokenList::new(),
            comments: CommentList::new(),
            blank_lines: Vec::new(),
            newlines: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create with pre-allocated capacity based on source length.
    ///
    /// Ori's dense syntax (short keywords, single-char operators, `@` prefixes)
    /// produces roughly 1 token per 2-3 bytes of source. Using `source_len / 2`
    /// slightly over-allocates but eliminates Vec reallocations, which callgrind
    /// showed as 5.7% of total lexer instructions.
    pub fn with_capacity(source_len: usize) -> Self {
        LexOutput {
            tokens: TokenList::with_capacity(source_len / 2 + 1),
            comments: CommentList::new(),
            blank_lines: Vec::with_capacity(source_len / 400),
            newlines: Vec::with_capacity(source_len / 40),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Check if any lexer errors were accumulated.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get the accumulated lexer errors.
    pub fn errors(&self) -> &[LexError] {
        &self.errors
    }

    /// Convert the lexer output into a `ModuleExtra` for the parser.
    ///
    /// This transfers ownership of comments and positions into a format
    /// suitable for `ParseOutput`.
    pub fn into_metadata(self) -> ModuleExtra {
        ModuleExtra {
            comments: self.comments,
            blank_lines: self.blank_lines,
            newlines: self.newlines,
            trailing_commas: Vec::new(), // filled in by the parser
        }
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

/// Result of lexing: tokens plus accumulated errors.
///
/// This is the primary output for the parsing pipeline, carrying both the
/// token stream and any lexer errors (unterminated strings, `===`, `;`, etc.).
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct LexResult {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Accumulated lexer errors.
    pub errors: Vec<LexError>,
}

/// Lex source code into tokens and accumulated errors.
///
/// Delegates to [`lex_with_comments()`] to avoid duplicating the driver loop.
/// Comments and formatting metadata are discarded; only the token stream
/// and errors are kept.
pub fn lex_full(source: &str, interner: &StringInterner) -> LexResult {
    let output = lex_with_comments(source, interner);
    LexResult {
        tokens: output.tokens,
        errors: output.errors,
    }
}

/// Lex source code into a [`TokenList`].
///
/// Uses the hand-written `RawScanner` + `TokenCooker` pipeline.
/// Produces literals, keywords, identifiers, symbols, trivia (`Newline`),
/// and `Eof`/`Error` tokens. Each token carries [`TokenFlags`] metadata.
///
/// This is the fast path that discards errors. For the full pipeline
/// (tokens + errors), use [`lex_full()`].
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
#[expect(
    clippy::too_many_lines,
    reason = "lexer main loop with token classification"
)]
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    let buf = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut cooker = TokenCooker::new(buf.as_bytes(), interner);
    let mut output = LexOutput::with_capacity(source.len());

    // Convert encoding issues detected by SourceBuffer into LexErrors.
    // These provide more specific diagnostics than the raw scanner's generic
    // InvalidByte tokens (e.g., "UTF-8 BOM" vs "invalid byte 0xEF").
    for issue in buf.encoding_issues() {
        let issue_span = Span::new(issue.pos, issue.pos + issue.len);
        output.errors.push(match issue.kind {
            EncodingIssueKind::Utf8Bom => LexError::utf8_bom(issue_span),
            EncodingIssueKind::Utf16LeBom => LexError::utf16_le_bom(issue_span),
            EncodingIssueKind::Utf16BeBom => LexError::utf16_be_bom(issue_span),
            EncodingIssueKind::InteriorNull => LexError::interior_null(issue_span),
        });
    }

    let mut offset: u32 = 0;
    let mut last_significant_was_newline = false;

    // Trivia tracking for TokenFlags
    let mut pending_flags = TokenFlags::EMPTY;

    // Detached doc comment tracking: pending doc comment waiting for a declaration
    let mut pending_doc: Option<(Span, lex_error::DocMarker)> = None;
    let mut had_blank_line_since_doc = false;

    // IS_DOC flag: set on the next cooked token after a doc comment
    let mut pending_is_doc = false;

    loop {
        let raw = scanner.next_token();

        if raw.tag == RawTag::Eof {
            break;
        }

        let token_span = make_span(offset, raw.len);

        match raw.tag {
            // Accumulate trivia flags for the next significant token
            RawTag::Whitespace => {
                pending_flags.set(TokenFlags::SPACE_BEFORE);
            }

            // Comments: capture + classify, also accumulate trivia flag
            RawTag::LineComment => {
                let slice = &source[offset as usize..(offset + raw.len) as usize];
                let content_str = if slice.len() > 2 { &slice[2..] } else { "" };
                let (kind, normalized) = classify_and_normalize_comment(content_str);
                let content = interner.intern(&normalized);
                output
                    .comments
                    .push(Comment::new(content, token_span, kind));

                // Track doc comments for detached detection + IS_DOC flag
                if kind.is_doc() {
                    let marker = doc_comment_marker(kind);
                    if pending_doc.is_some() && had_blank_line_since_doc {
                        // Previous doc comment had a blank line gap — emit warning
                        if let Some((doc_span, doc_marker)) = pending_doc.take() {
                            output.warnings.push(DetachedDocWarning {
                                span: doc_span,
                                marker: doc_marker,
                            });
                        }
                    }
                    pending_doc = Some((token_span, marker));
                    had_blank_line_since_doc = false;
                    pending_is_doc = true;
                }

                pending_flags.set(TokenFlags::TRIVIA_BEFORE);
                last_significant_was_newline = false;
            }

            // Newlines: emit + track
            RawTag::Newline => {
                output.newlines.push(token_span.start);

                if last_significant_was_newline {
                    output.blank_lines.push(token_span.start);
                    // A blank line after a doc comment means it may be detached
                    if pending_doc.is_some() {
                        had_blank_line_since_doc = true;
                    }
                }

                let flags = finalize_flags(pending_flags);
                output
                    .tokens
                    .push_with_flags(Token::new(TokenKind::Newline, token_span), flags);
                // After a newline, the next token is at line start
                pending_flags =
                    TokenFlags::from_bits(TokenFlags::NEWLINE_BEFORE | TokenFlags::LINE_START);
                last_significant_was_newline = true;
            }

            // Interior null bytes: already reported via SourceBuffer
            // encoding_issues() with a specific diagnostic. Skip the
            // scanner's token to avoid duplicate errors.
            RawTag::InteriorNull => {}

            // Cook everything else
            _ => {
                last_significant_was_newline = false;
                let kind = cooker.cook(raw.tag, offset, raw.len);

                // Check for detached doc comments: if pending doc exists and
                // the next non-trivia token is NOT a declaration keyword, warn.
                if let Some((doc_span, doc_marker)) = pending_doc.take() {
                    if had_blank_line_since_doc || !is_declaration_start(&kind) {
                        output.warnings.push(DetachedDocWarning {
                            span: doc_span,
                            marker: doc_marker,
                        });
                    }
                    // else: correctly attached, no warning
                }

                let mut flags = finalize_flags(pending_flags);
                if cooker.last_cook_had_error() {
                    flags.set(TokenFlags::HAS_ERROR);
                }
                if cooker.last_cook_was_contextual_kw() {
                    flags.set(TokenFlags::CONTEXTUAL_KW);
                }
                if pending_is_doc {
                    flags.set(TokenFlags::IS_DOC);
                    pending_is_doc = false;
                }
                output
                    .tokens
                    .push_with_flags(Token::new(kind, token_span), flags);
                pending_flags = TokenFlags::EMPTY;
            }
        }

        offset += raw.len;
    }

    // If a doc comment is still pending at EOF, it's detached
    if let Some((doc_span, doc_marker)) = pending_doc {
        output.warnings.push(DetachedDocWarning {
            span: doc_span,
            marker: doc_marker,
        });
    }

    // Add EOF token
    let eof_pos = u32::try_from(source.len()).unwrap_or_else(|_| {
        let error_span = Span::new(u32::MAX - 1, u32::MAX);
        output.tokens.push(Token::new(TokenKind::Error, error_span));
        u32::MAX
    });
    let eof_span = Span::point(eof_pos);
    let eof_flags = finalize_flags(pending_flags);
    output
        .tokens
        .push_with_flags(Token::new(TokenKind::Eof, eof_span), eof_flags);

    // Append accumulated cooker errors to the output (preserving encoding issue
    // errors already pushed during SourceBuffer construction).
    output.errors.extend(cooker.into_errors());

    output
}

/// Create a span from offset and byte length.
#[inline]
fn make_span(offset: u32, len: u32) -> Span {
    Span::new(offset, offset + len)
}

/// Finalize pending flags for a token about to be pushed.
///
/// Sets `ADJACENT` when no whitespace, newline, or trivia preceded the token.
#[inline]
fn finalize_flags(mut flags: TokenFlags) -> TokenFlags {
    if !flags.contains(TokenFlags::SPACE_BEFORE)
        && !flags.contains(TokenFlags::NEWLINE_BEFORE)
        && !flags.contains(TokenFlags::TRIVIA_BEFORE)
    {
        flags.set(TokenFlags::ADJACENT);
    }
    flags
}

/// Map a `CommentKind` to a `DocMarker` for detached doc warning tracking.
fn doc_comment_marker(kind: CommentKind) -> lex_error::DocMarker {
    match kind {
        CommentKind::DocDescription => lex_error::DocMarker::Description,
        CommentKind::DocMember => lex_error::DocMarker::Member,
        CommentKind::DocWarning => lex_error::DocMarker::Warning,
        CommentKind::DocExample => lex_error::DocMarker::Example,
        CommentKind::Regular => lex_error::DocMarker::Plain,
    }
}

/// Check if a `TokenKind` represents the start of a declaration
/// (i.e., a valid target for a doc comment).
fn is_declaration_start(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::At        // @name function declaration
        | TokenKind::Type    // type definition
        | TokenKind::Trait   // trait definition
        | TokenKind::Let     // let binding
        | TokenKind::Pub     // pub modifier
        | TokenKind::Impl    // impl block
        | TokenKind::Use // use import
    )
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test code: source lengths always fit u32"
)]
mod tests;
