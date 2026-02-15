use super::*;
use ori_diagnostic::ErrorCode;
use ori_ir::TokenKind;

fn make_error() -> ParseError {
    ParseError::new(ErrorCode::E1001, "test error", Span::new(0, 1))
}

#[test]
fn test_consumed_ok() {
    let outcome: ParseOutcome<i32> = ParseOutcome::consumed_ok(42);
    assert!(outcome.is_ok());
    assert!(outcome.made_progress());
    assert!(!outcome.no_progress());
    assert_eq!(outcome.unwrap(), 42);
}

#[test]
fn test_empty_ok() {
    let outcome: ParseOutcome<i32> = ParseOutcome::empty_ok(42);
    assert!(outcome.is_ok());
    assert!(!outcome.made_progress());
    assert!(outcome.no_progress());
    assert_eq!(outcome.unwrap(), 42);
}

#[test]
fn test_consumed_err() {
    let outcome: ParseOutcome<i32> = ParseOutcome::consumed_err(make_error(), Span::new(0, 10));
    assert!(outcome.is_err());
    assert!(outcome.made_progress());
    assert!(outcome.failed_with_progress());
    assert!(!outcome.failed_without_progress());
}

#[test]
fn test_empty_err() {
    let expected = TokenSet::new().with(TokenKind::LParen);
    let outcome: ParseOutcome<i32> = ParseOutcome::empty_err(expected, 5);
    assert!(outcome.is_err());
    assert!(!outcome.made_progress());
    assert!(!outcome.failed_with_progress());
    assert!(outcome.failed_without_progress());
}

#[test]
fn test_map() {
    let outcome = ParseOutcome::consumed_ok(42).map(|x| x * 2);
    assert_eq!(outcome.unwrap(), 84);

    let outcome = ParseOutcome::empty_ok(42).map(|x| x * 2);
    assert_eq!(outcome.unwrap(), 84);
}

#[test]
fn test_and_then_consumed_ok() {
    let outcome = ParseOutcome::consumed_ok(42).and_then(|x| ParseOutcome::consumed_ok(x * 2));
    assert!(outcome.made_progress());
    assert_eq!(outcome.unwrap(), 84);
}

#[test]
fn test_and_then_empty_ok_to_consumed() {
    // Empty ok followed by consumed becomes consumed
    let outcome = ParseOutcome::empty_ok(42).and_then(|x| ParseOutcome::consumed_ok(x * 2));
    assert!(outcome.made_progress());
    assert_eq!(outcome.unwrap(), 84);
}

#[test]
fn test_and_then_consumed_ok_to_empty() {
    // Consumed followed by empty stays consumed
    let outcome = ParseOutcome::consumed_ok(42).and_then(|x| ParseOutcome::empty_ok(x * 2));
    assert!(outcome.made_progress());
    assert_eq!(outcome.unwrap(), 84);
}

#[test]
fn test_or_else_success() {
    let outcome = ParseOutcome::consumed_ok(42).or_else(|| ParseOutcome::consumed_ok(0));
    assert_eq!(outcome.unwrap(), 42);
}

#[test]
fn test_or_else_consumed_err() {
    // Consumed error doesn't try alternative
    let outcome = ParseOutcome::<i32>::consumed_err(make_error(), Span::new(0, 5))
        .or_else(|| ParseOutcome::consumed_ok(0));
    assert!(outcome.failed_with_progress());
}

#[test]
fn test_or_else_empty_err() {
    // Empty error tries alternative
    let expected = TokenSet::new().with(TokenKind::LParen);
    let outcome =
        ParseOutcome::<i32>::empty_err(expected, 0).or_else(|| ParseOutcome::consumed_ok(99));
    assert_eq!(outcome.unwrap(), 99);
}

#[test]
fn test_or_else_accumulate() {
    let expected1 = TokenSet::new().with(TokenKind::LParen);
    let expected2 = TokenSet::new().with(TokenKind::LBracket);

    let outcome = ParseOutcome::<i32>::empty_err(expected1, 0)
        .or_else_accumulate(|| ParseOutcome::empty_err(expected2, 0));

    if let ParseOutcome::EmptyErr { expected, .. } = outcome {
        assert!(expected.contains(&TokenKind::LParen));
        assert!(expected.contains(&TokenKind::LBracket));
    } else {
        panic!("Expected EmptyErr");
    }
}

