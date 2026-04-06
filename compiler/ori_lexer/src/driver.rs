//! Lexer driver: the main tokenization loop.
//!
//! Contains the unified `lex_driver()` function and its helpers.
//! The public entry points (`lex`, `lex_full`, `lex_with_comments`)
//! are thin wrappers in `lib.rs`.

use ori_ir::{Comment, CommentKind, Span, StringInterner, Token, TokenFlags, TokenKind};
use ori_lexer_core::{EncodingIssueKind, RawScanner, RawTag, SourceBuffer};
use tracing::{debug, trace};

use crate::comments::classify_and_normalize_comment;
use crate::cooker::TokenCooker;
use crate::lex_error::{DetachedDocWarning, DocMarker, LexError};
use crate::output::LexOutput;
use crate::trivial::try_trivial;

/// Unified lexer driver parameterized on metadata collection.
///
/// When `WITH_METADATA` is `true`, collects comments, newline/blank-line
/// positions, and doc comment tracking (`IS_DOC` flag, detached doc warnings).
/// When `false`, only produces tokens + errors — the fast path for parsing.
///
/// Uses const generics so LLVM monomorphizes two versions from the same source,
/// preserving optimization context (inlining decisions, register allocation)
/// while eliminating dead metadata branches at compile time.
#[expect(
    clippy::too_many_lines,
    reason = "lexer main loop with token classification"
)]
#[expect(
    clippy::cognitive_complexity,
    reason = "lexer driver: inherent complexity from token classification + metadata branches"
)]
pub(crate) fn lex_driver<const WITH_METADATA: bool>(
    source: &str,
    interner: &StringInterner,
) -> LexOutput {
    debug!(
        source_len = source.len(),
        with_metadata = WITH_METADATA,
        "lexing started"
    );

    let buf = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut cooker = TokenCooker::new(buf.as_bytes(), interner);
    let mut output = if WITH_METADATA {
        LexOutput::with_capacity(source.len())
    } else {
        // Non-metadata path: only tokens need capacity; metadata vecs stay empty.
        LexOutput::with_token_capacity(source.len())
    };

    // Convert encoding issues detected by SourceBuffer into LexErrors.
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
    let mut pending_flags = TokenFlags::EMPTY;

    // Metadata-only state (dead code eliminated when WITH_METADATA=false)
    let mut last_significant_was_newline = false;
    let mut pending_doc: Option<(Span, DocMarker)> = None;
    let mut had_blank_line_since_doc = false;
    let mut pending_is_doc = false;

    loop {
        let raw = scanner.next_token();

        if raw.tag == RawTag::Eof {
            break;
        }

        let token_span = crate::cooker::span(offset, raw.len);

        match raw.tag {
            RawTag::Whitespace => {
                pending_flags.set(TokenFlags::SPACE_BEFORE);
            }

            RawTag::LineComment => {
                if WITH_METADATA {
                    let slice = &source[offset as usize..(offset + raw.len) as usize];
                    let content_str = if slice.len() > 2 { &slice[2..] } else { "" };
                    let (kind, normalized) = classify_and_normalize_comment(content_str);
                    let content = interner.intern(&normalized);
                    output
                        .comments
                        .push(Comment::new(content, token_span, kind));

                    if kind.is_doc() {
                        let marker = doc_comment_marker(kind);
                        if pending_doc.is_some() && had_blank_line_since_doc {
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

                    last_significant_was_newline = false;
                }
                pending_flags.set(TokenFlags::TRIVIA_BEFORE);
            }

            RawTag::Newline => {
                if WITH_METADATA {
                    output.newlines.push(token_span.start);

                    if last_significant_was_newline {
                        output.blank_lines.push(token_span.start);
                        if pending_doc.is_some() {
                            had_blank_line_since_doc = true;
                        }
                    }
                }

                let flags = finalize_flags(pending_flags);
                output
                    .tokens
                    .push_with_flags(Token::new(TokenKind::Newline, token_span), flags);
                pending_flags =
                    TokenFlags::from_bits(TokenFlags::NEWLINE_BEFORE | TokenFlags::LINE_START);

                if WITH_METADATA {
                    last_significant_was_newline = true;
                }
            }

            RawTag::InteriorNull => {}

            // Cook everything else
            _ => {
                if WITH_METADATA {
                    last_significant_was_newline = false;
                }

                // Try the trivial fast path: operators and delimiters that
                // map 1:1 from RawTag to TokenKind with no data or side effects.
                // Bypasses cook() and discriminant_index() entirely.
                let (kind, tag, had_error, was_contextual) =
                    if let Some((kind, tag)) = try_trivial(raw.tag) {
                        (kind, tag, false, false)
                    } else {
                        let result = cooker.cook(raw.tag, offset, raw.len);
                        trace!(offset, raw_tag = ?raw.tag, kind = ?result.kind, "cooked token");
                        (
                            result.kind,
                            result.tag,
                            result.had_error,
                            result.contextual_kw,
                        )
                    };

                if WITH_METADATA {
                    if let Some((doc_span, doc_marker)) = pending_doc.take() {
                        if had_blank_line_since_doc || !is_declaration_start(&kind) {
                            output.warnings.push(DetachedDocWarning {
                                span: doc_span,
                                marker: doc_marker,
                            });
                        }
                    }
                }

                let mut flags = finalize_flags(pending_flags);
                if had_error {
                    flags.set(TokenFlags::HAS_ERROR);
                }
                if was_contextual {
                    flags.set(TokenFlags::CONTEXTUAL_KW);
                }
                if WITH_METADATA && pending_is_doc {
                    flags.set(TokenFlags::IS_DOC);
                    pending_is_doc = false;
                }
                output
                    .tokens
                    .push_with_tag(Token::new(kind, token_span), tag, flags);
                pending_flags = TokenFlags::EMPTY;

                // Track last non-trivia raw tag for O(1) method-position detection.
                cooker.set_last_non_trivia(raw.tag);
            }
        }

        offset += raw.len;
    }

    // If a doc comment is still pending at EOF, it's detached
    if WITH_METADATA {
        if let Some((doc_span, doc_marker)) = pending_doc {
            output.warnings.push(DetachedDocWarning {
                span: doc_span,
                marker: doc_marker,
            });
        }
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

    output.errors.extend(cooker.into_errors());

    debug!(
        tokens = output.tokens.len(),
        errors = output.errors.len(),
        "lexing complete"
    );

    output
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
fn doc_comment_marker(kind: CommentKind) -> DocMarker {
    match kind {
        CommentKind::DocDescription => DocMarker::Description,
        CommentKind::DocMember => DocMarker::Member,
        CommentKind::DocWarning => DocMarker::Warning,
        CommentKind::DocExample => DocMarker::Example,
        CommentKind::Regular => DocMarker::Plain,
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
