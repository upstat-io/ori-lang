//! Postfix Expression Parsing
//!
//! Parses call, method call, field access, index expressions, and struct literals.

use crate::{chain, committed, ParseError, ParseOutcome, Parser};
use ori_ir::{
    CallArg, Expr, ExprId, ExprKind, FieldInit, Param, ParsedTypeId, StructLitField, TokenKind,
};

/// Bitset of tags that can start a postfix operation.
/// Bit N is set if tag N can start a postfix op.
/// Uses two u64s to cover tags 0-127.
const POSTFIX_BITSET: [u64; 2] = {
    let mut bits = [0u64; 2];
    let tags: [u8; 7] = [
        TokenKind::TAG_LPAREN,   // 80
        TokenKind::TAG_DOT,      // 89
        TokenKind::TAG_LBRACKET, // 84
        TokenKind::TAG_LBRACE,   // 82
        TokenKind::TAG_QUESTION, // 96
        TokenKind::TAG_AS,       // 43
        TokenKind::TAG_ARROW,    // 93
    ];
    let mut i = 0;
    while i < tags.len() {
        let t = tags[i] as usize;
        bits[t / 64] |= 1u64 << (t % 64);
        i += 1;
    }
    bits
};

/// O(1) bitset check for postfix-starting tokens.
#[inline]
fn is_postfix_tag(tag: u8) -> bool {
    let idx = tag as usize;
    if idx >= 128 {
        return false;
    }
    (POSTFIX_BITSET[idx / 64] >> (idx % 64)) & 1 != 0
}

