//! Constant parsing.

use crate::recovery::TokenSet;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{ConstDef, DurationUnit, Expr, ExprKind, Name, SizeUnit, TokenKind, Visibility};

/// Tokens valid as constant literal values.
const CONST_LITERAL_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Int(0))
    .with(TokenKind::Float(0))
    .with(TokenKind::String(Name::EMPTY))
    .with(TokenKind::True)
    .with(TokenKind::False)
    .with(TokenKind::Char('\0'))
    .with(TokenKind::Duration(0, DurationUnit::Nanoseconds))
    .with(TokenKind::Size(0, SizeUnit::Bytes));

impl Parser<'_> {
    /// Parse a constant declaration.
    ///
    /// Syntax: `[pub] let $name = literal`
    ///
    /// Returns `EmptyErr` if no `$` is present.
    pub(crate) fn parse_const(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        if !self.cursor.check(&TokenKind::Dollar) {
            return ParseOutcome::empty_err_expected(&TokenKind::Dollar, self.cursor.position());
        }

        self.parse_const_body(visibility)
    }

    fn parse_const_body(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        let start_span = self.cursor.current_span();

        // $
        committed!(self.cursor.expect(&TokenKind::Dollar));

        // name
        let name = committed!(self.cursor.expect_ident());

        // =
        committed!(self.cursor.expect(&TokenKind::Eq));

        // literal value
        let value = require!(self, self.parse_literal_expr(), "literal value");

        let span = start_span.merge(self.cursor.previous_span());

        ParseOutcome::consumed_ok(ConstDef {
            name,
            value,
            span,
            visibility,
        })
    }

    /// Parse a literal expression for constant values.
    ///
    /// Returns `EmptyErr` if the current token is not a valid literal.
    fn parse_literal_expr(&mut self) -> ParseOutcome<ori_ir::ExprId> {
        let span = self.cursor.current_span();
        let kind = match *self.cursor.current_kind() {
            TokenKind::Int(n) => {
                self.cursor.advance();
                let Ok(value) = i64::try_from(n) else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large".to_string(),
                            span,
                        ),
                        span,
                    );
                };
                ExprKind::Int(value)
            }
            TokenKind::Float(bits) => {
                self.cursor.advance();
                ExprKind::Float(bits)
            }
            TokenKind::String(s) => {
                self.cursor.advance();
                ExprKind::String(s)
            }
            TokenKind::True => {
                self.cursor.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.cursor.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Char(c) => {
                self.cursor.advance();
                ExprKind::Char(c)
            }
            // Duration literals (e.g., 100ms, 30s)
            TokenKind::Duration(value, unit) => {
                self.cursor.advance();
                ExprKind::Duration { value, unit }
            }
            // Size literals (e.g., 4kb, 10mb)
            TokenKind::Size(value, unit) => {
                self.cursor.advance();
                ExprKind::Size { value, unit }
            }
            _ => {
                return ParseOutcome::empty_err(CONST_LITERAL_TOKENS, self.cursor.position());
            }
        };

        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(kind, span)))
    }
}
