//! Postfix Expression Parsing
//!
//! Parses call, method call, field access, and index expressions.

use sigil_ir::{CallArg, Expr, ExprId, ExprKind, Param, TokenKind};
use crate::{ParseError, Parser};

impl<'a> Parser<'a> {
    /// Parse function calls and field access.
    pub(crate) fn parse_call(&mut self) -> Result<ExprId, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(TokenKind::LParen) {
                // Function call
                self.advance();
                let (call_args, has_positional, has_named) = self.parse_call_args()?;
                self.expect(TokenKind::RParen)?;

                let call_span = self.arena.get_expr(expr).span.merge(self.previous_span());

                // Validate: multi-arg calls with positional args are an error
                if call_args.len() > 1 && has_positional {
                    return Err(ParseError::new(
                        sigil_diagnostic::ErrorCode::E1011,
                        "function calls with multiple arguments require named arguments (name: value)".to_string(),
                        call_span,
                    ));
                }

                // Choose representation based on whether we have named args
                if has_named {
                    let args_range = self.arena.alloc_call_args(call_args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::CallNamed { func: expr, args: args_range },
                        call_span,
                    ));
                } else {
                    let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                    let args_range = self.arena.alloc_expr_list(args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Call { func: expr, args: args_range },
                        call_span,
                    ));
                }
            } else if self.check(TokenKind::Dot) {
                // Field access or method call
                self.advance();
                let field = self.expect_ident()?;

                if self.check(TokenKind::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        while self.check(TokenKind::Comma) {
                            self.advance();
                            self.skip_newlines();
                            if self.check(TokenKind::RParen) {
                                break;
                            }
                            args.push(self.parse_expr()?);
                        }
                    }
                    let args_range = self.arena.alloc_expr_list(args);
                    self.expect(TokenKind::RParen)?;

                    let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::MethodCall { receiver: expr, method: field, args: args_range },
                        span,
                    ));
                } else {
                    let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Field { receiver: expr, field },
                        span,
                    ));
                }
            } else if self.check(TokenKind::LBracket) {
                // Index access
                self.advance();
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;

                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Index { receiver: expr, index },
                    span,
                ));
            } else if self.check(TokenKind::Arrow) {
                // Single-param lambda without parens: x -> body
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let param_span = expr_data.span;
                    let param_name = *name;
                    self.advance();
                    let body = self.parse_expr()?;
                    let end_span = self.arena.get_expr(body).span;
                    let params = self.arena.alloc_params(vec![Param {
                        name: param_name,
                        ty: None,
                        span: param_span,
                    }]);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Lambda { params, ret_ty: None, body },
                        param_span.merge(end_span),
                    ));
                }
                break;
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse call arguments, supporting both positional and named args.
    pub(crate) fn parse_call_args(&mut self) -> Result<(Vec<CallArg>, bool, bool), ParseError> {
        let mut args = Vec::new();
        let mut has_positional = false;
        let mut has_named = false;

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arg_span = self.current_span();

            if self.is_named_arg_start() {
                let name = self.expect_ident_or_keyword()?;
                self.expect(TokenKind::Colon)?;
                let value = self.parse_expr()?;
                let end_span = self.arena.get_expr(value).span;

                args.push(CallArg {
                    name: Some(name),
                    value,
                    span: arg_span.merge(end_span),
                });
                has_named = true;
            } else {
                let value = self.parse_expr()?;
                let end_span = self.arena.get_expr(value).span;

                args.push(CallArg {
                    name: None,
                    value,
                    span: arg_span.merge(end_span),
                });
                has_positional = true;
            }

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();

        Ok((args, has_positional, has_named))
    }
}