#[test]
fn test_into_result() {
    let outcome = ParseOutcome::consumed_ok(42);
    assert_eq!(outcome.into_result().unwrap(), 42);

    let outcome = ParseOutcome::<i32>::consumed_err(make_error(), Span::new(0, 1));
    assert!(outcome.into_result().is_err());
}

#[test]
fn test_with_error_context_on_consumed_err() {
    let outcome: ParseOutcome<i32> = ParseOutcome::consumed_err(make_error(), Span::new(0, 5))
        .with_error_context(ErrorContext::IfExpression);

    if let ParseOutcome::ConsumedErr { error, .. } = outcome {
        assert!(error.context.is_some());
        assert!(error.context.unwrap().contains("if expression"));
    } else {
        panic!("Expected ConsumedErr");
    }
}

#[test]
fn test_with_error_context_preserves_success() {
    let outcome = ParseOutcome::consumed_ok(42).with_error_context(ErrorContext::IfExpression);
    assert!(outcome.is_ok());
    assert_eq!(outcome.unwrap(), 42);
}

#[test]
fn test_with_error_context_preserves_empty_err() {
    let expected = TokenSet::new().with(TokenKind::LParen);
    let outcome =
        ParseOutcome::<i32>::empty_err(expected, 5).with_error_context(ErrorContext::IfExpression);

    // EmptyErr should not be modified (context only applies to hard errors)
    if let ParseOutcome::EmptyErr {
        expected: e,
        position,
    } = outcome
    {
        assert_eq!(position, 5);
        assert!(e.contains(&TokenKind::LParen));
    } else {
        panic!("Expected EmptyErr");
    }
}

#[test]
fn test_with_error_context_doesnt_overwrite() {
    let mut error = make_error();
    error.context = Some("existing context".to_string());
    let outcome: ParseOutcome<i32> = ParseOutcome::consumed_err(error, Span::new(0, 5))
        .with_error_context(ErrorContext::IfExpression);

    if let ParseOutcome::ConsumedErr { error, .. } = outcome {
        assert_eq!(error.context, Some("existing context".to_string()));
    } else {
        panic!("Expected ConsumedErr");
    }
}

// === Macro Tests ===
//
// These tests verify the backtracking macros work correctly.
// We use a simple mock parser that tracks position for snapshot/restore.

/// Mock cursor that tracks position, matching the `one_of!` macro's
/// expectation that `$self.cursor.position()` is available.
struct MockCursor {
    position: usize,
}

impl MockCursor {
    fn position(&self) -> usize {
        self.position
    }
}

/// Mock parser for testing macros
struct MockParser {
    cursor: MockCursor,
}

impl MockParser {
    fn new() -> Self {
        Self {
            cursor: MockCursor { position: 0 },
        }
    }

    fn snapshot(&self) -> MockSnapshot {
        MockSnapshot {
            position: self.cursor.position,
        }
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "matches macro API which clones"
    )]
    fn restore(&mut self, snap: MockSnapshot) {
        self.cursor.position = snap.position;
    }

    fn position(&self) -> usize {
        self.cursor.position
    }

    fn advance(&mut self) {
        self.cursor.position += 1;
    }

    /// Parse something that succeeds after consuming
    fn parse_consuming(&mut self) -> ParseOutcome<i32> {
        self.advance();
        ParseOutcome::consumed_ok(42)
    }

    /// Parse something that succeeds without consuming
    #[expect(
        clippy::unused_self,
        reason = "consistent API with other parse methods"
    )]
    fn parse_empty(&mut self) -> ParseOutcome<i32> {
        ParseOutcome::empty_ok(0)
    }

    /// Parse something that fails without consuming (soft error)
    fn parse_soft_fail(&mut self) -> ParseOutcome<i32> {
        ParseOutcome::empty_err(
            TokenSet::new().with(TokenKind::LParen),
            self.cursor.position,
        )
    }

    /// Parse something that fails after consuming (hard error)
    #[expect(clippy::cast_possible_truncation, reason = "test position fits in u32")]
    fn parse_hard_fail(&mut self) -> ParseOutcome<i32> {
        self.advance();
        ParseOutcome::consumed_err(make_error(), Span::new(self.cursor.position as u32, 1))
    }

    /// Parse something that fails with a different expected token
    fn parse_soft_fail_bracket(&mut self) -> ParseOutcome<i32> {
        ParseOutcome::empty_err(
            TokenSet::new().with(TokenKind::LBracket),
            self.cursor.position,
        )
    }
}

