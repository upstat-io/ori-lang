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
//! - [`token_flags`]: Per-token whitespace metadata (re-exported from `ori_ir`)
//! - [`lex_error`]: Lexer error types

mod comments;
mod cook_escape;
mod cooker;
pub mod foreign_keywords;
mod keywords;
pub mod lex_error;
mod parse_helpers;
pub mod token_flags;
pub mod unicode_confusables;
mod what_is_next;

// Re-export core types from the standalone tokenizer crate.
pub use ori_lexer_core::{
    Cursor, EncodingIssue, EncodingIssueKind, RawTag, RawToken, SourceBuffer,
};

use comments::classify_and_normalize_comment;
use cooker::TokenCooker;
use lex_error::{DetachedDocWarning, LexError};
use ori_ir::{
    Comment, CommentKind, CommentList, ModuleExtra, Span, StringInterner, Token, TokenFlags,
    TokenKind, TokenList,
};
use ori_lexer_core::RawScanner;

/// Output from lexing with comment capture and metadata.
///
/// Contains both the token stream (for parsing) and formatting metadata,
/// plus accumulated lexer errors and warnings.
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`, `Default`
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
    /// Accumulated lexer errors.
    pub errors: Vec<LexError>,
    /// Accumulated warnings (e.g., detached doc comments).
    pub warnings: Vec<DetachedDocWarning>,
}

impl PartialEq for LexOutput {
    fn eq(&self, other: &Self) -> bool {
        self.tokens == other.tokens
            && self.comments == other.comments
            && self.blank_lines == other.blank_lines
            && self.newlines == other.newlines
            && self.errors == other.errors
            && self.warnings == other.warnings
    }
}

impl Eq for LexOutput {}

impl std::hash::Hash for LexOutput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
        self.comments.hash(state);
        self.blank_lines.hash(state);
        self.newlines.hash(state);
        self.errors.hash(state);
        self.warnings.hash(state);
    }
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

/// Lex source code into a [`TokenList`].
///
/// Uses the hand-written `RawScanner` + `TokenCooker` pipeline.
/// Produces literals, keywords, identifiers, symbols, trivia (`Newline`),
/// and `Eof`/`Error` tokens. Each token carries [`TokenFlags`] metadata.
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    let buf = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut cooker = TokenCooker::new(buf.as_bytes(), interner);
    let mut result = TokenList::with_capacity(source.len() / 2 + 1);
    let mut offset: u32 = 0;

    // Trivia tracking for TokenFlags
    let mut pending_flags = TokenFlags::EMPTY;

    loop {
        let raw = scanner.next_token();

        if raw.tag == RawTag::Eof {
            break;
        }

        match raw.tag {
            // Accumulate trivia flags for the next significant token
            RawTag::Whitespace => {
                pending_flags.set(TokenFlags::SPACE_BEFORE);
            }
            RawTag::LineComment => {
                pending_flags.set(TokenFlags::TRIVIA_BEFORE);
            }

            // Emit newline tokens (significant for parser)
            RawTag::Newline => {
                let token_span = make_span(offset, raw.len);
                let flags = finalize_flags(pending_flags);
                result.push_with_flags(Token::new(TokenKind::Newline, token_span), flags);
                // After a newline, the next token is at line start
                pending_flags =
                    TokenFlags::from_bits(TokenFlags::NEWLINE_BEFORE | TokenFlags::LINE_START);
            }

            // Cook everything else
            _ => {
                let token_span = make_span(offset, raw.len);
                let kind = cooker.cook(raw.tag, offset, raw.len);
                let mut flags = finalize_flags(pending_flags);
                if cooker.last_cook_had_error() {
                    flags.set(TokenFlags::HAS_ERROR);
                }
                if cooker.last_cook_was_contextual_kw() {
                    flags.set(TokenFlags::CONTEXTUAL_KW);
                }
                result.push_with_flags(Token::new(kind, token_span), flags);
                pending_flags = TokenFlags::EMPTY;
            }
        }

        offset += raw.len;
    }

    // Add EOF token
    let eof_pos = u32::try_from(source.len()).unwrap_or_else(|_| {
        let error_span = Span::new(u32::MAX - 1, u32::MAX);
        result.push(Token::new(TokenKind::Error, error_span));
        u32::MAX
    });
    let eof_span = Span::point(eof_pos);
    let eof_flags = finalize_flags(pending_flags);
    result.push_with_flags(Token::new(TokenKind::Eof, eof_span), eof_flags);

    result
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
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    let buf = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut cooker = TokenCooker::new(buf.as_bytes(), interner);
    let mut output = LexOutput::with_capacity(source.len());
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

    // Wire accumulated cooker errors into the output
    output.errors = cooker.into_errors();

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
#[allow(clippy::cast_possible_truncation)]
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

    // === V2 Entry Point Tests ===

    #[test]
    fn test_lex_basic() {
        let interner = StringInterner::new();
        let tokens = lex("let x = 42", &interner);
        // let, x, =, 42, EOF
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
        assert_eq!(tokens[2].kind, TokenKind::Eq);
        assert_eq!(tokens[3].kind, TokenKind::Int(42));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_empty() {
        let interner = StringInterner::new();
        let tokens = lex("", &interner);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_newlines() {
        let interner = StringInterner::new();
        let tokens = lex("a\nb", &interner);
        // a, newline, b, EOF
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
        assert_eq!(tokens[1].kind, TokenKind::Newline);
        assert!(matches!(tokens[2].kind, TokenKind::Ident(_)));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_with_comments() {
        let interner = StringInterner::new();
        // Comments should be skipped in lex (same as lex)
        let tokens = lex("// comment\nlet x = 1", &interner);
        // newline, let, x, =, 1, EOF
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::Newline);
        assert_eq!(tokens[1].kind, TokenKind::Let);
    }

    #[test]
    fn test_lex_with_comments_basic() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// hello\nlet x = 1", &interner);
        assert_eq!(output.comments.len(), 1);
        assert_eq!(output.newlines.len(), 1);
        // newline, let, x, =, 1, EOF
        assert_eq!(output.tokens.len(), 6);
    }

    #[test]
    fn test_lex_with_comments_blank_lines() {
        let interner = StringInterner::new();
        let output = lex_with_comments("a\n\nb", &interner);
        assert_eq!(output.blank_lines.len(), 1);
        assert_eq!(output.blank_lines[0], 2);
    }

    // === Span coverage property ===

    #[test]
    fn spans_cover_source() {
        let interner = StringInterner::new();
        let source = "let x = 42 + 3\n// comment\nfoo()";
        let tokens = lex(source, &interner);

        // Every non-EOF token should have start < end
        for token in tokens.iter() {
            if token.kind != TokenKind::Eof {
                assert!(
                    token.span.start < token.span.end,
                    "zero-width token: {token:?}"
                );
            }
        }

        // EOF span should point to end of source
        let eof = &tokens[tokens.len() - 1];
        assert_eq!(eof.kind, TokenKind::Eof);
        assert_eq!(eof.span.start, source.len() as u32);
    }

    // === ADJACENT flag tests ===

    #[test]
    fn adjacent_flag_on_first_token() {
        // First token in file has no preceding trivia → ADJACENT
        let interner = StringInterner::new();
        let tokens = lex("let", &interner);
        let flags = tokens.flags();
        assert!(flags[0].is_adjacent(), "first token should be ADJACENT");
    }

    #[test]
    fn adjacent_flag_with_space() {
        // "a b" — second token has SPACE_BEFORE, NOT ADJACENT
        let interner = StringInterner::new();
        let tokens = lex("a b", &interner);
        let flags = tokens.flags();
        assert!(flags[0].is_adjacent(), "'a' is first token → ADJACENT");
        assert!(
            !flags[1].is_adjacent(),
            "'b' has space before → not ADJACENT"
        );
        assert!(flags[1].has_space_before());
    }

    #[test]
    fn adjacent_flag_no_space() {
        // "a+b" — all three tokens adjacent
        let interner = StringInterner::new();
        let tokens = lex("a+b", &interner);
        let flags = tokens.flags();
        assert!(flags[0].is_adjacent(), "'a' is first → ADJACENT");
        assert!(flags[1].is_adjacent(), "'+' no space → ADJACENT");
        assert!(flags[2].is_adjacent(), "'b' no space → ADJACENT");
    }

    #[test]
    fn adjacent_flag_after_newline() {
        // "a\nb" — 'b' has NEWLINE_BEFORE, not ADJACENT
        let interner = StringInterner::new();
        let tokens = lex("a\nb", &interner);
        let flags = tokens.flags();
        // tokens: a, newline, b, EOF
        assert!(flags[0].is_adjacent());
        // newline token itself has no preceding trivia (follows 'a' directly)
        // 'b' has NEWLINE_BEFORE | LINE_START, not ADJACENT
        assert!(!flags[2].is_adjacent());
        assert!(flags[2].has_newline_before());
    }

    #[test]
    fn adjacent_flag_after_comment() {
        // "a// comment\nb" — 'b' has TRIVIA_BEFORE + NEWLINE_BEFORE
        let interner = StringInterner::new();
        let tokens = lex("a// comment\nb", &interner);
        let flags = tokens.flags();
        assert!(flags[0].is_adjacent());
        // tokens: a (0), newline (1), b (2), EOF (3)
        // 'b' at index 2 has NEWLINE_BEFORE from the newline, not ADJACENT
        assert!(!flags[2].is_adjacent());
    }

    #[test]
    fn adjacent_mutual_exclusion_with_space() {
        // ADJACENT and SPACE_BEFORE should be mutually exclusive
        let interner = StringInterner::new();
        let tokens = lex("a b", &interner);
        let flags = tokens.flags();
        // 'a' — adjacent, no space
        assert!(flags[0].is_adjacent());
        assert!(!flags[0].has_space_before());
        // 'b' — space, not adjacent
        assert!(!flags[1].is_adjacent());
        assert!(flags[1].has_space_before());
    }

    // === HAS_ERROR flag tests ===

    #[test]
    fn has_error_on_invalid_byte() {
        let interner = StringInterner::new();
        let tokens = lex("\x01", &interner);
        let flags = tokens.flags();
        assert!(flags[0].has_error(), "invalid byte should have HAS_ERROR");
    }

    #[test]
    fn has_error_on_integer_overflow() {
        let interner = StringInterner::new();
        let tokens = lex("99999999999999999999999", &interner);
        let flags = tokens.flags();
        assert!(flags[0].has_error(), "overflow int should have HAS_ERROR");
    }

    #[test]
    fn no_error_on_valid_token() {
        let interner = StringInterner::new();
        let tokens = lex("let x = 42", &interner);
        let flags = tokens.flags();
        for (i, f) in flags.iter().enumerate() {
            assert!(!f.has_error(), "token {i} should not have HAS_ERROR");
        }
    }

    #[test]
    fn has_error_on_semicolon() {
        let interner = StringInterner::new();
        let tokens = lex(";", &interner);
        let flags = tokens.flags();
        assert!(flags[0].has_error(), "semicolon should have HAS_ERROR");
    }

    // === CONTEXTUAL_KW flag tests ===

    #[test]
    fn contextual_kw_on_soft_keyword_with_paren() {
        // "cache (x)" — 'cache' is a soft keyword with ( → CONTEXTUAL_KW
        let interner = StringInterner::new();
        let tokens = lex("cache (x)", &interner);
        let flags = tokens.flags();
        assert_eq!(tokens[0].kind, TokenKind::Cache);
        assert!(
            flags[0].is_contextual_kw(),
            "cache followed by ( should have CONTEXTUAL_KW"
        );
    }

    #[test]
    fn no_contextual_kw_on_reserved_keyword() {
        // "if (x)" — 'if' is a reserved keyword, NOT contextual
        let interner = StringInterner::new();
        let tokens = lex("if (x)", &interner);
        let flags = tokens.flags();
        assert_eq!(tokens[0].kind, TokenKind::If);
        assert!(
            !flags[0].is_contextual_kw(),
            "reserved keyword should not have CONTEXTUAL_KW"
        );
    }

    #[test]
    fn no_contextual_kw_on_identifier() {
        // "cache = 42" — 'cache' without ( is an identifier, no flag
        let interner = StringInterner::new();
        let tokens = lex("cache = 42", &interner);
        let flags = tokens.flags();
        assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
        assert!(
            !flags[0].is_contextual_kw(),
            "identifier should not have CONTEXTUAL_KW"
        );
    }

    #[test]
    fn contextual_kw_on_all_soft_keywords() {
        let interner = StringInterner::new();
        for kw in &["cache", "catch", "parallel", "spawn", "recurse", "timeout"] {
            let source = format!("{kw}(x)");
            let tokens = lex(&source, &interner);
            let flags = tokens.flags();
            assert!(
                flags[0].is_contextual_kw(),
                "{kw}(x) should have CONTEXTUAL_KW"
            );
        }
    }

    // === Reserved-future keyword tests ===

    #[test]
    fn reserved_future_keyword_produces_error() {
        let interner = StringInterner::new();
        let output = lex_with_comments("let static = 1", &interner);
        assert!(
            output.has_errors(),
            "reserved-future keyword should produce error"
        );
        // 'static' is token index 1 (no whitespace tokens — they become flags)
        assert!(matches!(output.tokens[1].kind, TokenKind::Ident(_)));
        // HAS_ERROR should be set on the 'static' token
        assert!(output.tokens.flags()[1].has_error());
    }

    #[test]
    fn all_reserved_future_keywords_produce_errors() {
        let interner = StringInterner::new();
        for kw in &["asm", "inline", "static", "union", "view"] {
            let output = lex_with_comments(kw, &interner);
            assert!(
                output.has_errors(),
                "`{kw}` should produce a reserved-future keyword error"
            );
            assert!(
                matches!(output.tokens[0].kind, TokenKind::Ident(_)),
                "`{kw}` should still lex as identifier"
            );
        }
    }

    // === IS_DOC flag tests ===

    #[test]
    fn is_doc_on_token_after_description() {
        // "// #Desc\ndef" — 'def' should have IS_DOC
        let interner = StringInterner::new();
        let output = lex_with_comments("// #Description\ndef", &interner);
        let flags = output.tokens.flags();
        // tokens: newline, def, EOF
        assert_eq!(output.tokens[1].kind, TokenKind::Def);
        assert!(
            flags[1].is_doc(),
            "'def' after doc description should have IS_DOC"
        );
    }

    #[test]
    fn is_doc_on_token_after_member() {
        // "// * x: val\ndef" — 'def' should have IS_DOC
        let interner = StringInterner::new();
        let output = lex_with_comments("// * x: value\ndef", &interner);
        let flags = output.tokens.flags();
        assert_eq!(output.tokens[1].kind, TokenKind::Def);
        assert!(
            flags[1].is_doc(),
            "'def' after doc member should have IS_DOC"
        );
    }

    #[test]
    fn is_doc_on_token_after_warning() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// !Panics\ndef", &interner);
        let flags = output.tokens.flags();
        assert_eq!(output.tokens[1].kind, TokenKind::Def);
        assert!(
            flags[1].is_doc(),
            "'def' after doc warning should have IS_DOC"
        );
    }

    #[test]
    fn is_doc_on_token_after_example() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// >foo()\ndef", &interner);
        let flags = output.tokens.flags();
        assert_eq!(output.tokens[1].kind, TokenKind::Def);
        assert!(
            flags[1].is_doc(),
            "'def' after doc example should have IS_DOC"
        );
    }

    #[test]
    fn no_is_doc_after_regular_comment() {
        // "// regular\ndef" — 'def' should NOT have IS_DOC
        let interner = StringInterner::new();
        let output = lex_with_comments("// regular comment\ndef", &interner);
        let flags = output.tokens.flags();
        assert_eq!(output.tokens[1].kind, TokenKind::Def);
        assert!(
            !flags[1].is_doc(),
            "'def' after regular comment should not have IS_DOC"
        );
    }

    #[test]
    fn is_doc_with_multiple_doc_comments() {
        // Multiple doc comments before a declaration
        let interner = StringInterner::new();
        let output = lex_with_comments("// #Description\n// * x: value\ndef", &interner);
        let flags = output.tokens.flags();
        // tokens: newline, newline, def, EOF
        assert_eq!(output.tokens[2].kind, TokenKind::Def);
        assert!(
            flags[2].is_doc(),
            "'def' after multiple doc comments should have IS_DOC"
        );
    }

    #[test]
    fn no_is_doc_on_newline_token() {
        // IS_DOC should not be set on the newline between doc comment and def
        let interner = StringInterner::new();
        let output = lex_with_comments("// #Description\ndef", &interner);
        let flags = output.tokens.flags();
        // tokens: newline(0), def(1), EOF(2)
        assert_eq!(output.tokens[0].kind, TokenKind::Newline);
        assert!(!flags[0].is_doc(), "newline should not have IS_DOC");
    }

    #[test]
    fn is_doc_on_non_declaration_token() {
        // IS_DOC is positional — set even before non-declaration tokens
        let interner = StringInterner::new();
        let output = lex_with_comments("// #Description\nlet", &interner);
        let flags = output.tokens.flags();
        assert_eq!(output.tokens[1].kind, TokenKind::Let);
        assert!(
            flags[1].is_doc(),
            "'let' after doc comment should have IS_DOC"
        );
    }

    #[test]
    fn no_is_doc_without_comment() {
        // No comments at all — no IS_DOC
        let interner = StringInterner::new();
        let output = lex_with_comments("def foo", &interner);
        let flags = output.tokens.flags();
        assert!(
            !flags[0].is_doc(),
            "'def' without preceding doc should not have IS_DOC"
        );
    }

    #[test]
    fn is_doc_not_set_in_simple_lex() {
        // The fast lex() path does not classify comments — IS_DOC never set
        let interner = StringInterner::new();
        let tokens = lex("// #Description\ndef", &interner);
        let flags = tokens.flags();
        // tokens: newline, def, EOF
        assert!(
            !flags[1].is_doc(),
            "lex() should not set IS_DOC (no comment classification)"
        );
    }

    // === LexOutput Salsa Trait Tests ===

    #[test]
    fn lex_output_equality() {
        let interner = StringInterner::new();
        let a = lex_with_comments("let x = 42", &interner);
        let b = lex_with_comments("let x = 42", &interner);
        assert_eq!(a, b);
    }

    #[test]
    fn lex_output_inequality() {
        let interner = StringInterner::new();
        let a = lex_with_comments("let x = 42", &interner);
        let b = lex_with_comments("let y = 42", &interner);
        assert_ne!(a, b);
    }

    #[test]
    fn lex_output_hashset_insertion() {
        use std::collections::HashSet;
        let interner = StringInterner::new();

        let a = lex_with_comments("let x = 1", &interner);
        let b = lex_with_comments("let x = 1", &interner);
        let c = lex_with_comments("let y = 2", &interner);

        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b); // duplicate
        set.insert(c);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn lex_output_debug_format() {
        let interner = StringInterner::new();
        let output = lex_with_comments("// comment\nlet x = 42", &interner);
        let debug = format!("{output:?}");
        assert!(debug.contains("LexOutput"));
        assert!(debug.contains("tokens"));
        assert!(debug.contains("comments"));
    }
}
