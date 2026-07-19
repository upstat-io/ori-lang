//! Literal and range match-pattern parsing.
//!
//! Parses literal patterns (integers, booleans, strings, chars, including
//! negative integers) and range patterns (`1..10`, `'a'..='z'`).

use crate::recovery::TokenSet;
use crate::{ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, MatchPattern, Name, TokenKind};

/// Tokens that start a literal pattern (including negative via `-`).
const PATTERN_LITERAL_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Minus)
    .with(TokenKind::Int(0))
    .with(TokenKind::True)
    .with(TokenKind::False)
    .with(TokenKind::String(Name::EMPTY))
    .with(TokenKind::Char('\0'));

impl Parser<'_> {
    /// Parse literal patterns: integers (possibly negative), booleans, strings.
    /// Also handles range patterns: `1..10`, `1..=10`.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive literal and range pattern dispatch across all token kinds"
    )]
    pub(super) fn parse_pattern_literal(&mut self) -> ParseOutcome<MatchPattern> {
        match *self.cursor.current_kind() {
            // Negative integer literal: -42
            TokenKind::Minus => {
                let start_span = self.cursor.current_span();
                self.cursor.advance();
                if let TokenKind::Int(n) = *self.cursor.current_kind() {
                    self.cursor.advance();
                    let Ok(value) = i64::try_from(n) else {
                        return ParseOutcome::consumed_err(
                            ParseError::new(
                                ori_diagnostic::ErrorCode::E1002,
                                "integer literal too large",
                                start_span,
                            ),
                            start_span,
                        );
                    };
                    let span = start_span.merge(self.cursor.previous_span());
                    ParseOutcome::consumed_ok(MatchPattern::Literal(
                        self.arena
                            .alloc_expr(Expr::new(ExprKind::Int(-value), span)),
                    ))
                } else {
                    ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "expected integer after `-` in pattern",
                            self.cursor.current_span(),
                        ),
                        start_span,
                    )
                }
            }

            // Positive integer literal: 42 (with possible range)
            TokenKind::Int(n) => {
                let pat_span = self.cursor.current_span();
                self.cursor.advance();
                let Ok(value) = i64::try_from(n) else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large",
                            pat_span,
                        ),
                        pat_span,
                    );
                };

                // Check for range pattern: 1..10 or 1..=10
                if self.cursor.check(&TokenKind::DotDot) || self.cursor.check(&TokenKind::DotDotEq)
                {
                    let inclusive = self.cursor.check(&TokenKind::DotDotEq);
                    self.cursor.advance();
                    let start_expr = self
                        .arena
                        .alloc_expr(Expr::new(ExprKind::Int(value), pat_span));

                    // Parse end of range (optional for open-ended ranges)
                    let end = if self.is_range_bound_start() {
                        match self.parse_range_bound() {
                            Ok(e) => Some(e),
                            Err(err) => {
                                return ParseOutcome::consumed_err(err, pat_span);
                            }
                        }
                    } else {
                        None
                    };

                    return ParseOutcome::consumed_ok(MatchPattern::Range {
                        start: Some(start_expr),
                        end,
                        inclusive,
                    });
                }

                ParseOutcome::consumed_ok(MatchPattern::Literal(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Int(value), self.cursor.previous_span())),
                ))
            }
            TokenKind::True => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(MatchPattern::Literal(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Bool(true), self.cursor.previous_span())),
                ))
            }
            TokenKind::False => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Bool(false),
                    self.cursor.previous_span(),
                ))))
            }
            TokenKind::String(name) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::String(name),
                    self.cursor.previous_span(),
                ))))
            }
            TokenKind::Char(c) => {
                let pat_span = self.cursor.current_span();
                self.cursor.advance();

                // Check for range pattern: 'a'..'z' or 'a'..='z'
                if self.cursor.check(&TokenKind::DotDot) || self.cursor.check(&TokenKind::DotDotEq)
                {
                    let inclusive = self.cursor.check(&TokenKind::DotDotEq);
                    self.cursor.advance();
                    let start_expr = self
                        .arena
                        .alloc_expr(Expr::new(ExprKind::Char(c), pat_span));

                    let end = if self.is_range_bound_start() {
                        match self.parse_range_bound() {
                            Ok(e) => Some(e),
                            Err(err) => {
                                return ParseOutcome::consumed_err(err, pat_span);
                            }
                        }
                    } else {
                        None
                    };

                    return ParseOutcome::consumed_ok(MatchPattern::Range {
                        start: Some(start_expr),
                        end,
                        inclusive,
                    });
                }

                ParseOutcome::consumed_ok(MatchPattern::Literal(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Char(c), self.cursor.previous_span())),
                ))
            }
            _ => ParseOutcome::empty_err(
                PATTERN_LITERAL_TOKENS,
                self.cursor.current_span().start as usize,
            ),
        }
    }

    /// Check if current token can start a range bound (integer, char, or minus).
    fn is_range_bound_start(&self) -> bool {
        matches!(
            self.cursor.current_kind(),
            TokenKind::Int(_) | TokenKind::Char(_) | TokenKind::Minus
        )
    }

    /// Parse a range bound (integer, possibly negative).
    fn parse_range_bound(&mut self) -> Result<ExprId, ParseError> {
        let start_span = self.cursor.current_span();

        if self.cursor.check(&TokenKind::Minus) {
            self.cursor.advance();
            if let TokenKind::Int(n) = *self.cursor.current_kind() {
                self.cursor.advance();
                let value = i64::try_from(n).map_err(|_| {
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "integer literal too large",
                        start_span,
                    )
                })?;
                let span = start_span.merge(self.cursor.previous_span());
                Ok(self
                    .arena
                    .alloc_expr(Expr::new(ExprKind::Int(-value), span)))
            } else {
                Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "expected integer after `-` in range pattern",
                    self.cursor.current_span(),
                ))
            }
        } else if let TokenKind::Int(n) = *self.cursor.current_kind() {
            self.cursor.advance();
            let value = i64::try_from(n).map_err(|_| {
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "integer literal too large",
                    start_span,
                )
            })?;
            Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Int(value), self.cursor.previous_span())))
        } else if let TokenKind::Char(c) = *self.cursor.current_kind() {
            self.cursor.advance();
            Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Char(c), self.cursor.previous_span())))
        } else {
            Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected integer or char literal in range pattern",
                self.cursor.current_span(),
            ))
        }
    }
}
