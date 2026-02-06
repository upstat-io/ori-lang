//! Postfix Expression Parsing
//!
//! Parses call, method call, field access, index expressions, and struct literals.

use crate::{ParseError, Parser};
use ori_ir::{
    CallArg, Expr, ExprId, ExprKind, FieldInit, Param, ParsedTypeId, StructLitField, TokenKind,
};

impl Parser<'_> {
    /// Parse function calls and field access.
    #[inline]
    pub(crate) fn parse_call(&mut self) -> Result<ExprId, ParseError> {
        let expr = self.parse_primary().into_result()?;
        self.apply_postfix_ops(expr)
    }

    /// Apply postfix operators to an expression.
    ///
    /// This is factored out from `parse_call()` to be reusable in `parse_unary()`
    /// for cases like `-100 as float` where negative integer folding produces
    /// an expression that still needs postfix operator handling.
    #[inline]
    pub(crate) fn apply_postfix_ops(&mut self, mut expr: ExprId) -> Result<ExprId, ParseError> {
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
                    let args_list = self.arena.alloc_expr_list_inline(&args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Call {
                            func: expr,
                            args: args_list,
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
                        let args_list = self.arena.alloc_expr_list_inline(&args);
                        expr = self.arena.alloc_expr(Expr::new(
                            ExprKind::MethodCall {
                                receiver: expr,
                                method: field,
                                args: args_list,
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
                // Struct literal: Name { field: value, ... } or with spread: Name { ...base, x: 10 }
                // Only valid if expr is an identifier and struct literals are allowed
                // (not allowed in if conditions to avoid ambiguity)
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let struct_name = *name;
                    let start_span = expr_data.span;
                    self.advance(); // {

                    // Parse struct literal fields (may include spread)
                    let struct_lit_fields: Vec<StructLitField> = self.brace_series(|p| {
                        if p.check(&TokenKind::RBrace) {
                            return Ok(None);
                        }

                        let field_span = p.current_span();

                        // Check for spread syntax: ...expr
                        if p.check(&TokenKind::DotDotDot) {
                            p.advance();
                            let spread_expr = p.parse_expr()?;
                            let end_span = p.arena.get_expr(spread_expr).span;
                            return Ok(Some(StructLitField::Spread {
                                expr: spread_expr,
                                span: field_span.merge(end_span),
                            }));
                        }

                        // Regular field: name or name: value
                        let field_name = p.expect_ident()?;

                        // Check for shorthand { x } vs full { x: value }
                        let value = if p.check(&TokenKind::Colon) {
                            p.advance();
                            Some(p.parse_expr()?)
                        } else {
                            // Shorthand: { x } means { x: x }
                            None
                        };

                        let end_span = if let Some(v) = value {
                            p.arena.get_expr(v).span
                        } else {
                            p.previous_span()
                        };

                        Ok(Some(StructLitField::Field(FieldInit {
                            name: field_name,
                            value,
                            span: field_span.merge(end_span),
                        })))
                    })?;

                    let end_span = self.previous_span();

                    // Check if any element is a spread
                    let has_spread = struct_lit_fields
                        .iter()
                        .any(|f| matches!(f, StructLitField::Spread { .. }));

                    if has_spread {
                        // Use StructWithSpread for literals with spread syntax
                        let fields_range = self.arena.alloc_struct_lit_fields(struct_lit_fields);
                        expr = self.arena.alloc_expr(Expr::new(
                            ExprKind::StructWithSpread {
                                name: struct_name,
                                fields: fields_range,
                            },
                            start_span.merge(end_span),
                        ));
                    } else {
                        // Use regular Struct for efficiency (common case)
                        let fields: Vec<FieldInit> = struct_lit_fields
                            .into_iter()
                            .filter_map(|f| match f {
                                StructLitField::Field(init) => Some(init),
                                StructLitField::Spread { .. } => None,
                            })
                            .collect();
                        let fields_range = self.arena.alloc_field_inits(fields);
                        expr = self.arena.alloc_expr(Expr::new(
                            ExprKind::Struct {
                                name: struct_name,
                                fields: fields_range,
                            },
                            start_span.merge(end_span),
                        ));
                    }
                } else {
                    // Not an identifier - break and let other parsing handle it
                    break;
                }
            } else if self.check(&TokenKind::Question) {
                // Error propagation: expr?
                self.advance();
                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(ExprKind::Try(expr), span));
            } else if self.check(&TokenKind::As) {
                // Type conversion: `as type` (infallible) or `as? type` (fallible)
                self.advance();

                // Check for fallible version: as?
                let fallible = if self.check(&TokenKind::Question) {
                    self.advance();
                    true
                } else {
                    false
                };

                // Parse the target type
                let ty = self.parse_type().ok_or_else(|| {
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "expected type after `as`".to_string(),
                        self.current_span(),
                    )
                })?;

                let ty_id = self.arena.alloc_parsed_type(ty);
                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Cast {
                        expr,
                        ty: ty_id,
                        fallible,
                    },
                    span,
                ));
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
                        pattern: None,
                        ty: None,
                        default: None,
                        is_variadic: false,
                        span: param_span,
                    }]);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Lambda {
                            params,
                            ret_ty: ParsedTypeId::INVALID,
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
        use crate::series::SeriesConfig;

        let args: Vec<CallArg> = self.series(&SeriesConfig::comma(TokenKind::RParen), |p| {
            if p.check(&TokenKind::RParen) {
                return Ok(None);
            }

            let arg_span = p.current_span();

            // Check for spread syntax: ...expr
            let is_spread = p.check(&TokenKind::DotDotDot);
            if is_spread {
                p.advance();
            }

            let (name, value) = if p.is_named_arg_start() {
                let name = p.expect_ident_or_keyword()?;
                p.expect(&TokenKind::Colon)?;
                let value = p.parse_expr()?;
                (Some(name), value)
            } else {
                let value = p.parse_expr()?;
                (None, value)
            };

            let end_span = p.arena.get_expr(value).span;

            Ok(Some(CallArg {
                name,
                value,
                is_spread,
                span: arg_span.merge(end_span),
            }))
        })?;

        let has_positional = args.iter().any(|a| a.name.is_none());
        let has_named = args.iter().any(|a| a.name.is_some());

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
