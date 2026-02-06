//! Constant parsing.

use crate::{committed, ParseError, ParseOutcome, Parser};
use ori_ir::{ConstDef, Expr, ExprKind, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse a constant declaration.
    ///
    /// Syntax: `[pub] let $name = literal`
    ///
    /// Returns `EmptyErr` if no `$` is present.
    pub(crate) fn parse_const(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        if !self.check(&TokenKind::Dollar) {
            return ParseOutcome::empty_err_expected(&TokenKind::Dollar, self.position());
        }

        self.parse_const_body(visibility)
    }

    fn parse_const_body(&mut self, visibility: Visibility) -> ParseOutcome<ConstDef> {
        let start_span = self.current_span();

        // $
        committed!(self.expect(&TokenKind::Dollar));

        // name
        let name = committed!(self.expect_ident());

        // =
        committed!(self.expect(&TokenKind::Eq));

        // literal value
        let value = committed!(self.parse_literal_expr());

        let span = start_span.merge(self.previous_span());

        ParseOutcome::consumed_ok(ConstDef {
            name,
            value,
            span,
            visibility,
        })
    }

    /// Parse a literal expression for constant values.
    fn parse_literal_expr(&mut self) -> Result<ori_ir::ExprId, ParseError> {
        let span = self.current_span();
        let kind = match *self.current_kind() {
            TokenKind::Int(n) => {
                self.advance();
                let value = i64::try_from(n).map_err(|_| {
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "integer literal too large".to_string(),
                        span,
                    )
                })?;
                ExprKind::Int(value)
            }
            TokenKind::Float(bits) => {
                self.advance();
                ExprKind::Float(bits)
            }
            TokenKind::String(s) => {
                self.advance();
                ExprKind::String(s)
            }
            TokenKind::True => {
                self.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Char(c) => {
                self.advance();
                ExprKind::Char(c)
            }
            // Duration literals (e.g., 100ms, 30s)
            TokenKind::Duration(value, unit) => {
                self.advance();
                ExprKind::Duration { value, unit }
            }
            // Size literals (e.g., 4kb, 10mb)
            TokenKind::Size(value, unit) => {
                self.advance();
                ExprKind::Size { value, unit }
            }
            _ => {
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "constant must be initialized with a literal value".to_string(),
                    span,
                ));
            }
        };

        Ok(self.arena.alloc_expr(Expr::new(kind, span)))
    }
}