#[derive(Clone)]
struct MockSnapshot {
    position: usize,
}

#[test]
fn test_one_of_first_succeeds() {
    let mut parser = MockParser::new();
    let result = one_of!(parser, parser.parse_consuming(), parser.parse_soft_fail(),);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_one_of_empty_ok_succeeds() {
    let mut parser = MockParser::new();
    // EmptyOk should also be accepted
    let result = one_of!(parser, parser.parse_empty(), parser.parse_consuming(),);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // parse_empty returns 0
}

#[test]
fn test_one_of_second_succeeds() {
    let mut parser = MockParser::new();
    // First fails soft, second succeeds
    let result = one_of!(parser, parser.parse_soft_fail(), parser.parse_consuming(),);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_one_of_hard_error_propagates() {
    let mut parser = MockParser::new();
    // First fails hard - should propagate, not try second
    let result = one_of!(parser, parser.parse_hard_fail(), parser.parse_consuming(),);
    assert!(result.failed_with_progress());
}

#[test]
fn test_one_of_all_soft_fail_accumulates() {
    let mut parser = MockParser::new();
    // Both fail soft - should accumulate expected tokens
    let result: ParseOutcome<i32> = one_of!(
        parser,
        parser.parse_soft_fail(),
        parser.parse_soft_fail_bracket(),
    );

    if let ParseOutcome::EmptyErr { expected, .. } = result {
        // Should have both expected tokens
        assert!(expected.contains(&TokenKind::LParen));
        assert!(expected.contains(&TokenKind::LBracket));
    } else {
        panic!("Expected EmptyErr with accumulated tokens");
    }
}

#[test]
fn test_one_of_restores_on_soft_fail() {
    let mut parser = MockParser::new();
    let start_pos = parser.position();

    // This will fail soft, should restore position before trying next
    let _result: ParseOutcome<i32> =
        one_of!(parser, parser.parse_soft_fail(), parser.parse_soft_fail(),);

    // Position should still be at start (restored after soft fails)
    assert_eq!(parser.position(), start_pos);
}

// Helper functions for chain tests (defined outside test functions per clippy)
fn parse_with_chain(p: &mut MockParser) -> ParseOutcome<i32> {
    let a = chain!(p, p.parse_consuming());
    let b = chain!(p, p.parse_consuming());
    ParseOutcome::consumed_ok(a + b)
}

fn parse_with_chain_fail(p: &mut MockParser) -> ParseOutcome<i32> {
    let _a = chain!(p, p.parse_consuming());
    let _b = chain!(p, p.parse_hard_fail()); // Should propagate
    ParseOutcome::consumed_ok(0) // Never reached
}

#[test]
fn test_chain_success() {
    let mut parser = MockParser::new();
    let result = parse_with_chain(&mut parser);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 84); // 42 + 42
}

#[test]
fn test_chain_propagates_error() {
    let mut parser = MockParser::new();
    let result = parse_with_chain_fail(&mut parser);
    assert!(result.failed_with_progress());
}

// === committed! macro tests ===

fn parse_with_committed_ok(_p: &mut MockParser) -> ParseOutcome<i32> {
    let a: i32 = committed!(Ok::<i32, ParseError>(42));
    let b: i32 = committed!(Ok::<i32, ParseError>(10));
    ParseOutcome::consumed_ok(a + b)
}

fn parse_with_committed_err(_p: &mut MockParser) -> ParseOutcome<i32> {
    let _a: i32 = committed!(Ok::<i32, ParseError>(42));
    let _b: i32 = committed!(Err(make_error())); // Should return ConsumedErr
    ParseOutcome::consumed_ok(0) // Never reached
}

#[test]
fn test_committed_ok() {
    let mut parser = MockParser::new();
    let result = parse_with_committed_ok(&mut parser);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 52);
}

#[test]
fn test_committed_err() {
    let mut parser = MockParser::new();
    let result = parse_with_committed_err(&mut parser);
    assert!(result.failed_with_progress());
}
