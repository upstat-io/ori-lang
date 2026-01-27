//! Primary Expression Parsing
//!
//! Parses literals, identifiers, variant constructors, parenthesized expressions,
//! lists, if expressions, and let expressions.

use ori_ir::{
    BindingPattern, Expr, ExprId, ExprKind, ExprRange, Param, ParamRange, TokenKind,
};
use crate::{ParseError, Parser};

impl Parser<'_> {
    /// Parse primary expressions.
    pub(crate) fn parse_primary(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();

        // function_seq keywords (run, try)
        if let Some(is_try) = self.match_function_seq_kind() {
            self.advance();
            return self.parse_function_seq(is_try);
        }

        // match is also function_seq but parsed separately
        if self.check(&TokenKind::Match) {
            self.advance();
            return self.parse_match_expr();
        }

        // for pattern: for(over: items, match: pattern -> expr, default: value)
        if self.check(&TokenKind::For) && self.next_is_lparen() {
            self.advance();
            return self.parse_for_pattern();
        }

        // Capability provision: with Capability = Provider in body
        if self.check(&TokenKind::With) && self.is_with_capability_syntax() {
            return self.parse_with_capability();
        }

        // function_exp keywords
        if let Some(kind) = self.match_function_exp_kind() {
            self.advance();
            return self.parse_function_exp(kind);
        }

        match self.current_kind() {
            // Literals
            TokenKind::Int(n) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Int(n), span)))
            }
            TokenKind::Float(bits) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Float(bits), span)))
            }
            TokenKind::True => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Bool(true), span)))
            }
            TokenKind::False => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Bool(false), span)))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::String(name), span)))
            }
            TokenKind::Char(c) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Char(c), span)))
            }
            TokenKind::Duration(value, unit) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Duration { value, unit }, span)))
            }
            TokenKind::Size(value, unit) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Size { value, unit }, span)))
            }

            // Config reference: $name
            TokenKind::Dollar => {
                self.advance();
                let name = self.expect_ident()?;
                let full_span = span.merge(self.previous_span());
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Config(name), full_span)))
            }

            // Identifier
            TokenKind::Ident(name) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Built-in I/O primitives as soft keywords
            TokenKind::Print => {
                let name = self.interner().intern("print");
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::Panic => {
                let name = self.interner().intern("panic");
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // self
            TokenKind::SelfLower => {
                self.advance();
                let name = self.interner().intern("self");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Type conversion functions
            TokenKind::IntType => {
                self.advance();
                let name = self.interner().intern("int");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::FloatType => {
                self.advance();
                let name = self.interner().intern("float");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::StrType => {
                self.advance();
                let name = self.interner().intern("str");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::BoolType => {
                self.advance();
                let name = self.interner().intern("bool");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::CharType => {
                self.advance();
                let name = self.interner().intern("char");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::ByteType => {
                self.advance();
                let name = self.interner().intern("byte");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Variant constructors
            TokenKind::Some => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_expr()?;
                let end_span = self.current_span();
                self.expect(&TokenKind::RParen)?;
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Some(inner),
                    span.merge(end_span),
                )))
            }
            TokenKind::None => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::None, span)))
            }
            TokenKind::Ok => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(&TokenKind::RParen)?;
                    Some(expr)
                } else {
                    None
                };
                let end_span = self.previous_span();
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Ok(inner),
                    span.merge(end_span),
                )))
            }
            TokenKind::Err => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(&TokenKind::RParen)?;
                    Some(expr)
                } else {
                    None
                };
                let end_span = self.previous_span();
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Err(inner),
                    span.merge(end_span),
                )))
            }

            // Parenthesized expression, tuple, or lambda
            TokenKind::LParen => self.parse_parenthesized(),

            // List literal
            TokenKind::LBracket => self.parse_list_literal(),

            // Map literal
            TokenKind::LBrace => self.parse_map_literal(),

            // If expression
            TokenKind::If => self.parse_if_expr(),

            // Let expression
            TokenKind::Let => self.parse_let_expr(),

            _ => Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!("expected expression, found {:?}", self.current_kind()),
                span,
            )),
        }
    }

    /// Parse parenthesized expression, tuple, or lambda.
    fn parse_parenthesized(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();
        self.advance();

        // Case 1: () -> body (lambda with no params)
        if self.check(&TokenKind::RParen) {
            self.advance();

            if self.check(&TokenKind::Arrow) {
                self.advance();
                let ret_ty = if self.check_type_keyword() {
                    let ty = self.parse_type();
                    self.expect(&TokenKind::Eq)?;
                    ty
                } else {
                    None
                };
                let body = self.parse_expr()?;
                let end_span = self.arena.get_expr(body).span;
                return Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Lambda {
                        params: ParamRange::EMPTY,
                        ret_ty,
                        body
                    },
                    span.merge(end_span),
                )));
            }

            let end_span = self.previous_span();
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Tuple(ExprRange::EMPTY),
                span.merge(end_span),
            )));
        }

        // Case 2: Typed lambda params
        if self.is_typed_lambda_params() {
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;
            self.expect(&TokenKind::Arrow)?;

            let ret_ty = if self.check_type_keyword() {
                let ty = self.parse_type();
                self.expect(&TokenKind::Eq)?;
                ty
            } else {
                None
            };

            let body = self.parse_expr()?;
            let end_span = self.arena.get_expr(body).span;
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Lambda { params, ret_ty, body },
                span.merge(end_span),
            )));
        }

        // Case 3: Untyped - parse as expression(s)
        let expr = self.parse_expr()?;

        if self.check(&TokenKind::Comma) {
            let mut exprs = vec![expr];
            while self.check(&TokenKind::Comma) {
                self.advance();
                if self.check(&TokenKind::RParen) {
                    break;
                }
                exprs.push(self.parse_expr()?);
            }
            self.expect(&TokenKind::RParen)?;

            if self.check(&TokenKind::Arrow) {
                self.advance();
                let params = self.exprs_to_params(&exprs)?;
                let body = self.parse_expr()?;
                let end_span = self.arena.get_expr(body).span;
                return Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Lambda { params, ret_ty: None, body },
                    span.merge(end_span),
                )));
            }

            let end_span = self.previous_span();
            let range = self.arena.alloc_expr_list(exprs);
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Tuple(range),
                span.merge(end_span),
            )));
        }

        self.expect(&TokenKind::RParen)?;

        if self.check(&TokenKind::Arrow) {
            self.advance();
            let params = self.exprs_to_params(&[expr])?;
            let body = self.parse_expr()?;
            let end_span = self.arena.get_expr(body).span;
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Lambda { params, ret_ty: None, body },
                span.merge(end_span),
            )));
        }

        Ok(expr)
    }

    /// Parse list literal.
    fn parse_list_literal(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();
        self.advance();
        let mut exprs = Vec::new();

        while !self.check(&TokenKind::RBracket) && !self.is_at_end() {
            exprs.push(self.parse_expr()?);
            if !self.check(&TokenKind::RBracket) {
                self.expect(&TokenKind::Comma)?;
            }
        }

        self.expect(&TokenKind::RBracket)?;
        let end_span = self.previous_span();
        let range = self.arena.alloc_expr_list(exprs);
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::List(range),
            span.merge(end_span),
        )))
    }

    /// Parse map literal: `{ key: value, ... }` or `{}`.
    fn parse_map_literal(&mut self) -> Result<ExprId, ParseError> {
        use ori_ir::MapEntry;

        let span = self.current_span();
        self.advance(); // {
        self.skip_newlines();

        let mut entries = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let entry_span = self.current_span();
            let key = self.parse_expr()?;
            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            let end_span = self.arena.get_expr(value).span;

            entries.push(MapEntry {
                key,
                value,
                span: entry_span.merge(end_span),
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
        let range = self.arena.alloc_map_entries(entries);
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Map(range),
            span.merge(end_span),
        )))
    }

    /// Parse if expression.
    fn parse_if_expr(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();
        self.advance();
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::Then)?;
        self.skip_newlines();
        let then_branch = self.parse_expr()?;

        self.skip_newlines();

        let else_branch = if self.check(&TokenKind::Else) {
            self.advance();
            self.skip_newlines();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let end_span = if let Some(else_id) = else_branch {
            self.arena.get_expr(else_id).span
        } else {
            self.arena.get_expr(then_branch).span
        };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::If { cond, then_branch, else_branch },
            span.merge(end_span),
        )))
    }

    /// Parse let expression.
    fn parse_let_expr(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();
        self.advance();

        let mutable = if self.check(&TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };

        let pattern = self.parse_binding_pattern()?;

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            self.parse_type()
        } else {
            None
        };

        self.expect(&TokenKind::Eq)?;
        let init = self.parse_expr()?;

        let end_span = self.arena.get_expr(init).span;
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Let { pattern, ty, init, mutable },
            span.merge(end_span),
        )))
    }

    /// Parse a binding pattern.
    pub(crate) fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner().intern(name_str);
            self.advance();
            return Ok(BindingPattern::Name(name));
        }

        match self.current_kind() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(BindingPattern::Name(name))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(BindingPattern::Wildcard)
            }
            TokenKind::LParen => {
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                    patterns.push(self.parse_binding_pattern()?);
                    if !self.check(&TokenKind::RParen) {
                        self.expect(&TokenKind::Comma)?;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                Ok(BindingPattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!("expected binding pattern, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    /// Parse capability provision: `with Capability = Provider in body`
    fn parse_with_capability(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();
        self.expect(&TokenKind::With)?;

        // Parse capability name
        let capability = self.expect_ident()?;

        self.expect(&TokenKind::Eq)?;

        // Parse provider expression
        let provider = self.parse_expr()?;

        // Expect `in` keyword
        self.expect(&TokenKind::In)?;
        self.skip_newlines();

        // Parse body expression
        let body = self.parse_expr()?;

        let end_span = self.arena.get_expr(body).span;
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            },
            span.merge(end_span),
        )))
    }

    /// Check if typed lambda params.
    pub(crate) fn is_typed_lambda_params(&self) -> bool {
        let is_ident_like = matches!(self.current_kind(), TokenKind::Ident(_))
            || self.soft_keyword_to_name().is_some();
        if !is_ident_like {
            return false;
        }
        self.next_is_colon()
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
                        ty: None,
                        span: expr.span,
                    });
                }
                _ => {
                    return Err(ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "expected identifier for lambda parameter".to_string(),
                        expr.span,
                    ));
                }
            }
        }
        Ok(self.arena.alloc_params(params))
    }
}