impl Parser<'_> {
    /// Parse function calls and field access.
    ///
    /// Returns `EmptyErr` if no primary expression is found (propagated from `parse_primary`).
    /// Returns `ConsumedErr` if postfix parsing fails after consuming tokens.
    #[inline]
    pub(crate) fn parse_call(&mut self) -> ParseOutcome<ExprId> {
        let expr = chain!(self, self.parse_primary());
        let result = committed!(self.apply_postfix_ops(expr));
        ParseOutcome::consumed_ok(result)
    }

    /// Apply postfix operators to an expression.
    ///
    /// This is factored out from `parse_call()` to be reusable in `parse_unary()`
    /// for cases like `-100 as float` where negative integer folding produces
    /// an expression that still needs postfix operator handling.
    #[inline]
    pub(crate) fn apply_postfix_ops(&mut self, mut expr: ExprId) -> Result<ExprId, ParseError> {
        loop {
            // Skip newlines to allow method chaining across lines
            self.cursor.skip_newlines();

            // Fast exit: O(1) bitset check — if current tag can't start any
            // postfix op, break immediately without testing each alternative.
            if !is_postfix_tag(self.cursor.current_tag()) {
                break;
            }

            if self.cursor.check(&TokenKind::LParen) {
                self.cursor.advance();
                expr = self.parse_postfix_call(expr)?;
            } else if self.cursor.check(&TokenKind::Dot) {
                self.cursor.advance();
                expr = self.parse_postfix_dot(expr)?;
            } else if self.cursor.check(&TokenKind::LBracket) {
                self.cursor.advance();
                let index = self.parse_index_expr()?;
                self.cursor.expect(&TokenKind::RBracket)?;
                let span = self
                    .arena
                    .get_expr(expr)
                    .span
                    .merge(self.cursor.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Index {
                        receiver: expr,
                        index,
                    },
                    span,
                ));
            } else if self.cursor.check(&TokenKind::LBrace) && self.allows_struct_lit() {
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let struct_name = *name;
                    let start_span = expr_data.span;
                    self.cursor.advance();
                    expr = self.parse_postfix_struct_lit(struct_name, start_span)?;
                } else {
                    break;
                }
            } else if self.cursor.check(&TokenKind::Question) {
                self.cursor.advance();
                let span = self
                    .arena
                    .get_expr(expr)
                    .span
                    .merge(self.cursor.previous_span());
                expr = self.arena.alloc_expr(Expr::new(ExprKind::Try(expr), span));
            } else if self.cursor.check(&TokenKind::As) {
                self.cursor.advance();
                expr = self.parse_postfix_cast(expr)?;
            } else if self.cursor.check(&TokenKind::Arrow) {
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let param_span = expr_data.span;
                    let param_name = *name;
                    self.cursor.advance();
                    expr = self.parse_postfix_lambda(param_name, param_span)?;
                }
                break;
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse a function call after the opening `(` has been consumed.
    fn parse_postfix_call(&mut self, func: ExprId) -> Result<ExprId, ParseError> {
        let (call_args, has_named) = self.parse_call_args()?;
        self.cursor.expect(&TokenKind::RParen)?;

        let call_span = self
            .arena
            .get_expr(func)
            .span
            .merge(self.cursor.previous_span());

        if has_named {
            let args_range = self.arena.alloc_call_args(call_args);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::CallNamed {
                    func,
                    args: args_range,
                },
                call_span,
            )))
        } else {
            let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
            let args_list = self.arena.alloc_expr_list_inline(&args);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Call {
                    func,
                    args: args_list,
                },
                call_span,
            )))
        }
    }

    /// Parse dot access (field, method call, or method-style match) after `.` consumed.
    fn parse_postfix_dot(&mut self, receiver: ExprId) -> Result<ExprId, ParseError> {
        // Method-style match: expr.match(pattern -> body, ...)
        if self.cursor.check(&TokenKind::Match) {
            let start_span = self.arena.get_expr(receiver).span;
            self.cursor.advance();
            self.cursor.expect(&TokenKind::LParen)?;
            return self.parse_match_arms_with_scrutinee(receiver, start_span);
        }

        let field = self.cursor.expect_member_name()?;

        if self.cursor.check(&TokenKind::LParen) {
            // Method call
            self.cursor.advance();
            let (call_args, has_named) = self.parse_call_args()?;
            self.cursor.expect(&TokenKind::RParen)?;

            let span = self
                .arena
                .get_expr(receiver)
                .span
                .merge(self.cursor.previous_span());

            if has_named {
                let args_range = self.arena.alloc_call_args(call_args);
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::MethodCallNamed {
                        receiver,
                        method: field,
                        args: args_range,
                    },
                    span,
                )))
            } else {
                let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                let args_list = self.arena.alloc_expr_list_inline(&args);
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::MethodCall {
                        receiver,
                        method: field,
                        args: args_list,
                    },
                    span,
                )))
            }
        } else {
            // Field access
            let span = self
                .arena
                .get_expr(receiver)
                .span
                .merge(self.cursor.previous_span());
            Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Field { receiver, field }, span)))
        }
    }

    /// Parse a struct literal after `{` has been consumed.
    fn parse_postfix_struct_lit(
        &mut self,
        struct_name: ori_ir::Name,
        start_span: ori_ir::Span,
    ) -> Result<ExprId, ParseError> {
        // Struct literal fields use a Vec because nested struct literals
        // share the same buffer, causing same-buffer nesting conflicts.
        let mut fields: Vec<StructLitField> = Vec::new();
        let mut has_spread = false;
        self.brace_series_direct(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(false);
            }

            let field_span = p.cursor.current_span();

            // Check for spread syntax: ...expr
            if p.cursor.check(&TokenKind::DotDotDot) {
                p.cursor.advance();
                has_spread = true;
                let spread_expr = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(spread_expr).span;
                fields.push(StructLitField::Spread {
                    expr: spread_expr,
                    span: field_span.merge(end_span),
                });
                return Ok(true);
            }

            // Regular field: name or name: value
            let field_name = p.cursor.expect_ident()?;
            let value = if p.cursor.check(&TokenKind::Colon) {
                p.cursor.advance();
                Some(p.parse_expr().into_result()?)
            } else {
                None
            };

            let end_span = if let Some(v) = value {
                p.arena.get_expr(v).span
            } else {
                p.cursor.previous_span()
            };

            fields.push(StructLitField::Field(FieldInit {
                name: field_name,
                value,
                span: field_span.merge(end_span),
            }));
            Ok(true)
        })?;

        let end_span = self.cursor.previous_span();

        if has_spread {
            let fields_range = self.arena.alloc_struct_lit_fields(fields);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::StructWithSpread {
                    name: struct_name,
                    fields: fields_range,
                },
                start_span.merge(end_span),
            )))
        } else {
            let field_inits: Vec<FieldInit> = fields
                .into_iter()
                .filter_map(|f| match f {
                    StructLitField::Field(init) => Some(init),
                    StructLitField::Spread { .. } => None,
                })
                .collect();
            let fields_range = self.arena.alloc_field_inits(field_inits);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Struct {
                    name: struct_name,
                    fields: fields_range,
                },
                start_span.merge(end_span),
            )))
        }
    }

    /// Parse a type cast (`as type` or `as? type`) after `as` consumed.
    fn parse_postfix_cast(&mut self, expr: ExprId) -> Result<ExprId, ParseError> {
        let fallible = if self.cursor.check(&TokenKind::Question) {
            self.cursor.advance();
            true
        } else {
            false
        };

        let ty = self.parse_type().ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected type after `as`".to_string(),
                self.cursor.current_span(),
            )
        })?;

        let ty_id = self.arena.alloc_parsed_type(ty);
        let span = self
            .arena
            .get_expr(expr)
            .span
            .merge(self.cursor.previous_span());
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Cast {
                expr,
                ty: ty_id,
                fallible,
            },
            span,
        )))
    }

    /// Parse a single-param lambda shorthand (`x -> body`) after `->` consumed.
    fn parse_postfix_lambda(
        &mut self,
        param_name: ori_ir::Name,
        param_span: ori_ir::Span,
    ) -> Result<ExprId, ParseError> {
        let body = self.parse_expr().into_result()?;
        let end_span = self.arena.get_expr(body).span;
        let params = self.arena.alloc_params(vec![Param {
            name: param_name,
            pattern: None,
            ty: None,
            default: None,
            is_variadic: false,
            span: param_span,
        }]);
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Lambda {
                params,
                ret_ty: ParsedTypeId::INVALID,
                body,
            },
            param_span.merge(end_span),
        )))
    }

    /// Parse call arguments, supporting both positional and named args.
    pub(crate) fn parse_call_args(&mut self) -> Result<(Vec<CallArg>, bool), ParseError> {
        use crate::series::SeriesConfig;

        let args: Vec<CallArg> = self.series(&SeriesConfig::comma(TokenKind::RParen), |p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(None);
            }

            let arg_span = p.cursor.current_span();

            // Check for spread syntax: ...expr
            let is_spread = p.cursor.check(&TokenKind::DotDotDot);
            if is_spread {
                p.cursor.advance();
            }

            let (name, value) = if p.cursor.is_named_arg_start() {
                let name = p.cursor.expect_ident_or_keyword()?;
                p.cursor.expect(&TokenKind::Colon)?;
                let value = p.parse_expr().into_result()?;
                (Some(name), value)
            } else {
                let value = p.parse_expr().into_result()?;
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

        let has_named = args.iter().any(|a| a.name.is_some());

        Ok((args, has_named))
    }

    /// Parse an index expression, where `#` represents the length of the receiver.
    ///
    /// Inside `[...]`, the `#` symbol is parsed as `ExprKind::HashLength`,
    /// which is resolved to the receiver's length during evaluation.
    fn parse_index_expr(&mut self) -> Result<ExprId, ParseError> {
        use crate::context::ParseContext;
        self.with_context(ParseContext::IN_INDEX, Self::parse_expr)
            .into_result()
    }
}
