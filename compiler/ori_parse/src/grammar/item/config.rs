//! Config variable parsing.

use crate::{ParseError, Parser};
use ori_ir::{ConfigDef, Expr, ExprKind, TokenKind};

impl Parser<'_> {
    /// Parse a config variable declaration.
    ///
    /// Syntax: `[pub] $name = literal`
    pub(crate) fn parse_config(&mut self, is_public: bool) -> Result<ConfigDef, ParseError> {
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

        Ok(ConfigDef {
            name,
            value,
            span,
            is_public,
        })
    }

    /// Parse a literal expression for config values.
    fn parse_literal_expr(&mut self) -> Result<ori_ir::ExprId, ParseError> {
        let span = self.current_span();
        let kind = match self.current_kind() {
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
                    "config variable must be initialized with a literal value".to_string(),
                    span,
                ));
            }
        };

        Ok(self.arena.alloc_expr(Expr::new(kind, span)))
    }
}
