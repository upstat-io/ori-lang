//! Helper utilities for primary expression parsing.
//!
//! Contains lookahead checks and expression-to-parameter conversion
//! used by the parenthesized expression parser.

use crate::{ParseError, Parser};
use ori_ir::{ExprId, ExprKind, FunctionExpKind, Name, Param, ParamRange, TokenKind};

impl Parser<'_> {
    /// Check if typed lambda params.
    pub(crate) fn is_typed_lambda_params(&self) -> bool {
        let is_ident_like = matches!(self.cursor.current_kind(), TokenKind::Ident(_))
            || self.cursor.soft_keyword_to_name().is_some();
        if !is_ident_like {
            return false;
        }
        self.cursor.next_is_colon()
    }

    /// Convert expressions to lambda parameters.
    pub(crate) fn exprs_to_params(&mut self, exprs: &[ExprId]) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();
        for &expr_id in exprs {
            let expr = self.arena.get_expr(expr_id);
            match &expr.kind {
                ExprKind::Ident(name) => {
                    params.push(Param {
                        name: *name,
                        pattern: None,
                        ty: None,
                        default: None,
                        is_variadic: false,
                        span: expr.span,
                    });
                }
                _ => {
                    return Err(ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "expected identifier for lambda parameter",
                        expr.span,
                    ));
                }
            }
        }
        Ok(self.arena.alloc_params(params))
    }

    /// Check if an identifier name maps to a channel constructor kind.
    pub(super) fn match_channel_kind(&self, name: Name) -> Option<FunctionExpKind> {
        if name == self.known.channel {
            Some(FunctionExpKind::Channel)
        } else if name == self.known.channel_in {
            Some(FunctionExpKind::ChannelIn)
        } else if name == self.known.channel_out {
            Some(FunctionExpKind::ChannelOut)
        } else if name == self.known.channel_all {
            Some(FunctionExpKind::ChannelAll)
        } else {
            None
        }
    }
}
