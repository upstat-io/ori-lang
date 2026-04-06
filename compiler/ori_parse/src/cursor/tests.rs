use super::*;

/// All keywords that are usable as identifiers (the canonical list).
///
/// This array must match `is_keyword_usable_as_ident`. Tests below verify
/// that `soft_keyword_to_name` and `keyword_as_name` together cover exactly
/// this set. Adding a keyword here requires updating the corresponding
/// `*_to_name` method and `is_keyword_usable_as_ident`.
const KEYWORD_AS_IDENT_TOKENS: &[TokenKind] = &[
    // Soft keywords (always-resolved, usable as identifiers)
    TokenKind::Print,
    TokenKind::Panic,
    TokenKind::By,
    TokenKind::Run,
    TokenKind::Try,
    TokenKind::With,
    // Lexer-soft keywords (resolved only with `(` lookahead)
    TokenKind::Cache,
    TokenKind::Catch,
    TokenKind::Parallel,
    TokenKind::Recurse,
    TokenKind::Spawn,
    TokenKind::Timeout,
    // Positional keywords
    TokenKind::Where,
    TokenKind::Match,
    TokenKind::For,
    TokenKind::In,
    TokenKind::If,
    TokenKind::Type,
];

/// Non-identifier keywords that must NOT be accepted as identifiers.
const NON_IDENT_KEYWORDS: &[TokenKind] = &[
    TokenKind::Let,
    TokenKind::Else,
    TokenKind::Then,
    TokenKind::Do,
    TokenKind::Yield,
    TokenKind::Loop,
    TokenKind::Break,
    TokenKind::Continue,
    TokenKind::Return,
    TokenKind::Trait,
    TokenKind::Impl,
    TokenKind::Pub,
    TokenKind::Use,
    TokenKind::Void,
];

/// Owns the token list and interner so `Cursor` can borrow them
/// without `Box::leak`.
struct TestCtx {
    tokens: TokenList,
    interner: StringInterner,
}

impl TestCtx {
    fn new(source: &str) -> Self {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        Self { tokens, interner }
    }

    fn cursor(&self) -> Cursor<'_> {
        Cursor::new(&self.tokens, &self.interner)
    }
}

#[test]
fn test_cursor_navigation() {
    let ctx = TestCtx::new("let x = 42");
    let mut cursor = ctx.cursor();

    assert!(cursor.check(&TokenKind::Let));
    assert!(!cursor.is_at_end());

    cursor.advance();
    assert!(cursor.check_ident());

    cursor.advance();
    assert!(cursor.check(&TokenKind::Eq));

    cursor.advance();
    assert!(matches!(cursor.current_kind(), TokenKind::Int(_)));

    cursor.advance();
    assert!(cursor.is_at_end());
}

#[test]
fn test_expect_success() {
    let ctx = TestCtx::new("let x");
    let mut cursor = ctx.cursor();

    let result = cursor.expect(&TokenKind::Let);
    assert!(result.is_ok());
}

#[test]
fn test_expect_failure() {
    let ctx = TestCtx::new("let x");
    let mut cursor = ctx.cursor();

    let result = cursor.expect(&TokenKind::If);
    assert!(result.is_err());
}

#[test]
fn test_skip_newlines() {
    let ctx = TestCtx::new("let\n\n\nx");
    let mut cursor = ctx.cursor();

    cursor.advance(); // skip 'let'
    cursor.skip_newlines();
    assert!(cursor.check_ident()); // should be at 'x'
}

#[test]
fn test_lookahead() {
    let ctx = TestCtx::new("foo()");
    let cursor = ctx.cursor();

    assert!(cursor.next_is_lparen());
}

#[test]
fn test_check_type_keyword() {
    let ctx = TestCtx::new("int float bool str");
    let mut cursor = ctx.cursor();

    assert!(cursor.check_type_keyword()); // int
    cursor.advance();
    assert!(cursor.check_type_keyword()); // float
    cursor.advance();
    assert!(cursor.check_type_keyword()); // bool
    cursor.advance();
    assert!(cursor.check_type_keyword()); // str
}

#[test]
fn test_token_capture() {
    let ctx = TestCtx::new("let x = 42");
    let mut cursor = ctx.cursor();

    // Capture range covering "let x ="
    let start = cursor.start_capture();
    cursor.advance(); // let
    cursor.advance(); // x
    cursor.advance(); // =
    let capture = cursor.complete_capture(start);

    assert!(!capture.is_empty());
    assert_eq!(capture.len(), 3);

    // Verify the captured tokens
    let captured = cursor.tokens().get_range(capture);
    assert_eq!(captured.len(), 3);
    assert!(matches!(captured[0].kind, TokenKind::Let));
    assert!(matches!(captured[1].kind, TokenKind::Ident(_)));
    assert!(matches!(captured[2].kind, TokenKind::Eq));
}

