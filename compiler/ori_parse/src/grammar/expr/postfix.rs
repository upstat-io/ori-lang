//! Postfix Expression Parsing
//!
//! Parses call, method call, field access, index expressions, and struct literals.

use crate::{ParseError, Parser};
use ori_ir::{CallArg, Expr, ExprId, ExprKind, FieldInit, Param, TokenKind};

impl Parser<'_> {
    /// Parse function calls and field access.
    pub(crate) fn parse_call(&mut self) -> Result<ExprId, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            // Skip newlines to allow method chaining across lines:
            // Builder.new()
            //     .with_name(name: "example")
            //     .with_value(value: 42)
            self.skip_newlines();

            if self.check(&TokenKind::LParen) {
                // Function call
                self.advance();
                let (call_args, _has_positional, has_named) = self.parse_call_args()?;
                self.expect(&TokenKind::RParen)?;

                let call_span = self.arena.get_expr(expr).span.merge(self.previous_span());

                // Named args validation is done in type checking, where we can distinguish
                // between direct function calls (require named) and function variable calls
                // (allow positional since param names are unknowable)

                // Choose representation based on whether we have named args
                if has_named {
                    let args_range = self.arena.alloc_call_args(call_args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::CallNamed {
                            func: expr,
                            args: args_range,
                        },
                        call_span,
                    ));
                } else {
                    let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                    let args_range = self.arena.alloc_expr_list(args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Call {
                            func: expr,
                            args: args_range,
                        },
                        call_span,
                    ));
                }
            } else if self.check(&TokenKind::Dot) {
                // Field access or method call
                self.advance();
                let field = self.expect_ident()?;

                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let (call_args, _has_positional, has_named) = self.parse_call_args()?;
                    self.expect(&TokenKind::RParen)?;

                    let span = self.arena.get_expr(expr).span.merge(self.previous_span());

                    // Named args validation is done in type checking

                    if has_named {
                        // Use MethodCallNamed for named arguments
                        let args_range = self.arena.alloc_call_args(call_args);
                        expr = self.arena.alloc_expr(Expr::new(
                            ExprKind::MethodCallNamed {
                                receiver: expr,
                                method: field,
                                args: args_range,
                            },
                            span,
                        ));
                    } else {
                        // Use MethodCall for positional arguments
                        let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                        let args_range = self.arena.alloc_expr_list(args);
                        expr = self.arena.alloc_expr(Expr::new(
                            ExprKind::MethodCall {
                                receiver: expr,
                                method: field,
                                args: args_range,
                            },
                            span,
                        ));
                    }
                } else {
                    let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Field {
                            receiver: expr,
                            field,
                        },
                        span,
                    ));
                }
            } else if self.check(&TokenKind::LBracket) {
                // Index access
                self.advance();
                // Parse index expression, with # representing length of receiver
                let index = self.parse_index_expr()?;
                self.expect(&TokenKind::RBracket)?;

                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Index {
                        receiver: expr,
                        index,
                    },
                    span,
                ));
            } else if self.check(&TokenKind::LBrace) && self.allows_struct_lit() {
                // Struct literal: Name { field: value, ... }
                // Only valid if expr is an identifier and struct literals are allowed
                // (not allowed in if conditions to avoid ambiguity)
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let struct_name = *name;
                    let start_span = expr_data.span;
                    self.advance(); // {
                    self.skip_newlines();

                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                        let field_span = self.current_span();
                        let field_name = self.expect_ident()?;

                        // Check for shorthand { x } vs full { x: value }
                        let value = if self.check(&TokenKind::Colon) {
                            self.advance();
                            Some(self.parse_expr()?)
                        } else {
                            // Shorthand: { x } means { x: x }
                            None
                        };

                        let end_span = if let Some(v) = value {
                            self.arena.get_expr(v).span
                        } else {
                            self.previous_span()
                        };

                        fields.push(FieldInit {
                            name: field_name,
                            value,
                            span: field_span.merge(end_span),
                        });

                        self.skip_newlines();
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                            self.skip_newlines();
                        } else {
                            break;
                        }
                    }

                    self.expect(&TokenKind::RBrace)?;
                    let end_span = self.previous_span();
                    let fields_range = self.arena.alloc_field_inits(fields);

                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Struct {
                            name: struct_name,
                            fields: fields_range,
                        },
                        start_span.merge(end_span),
                    ));
                } else {
                    // Not an identifier - break and let other parsing handle it
                    break;
                }
            } else if self.check(&TokenKind::Arrow) {
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
                        ExprKind::Lambda {
                            params,
                            ret_ty: None,
                            body,
                        },
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

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arg_span = self.current_span();

            if self.is_named_arg_start() {
                let name = self.expect_ident_or_keyword()?;
                self.expect(&TokenKind::Colon)?;
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

            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();

        Ok((args, has_positional, has_named))
    }

    /// Parse an index expression, where `#` represents the length of the receiver.
    ///
    /// Inside `[...]`, the `#` symbol is parsed as `ExprKind::HashLength`,
    /// which is resolved to the receiver's length during evaluation.
    fn parse_index_expr(&mut self) -> Result<ExprId, ParseError> {
        use crate::context::ParseContext;
        self.with_context(ParseContext::IN_INDEX, Self::parse_expr)
    }
}
