//! Constant parsing.

use crate::{ParseError, ParseOutcome, Parser};
use ori_ir::{ConstDef, Expr, ExprKind, TokenKind, Visibility};

impl Parser<'_> {
    /// Parse a constant with outcome tracking.
    pub(crate) fn parse_const_with_outcome(
        &mut self,
        visibility: Visibility,
    ) -> ParseOutcome<ConstDef> {
        self.with_outcome(|p| p.parse_const(visibility))
    }

    /// Parse a constant declaration.
    ///
    /// Syntax: `[pub] let $name = literal`
    pub(crate) fn parse_const(&mut self, visibility: Visibility) -> Result<ConstDef, ParseError> {
        let start_span = self.current_span();

        // $
        self.expect(&TokenKind::Dollar)?;

        // name
        let name = self.expect_ident()?;

        // =
        self.expect(&TokenKind::Eq)?;

        // literal value
        let value = self.parse_literal_expr()?;

        let span = start_span.merge(self.previous_span());

        Ok(ConstDef {
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
