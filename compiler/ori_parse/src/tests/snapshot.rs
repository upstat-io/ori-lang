//! Tests for parser snapshot functionality.
//!
//! Verifies that snapshots correctly capture and restore parser state
//! for speculative parsing.
//!
//! # When to Use Snapshots
//!
//! See `snapshot.rs` module documentation for guidance on when to use:
//! - Simple lookahead (1-2 tokens)
//! - `look_ahead()` (complex multi-token patterns)
//! - `try_parse()` (speculative full parsing)
//! - Manual `snapshot()`/`restore()` (complex decision logic)

use crate::{ParseContext, Parser};
use ori_ir::{StringInterner, TokenKind};

/// Helper to create a parser for testing.
fn create_parser(code: &str) -> (Parser<'static>, &'static StringInterner) {
    let interner = Box::leak(Box::new(StringInterner::new()));
    let tokens = ori_lexer::lex(code, interner);
    let tokens = Box::leak(Box::new(tokens));
    (Parser::new(tokens, interner), interner)
}

#[test]
fn test_snapshot_captures_position() {
    let (mut parser, _) = create_parser("let x = 42");

    // Position at start
    let snapshot = parser.snapshot();
    assert_eq!(snapshot.cursor_pos(), 0);

    // Advance and verify position changed
    parser.cursor.advance();
    parser.cursor.advance();
    assert_eq!(parser.cursor.position(), 2);

    // Restore and verify position reverted
    parser.restore(snapshot);
    assert_eq!(parser.cursor.position(), 0);
}

#[test]
fn test_snapshot_captures_context() {
    let (mut parser, _) = create_parser("42");

    // Set some context
    parser.context = parser.context.with(ParseContext::IN_LOOP);
    let snapshot = parser.snapshot();

    // Change context
    parser.context = parser.context.with(ParseContext::IN_TYPE);
    assert!(parser.context.in_type());
    assert!(parser.context.in_loop());

    // Restore and verify context reverted
    parser.restore(snapshot);
    assert!(parser.context.in_loop());
    assert!(!parser.context.in_type());
}

#[test]
fn test_try_parse_success_preserves_position() {
    let (mut parser, _) = create_parser("let x = 42");

    // Start at 'let'
    assert!(parser.cursor.check(&TokenKind::Let));

    // try_parse that succeeds should advance position
    let result = parser.try_parse(|p| {
        p.cursor.expect(&TokenKind::Let)?;
        Ok(())
    });

    assert!(result.is_some());
    // Position should be after 'let'
    assert!(parser.cursor.check_ident()); // Now at 'x'
}

#[test]
fn test_try_parse_failure_restores_position() {
    let (mut parser, _) = create_parser("let x = 42");

    // Start at 'let'
    assert!(parser.cursor.check(&TokenKind::Let));
    let start_pos = parser.cursor.position();

    // try_parse that fails should restore position
    let result = parser.try_parse(|p| {
        p.cursor.expect(&TokenKind::Let)?;
        p.cursor.expect(&TokenKind::If)?; // This will fail - there's no 'if'
        Ok(())
    });

    assert!(result.is_none());
    // Position should be restored to start
    assert_eq!(parser.cursor.position(), start_pos);
    assert!(parser.cursor.check(&TokenKind::Let));
}

#[test]
fn test_look_ahead_always_restores() {
    let (mut parser, _) = create_parser("let x = 42");

    let start_pos = parser.cursor.position();

    // look_ahead should always restore, even on success
    let is_let = parser.look_ahead(|p| {
        p.cursor.advance(); // consume 'let'
        p.cursor.advance(); // consume 'x'
        p.cursor.check(&TokenKind::Eq) // check for '='
    });

    assert!(is_let);
    // Position should be restored despite success
    assert_eq!(parser.cursor.position(), start_pos);
    assert!(parser.cursor.check(&TokenKind::Let));
}

#[test]
fn test_look_ahead_preserves_context() {
    let (mut parser, _) = create_parser("42");

    parser.context = parser.context.with(ParseContext::IN_LOOP);

    // Modify context inside look_ahead
    parser.look_ahead(|p| {
        p.context = p.context.with(ParseContext::IN_TYPE);
        assert!(p.context.in_type());
    });

    // Context should be restored
    assert!(!parser.context.in_type());
    assert!(parser.context.in_loop());
}

#[test]
fn test_nested_snapshots() {
    let (mut parser, _) = create_parser("let x = 42 + 1");

    let snapshot1 = parser.snapshot();
    parser.cursor.advance(); // 'let' -> 'x'
    parser.cursor.advance(); // 'x' -> '='

    let snapshot2 = parser.snapshot();
    parser.cursor.advance(); // '=' -> '42'
    parser.cursor.advance(); // '42' -> '+'

    // Restore to snapshot2
    parser.restore(snapshot2);
    assert!(parser.cursor.check(&TokenKind::Eq));

    // Restore to snapshot1
    parser.restore(snapshot1);
    assert!(parser.cursor.check(&TokenKind::Let));
}

