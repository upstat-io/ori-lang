use super::*;

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

// ─────────────────────────────────────────────────────────────────────────
// TokenFlags tests
// ─────────────────────────────────────────────────────────────────────────

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