#[test]
fn test_token_capture_empty() {
    let ctx = TestCtx::new("let");
    let cursor = ctx.cursor();

    // Capture with no advancement
    let start = cursor.start_capture();
    let capture = cursor.complete_capture(start);

    assert!(capture.is_empty());
    assert_eq!(capture.len(), 0);
}

// TokenFlags tests

#[test]
fn test_newline_before_flag() {
    // "let\nx" -> tokens: [let, \n, x, EOF]
    let ctx = TestCtx::new("let\nx");
    let mut cursor = ctx.cursor();

    // `let` is the first token — no newline before it
    assert!(!cursor.has_newline_before());
    cursor.advance(); // skip `let`
    cursor.skip_newlines();

    // `x` follows a newline — NEWLINE_BEFORE should be set
    assert!(cursor.check_ident());
    assert!(cursor.has_newline_before());
}

#[test]
fn test_no_newline_on_same_line() {
    // "let x" -> tokens: [let, x, EOF]
    let ctx = TestCtx::new("let x");
    let mut cursor = ctx.cursor();

    // `let` — no newline before
    assert!(!cursor.has_newline_before());
    cursor.advance();

    // `x` — still no newline, just a space
    assert!(!cursor.has_newline_before());
}

#[test]
fn test_line_start_flag() {
    // "let\nx" -> tokens: [let, \n, x, EOF]
    let ctx = TestCtx::new("let\nx");
    let mut cursor = ctx.cursor();

    cursor.advance(); // skip `let`
    cursor.skip_newlines();

    // `x` is the first non-trivia token on its line — LINE_START set
    assert!(cursor.check_ident());
    assert!(cursor.at_line_start());
}

#[test]
fn test_no_line_start_mid_line() {
    // "let x = 42" -> all on same line
    let ctx = TestCtx::new("let x = 42");
    let mut cursor = ctx.cursor();

    cursor.advance(); // skip `let`

    // `x` is NOT at line start — it's mid-line
    assert!(!cursor.at_line_start());
}

#[test]
fn test_current_flags_returns_correct_value() {
    // "let   x" -> tokens: [let, x, EOF]
    let ctx = TestCtx::new("let   x");
    let mut cursor = ctx.cursor();

    cursor.advance(); // skip `let`

    // `x` is preceded by spaces — SPACE_BEFORE should be set
    let flags = cursor.current_flags();
    assert!(flags.has_space_before());
    assert!(!flags.has_newline_before());
}

#[test]
fn test_multiple_newlines_flag() {
    // "a\n\n\nb" -> tokens: [a, \n, \n, \n, b, EOF]
    let ctx = TestCtx::new("a\n\n\nb");
    let mut cursor = ctx.cursor();

    cursor.advance(); // skip `a`
    cursor.skip_newlines();

    // `b` follows multiple newlines
    assert!(cursor.check_ident());
    assert!(cursor.has_newline_before());
    assert!(cursor.at_line_start());
}

#[test]
fn test_eof_flags() {
    // "let\n" -> tokens: [let, \n, EOF]
    let ctx = TestCtx::new("let\n");
    let mut cursor = ctx.cursor();

    cursor.advance(); // skip `let`
    cursor.skip_newlines();

    // EOF follows a newline
    assert!(cursor.is_at_end());
    assert!(cursor.has_newline_before());
}

// Keyword-as-identifier consistency

#[test]
fn keyword_as_ident_consistency_positive() {
    // Every token in the canonical list must be accepted by is_keyword_usable_as_ident.
    for kind in KEYWORD_AS_IDENT_TOKENS {
        assert!(
            is_keyword_usable_as_ident(kind),
            "{kind:?} should be usable as ident but is_keyword_usable_as_ident returned false"
        );
    }
}

#[test]
fn keyword_as_ident_consistency_negative() {
    // Non-identifier keywords must NOT be accepted.
    for kind in NON_IDENT_KEYWORDS {
        assert!(
            !is_keyword_usable_as_ident(kind),
            "{kind:?} should NOT be usable as ident but is_keyword_usable_as_ident returned true"
        );
    }
}

#[test]
fn soft_keyword_covers_canonical_subset() {
    // soft_keyword_to_name must accept exactly the soft-keyword subset.
    // First 6: always-resolved soft keywords. Next 6: lexer-soft keywords
    // (only resolved when `(` follows, so we test them individually).
    let always_resolved = &KEYWORD_AS_IDENT_TOKENS[..6]; // Print, Panic, By, Run, Try, With
    let ctx = TestCtx::new("print panic by run try with");
    let mut cursor = ctx.cursor();
    for expected in always_resolved {
        assert!(
            cursor.soft_keyword_to_name().is_some(),
            "soft_keyword_to_name should accept {expected:?}"
        );
        cursor.advance();
    }

    // Lexer-soft keywords: these are only tokenized as keywords when `(`
    // follows, so we test each with `(` lookahead to produce the keyword token.
    let lexer_soft_sources = [
        ("cache()", TokenKind::Cache),
        ("catch()", TokenKind::Catch),
        ("parallel()", TokenKind::Parallel),
        ("recurse()", TokenKind::Recurse),
        ("spawn()", TokenKind::Spawn),
        ("timeout()", TokenKind::Timeout),
    ];
    for (source, expected_kind) in &lexer_soft_sources {
        let ctx = TestCtx::new(source);
        let cursor = ctx.cursor();
        assert_eq!(
            cursor.current_kind(),
            expected_kind,
            "lexer should resolve {source} as {expected_kind:?}"
        );
        assert!(
            cursor.soft_keyword_to_name().is_some(),
            "soft_keyword_to_name should accept {expected_kind:?}"
        );
    }
}

