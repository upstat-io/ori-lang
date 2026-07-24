//! Lexer driver: the main tokenization loop.
//!
//! Contains the unified `lex_driver()` function and its helpers.
//! The public entry points (`lex`, `lex_full`, `lex_with_comments`)
//! are thin wrappers in `lib.rs`.

use ori_ir::{Comment, CommentKind, Span, StringInterner, Token, TokenFlags, TokenKind};
use ori_lexer_core::{EncodingIssueKind, RawScanner, RawTag, RawToken, SourceBuffer};
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
    let mut cooker = TokenCooker::new(source, interner);
    let mut output = if WITH_METADATA {
        LexOutput::with_capacity(source.len())
    } else {
        // Non-metadata path: only tokens need capacity; metadata vecs stay empty.
        LexOutput::with_token_capacity(source.len())
    };

    record_encoding_issues(&buf, &mut output);

    let mut offset: u32 = 0;
    let mut pending_flags = TokenFlags::EMPTY;

    let mut metadata = MetadataState::default();

    loop {
        let raw = scanner.next_token();

        if raw.tag == RawTag::Eof {
            break;
        }

        process_raw_token::<WITH_METADATA>(
            raw,
            offset,
            &mut DriverState {
                source,
                interner,
                cooker: &mut cooker,
                output: &mut output,
                pending_flags: &mut pending_flags,
                metadata: &mut metadata,
            },
        );

        offset += raw.len;
    }

    if WITH_METADATA {
        if let Some((doc_span, doc_marker)) = metadata.pending_doc {
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

#[derive(Default)]
struct MetadataState {
    last_significant_was_newline: bool,
    pending_doc: Option<(Span, DocMarker)>,
    had_blank_line_since_doc: bool,
    pending_is_doc: bool,
}

struct DriverState<'state, 'src> {
    source: &'src str,
    interner: &'src StringInterner,
    cooker: &'state mut TokenCooker<'src>,
    output: &'state mut LexOutput,
    pending_flags: &'state mut TokenFlags,
    metadata: &'state mut MetadataState,
}

fn record_encoding_issues(buffer: &SourceBuffer, output: &mut LexOutput) {
    for issue in buffer.encoding_issues() {
        let span = Span::new(issue.pos, issue.pos + issue.len);
        output.errors.push(match issue.kind {
            EncodingIssueKind::Utf8Bom => LexError::utf8_bom(span),
            EncodingIssueKind::Utf16LeBom => LexError::utf16_le_bom(span),
            EncodingIssueKind::Utf16BeBom => LexError::utf16_be_bom(span),
            EncodingIssueKind::InteriorNull => LexError::interior_null(span),
        });
    }
}

fn process_raw_token<const WITH_METADATA: bool>(
    raw: RawToken,
    offset: u32,
    state: &mut DriverState<'_, '_>,
) {
    match raw.tag {
        RawTag::Whitespace => state.pending_flags.set(TokenFlags::SPACE_BEFORE),
        RawTag::LineComment => process_line_comment::<WITH_METADATA>(raw, offset, state),
        RawTag::Newline => process_newline::<WITH_METADATA>(raw, offset, state),
        RawTag::InteriorNull => {}
        _ => process_cooked_token::<WITH_METADATA>(raw, offset, state),
    }
}

fn process_line_comment<const WITH_METADATA: bool>(
    raw: RawToken,
    offset: u32,
    state: &mut DriverState<'_, '_>,
) {
    if WITH_METADATA {
        let token_span = crate::cooker::span(offset, raw.len);
        let slice = &state.source[offset as usize..(offset + raw.len) as usize];
        let content_str = slice.get(2..).unwrap_or_default();
        let (kind, normalized) = classify_and_normalize_comment(content_str);
        let content = state.interner.intern(&normalized);
        state
            .output
            .comments
            .push(Comment::new(content, token_span, kind));

        if kind.is_doc() {
            if state.metadata.had_blank_line_since_doc {
                emit_detached_doc_warning(state);
            }
            state.metadata.pending_doc = Some((token_span, doc_comment_marker(kind)));
            state.metadata.had_blank_line_since_doc = false;
            state.metadata.pending_is_doc = true;
        }
        state.metadata.last_significant_was_newline = false;
    }
    state.pending_flags.set(TokenFlags::TRIVIA_BEFORE);
}

fn process_newline<const WITH_METADATA: bool>(
    raw: RawToken,
    offset: u32,
    state: &mut DriverState<'_, '_>,
) {
    let token_span = crate::cooker::span(offset, raw.len);
    if WITH_METADATA {
        state.output.newlines.push(token_span.start);
        if state.metadata.last_significant_was_newline {
            state.output.blank_lines.push(token_span.start);
            state.metadata.had_blank_line_since_doc |= state.metadata.pending_doc.is_some();
        }
    }

    let flags = finalize_flags(*state.pending_flags);
    state
        .output
        .tokens
        .push_with_flags(Token::new(TokenKind::Newline, token_span), flags);
    *state.pending_flags =
        TokenFlags::from_bits(TokenFlags::NEWLINE_BEFORE | TokenFlags::LINE_START);
    state.metadata.last_significant_was_newline = WITH_METADATA;
}

fn process_cooked_token<const WITH_METADATA: bool>(
    raw: RawToken,
    offset: u32,
    state: &mut DriverState<'_, '_>,
) {
    let token_span = crate::cooker::span(offset, raw.len);
    state.metadata.last_significant_was_newline = false;

    let result = if let Some((kind, tag)) = try_trivial(raw.tag) {
        crate::cooker::CookResult::trivial(kind, tag)
    } else {
        let result = state.cooker.cook(raw.tag, offset, raw.len);
        trace!(offset, raw_tag = ?raw.tag, kind = ?result.kind, "cooked token");
        result
    };
    let had_error = result.had_error();
    let was_contextual = result.is_contextual_keyword();
    let kind = result.kind;
    let tag = result.tag;

    if WITH_METADATA
        && state.metadata.pending_doc.is_some()
        && (state.metadata.had_blank_line_since_doc || !is_declaration_start(&kind))
    {
        emit_detached_doc_warning(state);
    } else if WITH_METADATA {
        state.metadata.pending_doc = None;
    }

    let mut flags = finalize_flags(*state.pending_flags);
    if had_error {
        flags.set(TokenFlags::HAS_ERROR);
    }
    if was_contextual {
        flags.set(TokenFlags::CONTEXTUAL_KW);
    }
    if WITH_METADATA && state.metadata.pending_is_doc {
        flags.set(TokenFlags::IS_DOC);
        state.metadata.pending_is_doc = false;
    }
    state
        .output
        .tokens
        .push_with_tag(Token::new(kind, token_span), tag, flags);
    *state.pending_flags = TokenFlags::EMPTY;
    state.cooker.set_last_non_trivia(raw.tag);
}

fn emit_detached_doc_warning(state: &mut DriverState<'_, '_>) {
    if let Some((span, marker)) = state.metadata.pending_doc.take() {
        state
            .output
            .warnings
            .push(DetachedDocWarning { span, marker });
    }
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