#[test]
fn test_try_parse_with_context_change() {
    let (mut parser, _) = create_parser("let x = 42");

    // try_parse with context modification
    let result = parser.try_parse(|p| {
        p.context = p.context.with(ParseContext::IN_TYPE);
        p.cursor.expect(&TokenKind::If)?; // Will fail
        Ok(())
    });

    assert!(result.is_none());
    // Context should be restored
    assert!(!parser.context.in_type());
}

#[test]
fn test_snapshot_size() {
    // Verify snapshot is lightweight (documented as ~10 bytes + padding)
    let size = std::mem::size_of::<crate::ParserSnapshot>();
    assert!(
        size <= 24,
        "ParserSnapshot should be small, got {size} bytes"
    );
}

#[test]
fn test_try_parse_returns_value_on_success() {
    let (mut parser, _) = create_parser("let x = 42");

    let result = parser.try_parse(|p| {
        p.cursor.advance(); // consume 'let'
        Ok(42)
    });

    assert_eq!(result, Some(42));
}

#[test]
fn test_look_ahead_returns_computed_value() {
    let (mut parser, _) = create_parser("let x = 42");

    let count = parser.look_ahead(|p| {
        let mut n = 0;
        while !p.cursor.is_at_end() {
            p.cursor.advance();
            n += 1;
        }
        n
    });

    // Should have counted tokens (let, x, =, 42, Eof - but Eof doesn't advance)
    assert!(count >= 4);

    // Position should still be at start
    assert!(parser.cursor.check(&TokenKind::Let));
}

// ============================================================================
// Practical Demonstrations
// ============================================================================
// These tests demonstrate real-world patterns for using snapshots.

/// Demonstrates using `look_ahead()` for multi-token pattern detection.
///
/// This pattern is useful when you need to check more than 2 tokens ahead,
/// or when the lookahead involves skipping whitespace/newlines.
#[test]
fn test_look_ahead_for_pattern_detection() {
    // Detect "with Ident =" pattern (capability provision syntax)
    let (mut parser, _) = create_parser("with Http = Mock");

    let is_capability_syntax = parser.look_ahead(|p| {
        // Check: with
        if !p.cursor.check(&TokenKind::With) {
            return false;
        }
        p.cursor.advance();

        // Check: Ident
        if !p.cursor.check_ident() {
            return false;
        }
        p.cursor.advance();

        // Check: =
        p.cursor.check(&TokenKind::Eq)
    });

    assert!(is_capability_syntax);
    // Position unchanged
    assert!(parser.cursor.check(&TokenKind::With));
}

/// Demonstrates using `try_parse()` for fallback parsing.
///
/// This pattern is useful when you want to try one interpretation
/// and fall back to another if it fails.
#[test]
fn test_try_parse_for_fallback() {
    let (mut parser, _) = create_parser("42 + 1");

    // Try to parse as "let" binding (will fail)
    let binding_result = parser.try_parse(|p| {
        p.cursor.expect(&TokenKind::Let)?;
        p.cursor.expect_ident()
    });

    assert!(binding_result.is_none());
    // Position restored - we can now try as expression
    assert!(matches!(parser.cursor.current_kind(), TokenKind::Int(_)));

    // Try to parse as integer (will succeed)
    let int_result = parser.try_parse(|p| {
        if let TokenKind::Int(n) = *p.cursor.current_kind() {
            p.cursor.advance();
            Ok(n)
        } else {
            Err(crate::ParseError::new(
                ori_diagnostic::ErrorCode::E1001,
                "expected int".to_string(),
                p.cursor.current_span(),
            ))
        }
    });

    assert_eq!(int_result, Some(42));
    // Position advanced past the integer
    assert!(parser.cursor.check(&TokenKind::Plus));
}

/// Demonstrates manual snapshot for complex decision logic.
///
/// This pattern is useful when you need to examine the parse result
/// before deciding whether to keep or discard it.
#[test]
fn test_manual_snapshot_for_complex_decision() {
    let (mut parser, _) = create_parser("x = 42");

    let snapshot = parser.snapshot();

    // Parse identifier
    let name = parser.cursor.expect_ident();
    assert!(name.is_ok());

    // Check what follows - if `=`, this is an assignment
    // In a real parser, we might decide to restore and parse differently
    if parser.cursor.check(&TokenKind::Eq) {
        // This is an assignment - keep the parse
        parser.cursor.advance(); // consume '='
        assert!(matches!(parser.cursor.current_kind(), TokenKind::Int(_)));
    } else {
        // Not an assignment - restore and try something else
        parser.restore(snapshot);
        // Would parse differently here
    }
}

/// Demonstrates that snapshots don't capture arena state.
///
/// This is intentional - speculative parsing should examine tokens,
/// not allocate. If allocations happen during speculation, they persist.
#[test]
fn test_snapshot_does_not_capture_arena() {
    let (mut parser, _) = create_parser("42");

    // Allocations before snapshot would be tracked here if arena size was exposed

    let snapshot = parser.snapshot();

    // Allocations during speculation would persist
    // (In practice, avoid allocating during try_parse/look_ahead)

    parser.restore(snapshot);

    // Arena state unchanged by restore
    // (Any allocations made between snapshot and restore persist)
}