/// Regression: hygiene-full-2 §06.4b — `expect_member_name()` used to reuse
/// `make_expect_ident_error()`, producing "expected identifier" when it should
/// say "expected member name" (member position also accepts keywords and ints).
#[expect(
    clippy::unwrap_used,
    reason = "test code: unwrap_err on known-Err result"
)]
#[test]
fn expect_member_name_error_says_member_name() {
    // "foo.!" — after advancing past `foo` and `.`, `!` triggers the error.
    let ctx = TestCtx::new("foo.!");
    let mut cursor = ctx.cursor();
    cursor.advance(); // skip `foo`
    cursor.advance(); // skip `.`

    let result = cursor.expect_member_name();
    assert!(
        result.is_err(),
        "expect_member_name should fail on `!` token"
    );
    let msg = result.unwrap_err().message().to_owned();
    // Positive pin: the new wording is present
    assert!(
        msg.contains("expected member name"),
        "error should say 'expected member name', got: {msg}"
    );
    // Negative forbid-output pin: the old wording is gone
    assert!(
        !msg.contains("expected identifier"),
        "error should NOT say 'expected identifier', got: {msg}"
    );
}

// Direct acceptance matrix for the three expect_* public APIs

#[test]
fn expect_ident_accepts_ident_and_soft_keyword() {
    // Regular identifier
    let ctx = TestCtx::new("foo");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident().is_ok());

    // Soft keyword used as identifier
    let ctx = TestCtx::new("print");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident().is_ok());
}

#[test]
fn expect_ident_rejects_reserved_keyword_and_int() {
    // Reserved keyword
    let ctx = TestCtx::new("let");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident().is_err());

    // Integer literal
    let ctx = TestCtx::new("42");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident().is_err());
}

#[test]
fn expect_member_name_accepts_keyword_and_int() {
    // Regular identifier
    let ctx = TestCtx::new("foo.bar");
    let mut cursor = ctx.cursor();
    cursor.advance(); // skip `foo`
    cursor.advance(); // skip `.`
    assert!(cursor.expect_member_name().is_ok());

    // Soft keyword in member position
    let ctx = TestCtx::new("foo.print");
    let mut cursor = ctx.cursor();
    cursor.advance();
    cursor.advance();
    assert!(cursor.expect_member_name().is_ok());

    // Reserved keyword in member position (e.g. ordering.then)
    let ctx = TestCtx::new("x.then");
    let mut cursor = ctx.cursor();
    cursor.advance();
    cursor.advance();
    assert!(cursor.expect_member_name().is_ok());

    // Integer tuple field access (t.0)
    let ctx = TestCtx::new("t.0");
    let mut cursor = ctx.cursor();
    cursor.advance();
    cursor.advance();
    assert!(cursor.expect_member_name().is_ok());
}

#[test]
fn expect_ident_or_keyword_accepts_positional_keywords() {
    // Regular identifier
    let ctx = TestCtx::new("foo");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_ok());

    // Soft keyword
    let ctx = TestCtx::new("print");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_ok());

    // Positional keyword (where)
    let ctx = TestCtx::new("where");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_ok());

    // Positional keyword (match)
    let ctx = TestCtx::new("match");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_ok());
}

#[test]
fn expect_ident_or_keyword_rejects_non_positional_and_int() {
    // Non-positional reserved keyword (let)
    let ctx = TestCtx::new("let");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_err());

    // Non-positional reserved keyword (else)
    let ctx = TestCtx::new("else");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_err());

    // Integer literal
    let ctx = TestCtx::new("42");
    let mut cursor = ctx.cursor();
    assert!(cursor.expect_ident_or_keyword().is_err());
}

#[test]
fn keyword_as_name_covers_canonical_subset() {
    // keyword_as_name must accept exactly the positional-keyword subset.
    let positional = &KEYWORD_AS_IDENT_TOKENS[12..]; // Where, Match, For, In, If, Type
    let ctx = TestCtx::new("where match for in if type");
    let mut cursor = ctx.cursor();
    for expected in positional {
        assert!(
            cursor.keyword_as_name().is_some(),
            "keyword_as_name should accept {expected:?}"
        );
        cursor.advance();
    }
}
