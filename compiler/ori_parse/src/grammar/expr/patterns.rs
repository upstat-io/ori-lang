//! Pattern Parsing (`function_seq` and `function_exp`)
//!
//! Parses run, try, match, for patterns, and `function_exp` constructs.

use crate::{ParseError, Parser};
use ori_ir::{
    Expr, ExprId, ExprKind, FunctionExp, FunctionExpKind, FunctionSeq, MatchArm, MatchPattern,
    NamedExpr, SeqBinding, TokenKind,
};

impl Parser<'_> {
    /// Parse `function_seq`: run or try with sequential bindings and statements.
    pub(crate) fn parse_function_seq(&mut self, is_try: bool) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span();
        self.expect(&TokenKind::LParen)?;
        self.skip_newlines();

        let mut bindings = Vec::new();
        let mut result_expr = None;

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            if self.check(&TokenKind::Let) {
                let binding_span = self.current_span();
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
                let value = self.parse_expr()?;
                let end_span = self.arena.get_expr(value).span;

                bindings.push(SeqBinding::Let {
                    pattern,
                    ty,
                    value,
                    mutable,
                    span: binding_span.merge(end_span),
                });
            } else {
                let expr_span = self.current_span();
                let expr = self.parse_expr()?;
                let end_span = self.arena.get_expr(expr).span;

                self.skip_newlines();

                if self.check(&TokenKind::Comma) {
                    self.advance();
                    self.skip_newlines();

                    if self.check(&TokenKind::RParen) {
                        result_expr = Some(expr);
                    } else {
                        bindings.push(SeqBinding::Stmt {
                            expr,
                            span: expr_span.merge(end_span),
                        });
                    }
                    continue;
                }
                result_expr = Some(expr);
            }

            self.skip_newlines();

            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(&TokenKind::RParen)?;
        let end_span = self.previous_span();

        let result = result_expr.ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "{} requires a result expression",
                    if is_try { "try" } else { "run" }
                ),
                end_span,
            )
        })?;

        let bindings_range = self.arena.alloc_seq_bindings(bindings);
        let span = start_span.merge(end_span);
        let func_seq = if is_try {
            FunctionSeq::Try {
                bindings: bindings_range,
                result,
                span,
            }
        } else {
            FunctionSeq::Run {
                bindings: bindings_range,
                result,
                span,
            }
        };

        Ok(self
            .arena
            .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq), span)))
    }

    /// Parse match as `function_seq`: match(scrutinee, Pattern -> expr, ...)
    pub(crate) fn parse_match_expr(&mut self) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span();
        self.expect(&TokenKind::LParen)?;
        self.skip_newlines();

        let scrutinee = self.parse_expr()?;

        self.skip_newlines();
        self.expect(&TokenKind::Comma)?;
        self.skip_newlines();

        let mut arms = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arm_span = self.current_span();
            let pattern = self.parse_match_pattern()?;

            self.expect(&TokenKind::Arrow)?;
            let body = self.parse_expr()?;
            let end_span = self.arena.get_expr(body).span;

            arms.push(MatchArm {
                pattern,
                guard: None,
                body,
                span: arm_span.merge(end_span),
            });

            self.skip_newlines();

            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(&TokenKind::RParen)?;
        let end_span = self.previous_span();

        if arms.is_empty() {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "match requires at least one arm".to_string(),
                end_span,
            ));
        }

        let arms_range = self.arena.alloc_arms(arms);
        let span = start_span.merge(end_span);
        let func_seq = FunctionSeq::Match {
            scrutinee,
            arms: arms_range,
            span,
        };

        Ok(self
            .arena
            .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq), span)))
    }

    /// Parse for pattern: for(over: items, [map: transform,] match: Pattern -> expr, default: value)
    pub(crate) fn parse_for_pattern(&mut self) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span();
        self.expect(&TokenKind::LParen)?;
        self.skip_newlines();

        let mut over: Option<ExprId> = None;
        let mut map: Option<ExprId> = None;
        let mut match_arm: Option<MatchArm> = None;
        let mut default: Option<ExprId> = None;

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            if !self.is_named_arg_start() {
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    "`for` pattern requires named properties (over:, match:, default:)".to_string(),
                    self.current_span(),
                ));
            }

            let name = self.expect_ident_or_keyword()?;
            let name_str = self.interner().lookup(name).to_string();
            self.expect(&TokenKind::Colon)?;

            match name_str.as_str() {
                "over" => {
                    over = Some(self.parse_expr()?);
                }
                "map" => {
                    map = Some(self.parse_expr()?);
                }
                "match" => {
                    let arm_span = self.current_span();
                    let pattern = self.parse_match_pattern()?;
                    self.expect(&TokenKind::Arrow)?;
                    let body = self.parse_expr()?;
                    let end_span = self.arena.get_expr(body).span;
                    match_arm = Some(MatchArm {
                        pattern,
                        guard: None,
                        body,
                        span: arm_span.merge(end_span),
                    });
                }
                "default" => {
                    default = Some(self.parse_expr()?);
                }
                _ => {
                    return Err(ParseError::new(
                        ori_diagnostic::ErrorCode::E1013,
                        format!("`for` pattern does not accept property `{name_str}`"),
                        self.previous_span(),
                    ));
                }
            }

            self.skip_newlines();
            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(&TokenKind::RParen)?;
        let end_span = self.previous_span();
        let span = start_span.merge(end_span);

        let over = over.ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1013,
                "`for` pattern requires `over:` property".to_string(),
                span,
            )
        })?;
        let arm = match_arm.ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1013,
                "`for` pattern requires `match:` property".to_string(),
                span,
            )
        })?;
        let default = default.ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1013,
                "`for` pattern requires `default:` property".to_string(),
                span,
            )
        })?;

        let func_seq = FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            span,
        };

        Ok(self
            .arena
            .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq), span)))
    }

    /// Parse a match pattern (for match arms).
    pub(crate) fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        match self.current_kind() {
            TokenKind::Underscore => {
                self.advance();
                Ok(MatchPattern::Wildcard)
            }
            TokenKind::Int(n) => {
                let pat_span = self.current_span();
                self.advance();
                let value = i64::try_from(n).map_err(|_| {
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "integer literal too large".to_string(),
                        pat_span,
                    )
                })?;
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Int(value),
                    self.previous_span(),
                ))))
            }
            TokenKind::True => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Bool(true),
                    self.previous_span(),
                ))))
            }
            TokenKind::False => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Bool(false),
                    self.previous_span(),
                ))))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::String(name),
                    self.previous_span(),
                ))))
            }
            TokenKind::Ident(name) => {
                self.advance();
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let inner = self.parse_variant_inner_patterns()?;
                    self.expect(&TokenKind::RParen)?;
                    Ok(MatchPattern::Variant { name, inner })
                } else {
                    Ok(MatchPattern::Binding(name))
                }
            }
            TokenKind::Some => {
                let name = self.interner().intern("Some");
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_variant_inner_patterns()?;
                self.expect(&TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::None => {
                let name = self.interner().intern("None");
                self.advance();
                Ok(MatchPattern::Variant { name, inner: vec![] })
            }
            TokenKind::Ok => {
                let name = self.interner().intern("Ok");
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_variant_inner_patterns()?;
                self.expect(&TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::Err => {
                let name = self.interner().intern("Err");
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_variant_inner_patterns()?;
                self.expect(&TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::LParen => {
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                    patterns.push(self.parse_match_pattern()?);
                    if !self.check(&TokenKind::RParen) {
                        self.expect(&TokenKind::Comma)?;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                Ok(MatchPattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!("expected match pattern, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    /// Parse `function_exp`: map, filter, fold, etc. with named properties.
    pub(crate) fn parse_function_exp(
        &mut self,
        kind: FunctionExpKind,
    ) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span();
        self.expect(&TokenKind::LParen)?;
        self.skip_newlines();

        let mut props = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            if !self.is_named_arg_start() {
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    format!("`{}` requires named properties (name: value)", kind.name()),
                    self.current_span(),
                ));
            }

            let name = self.expect_ident_or_keyword()?;
            let prop_span = self.previous_span();
            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            let end_span = self.arena.get_expr(value).span;

            props.push(NamedExpr {
                name,
                value,
                span: prop_span.merge(end_span),
            });

            self.skip_newlines();

            if !self.check(&TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(&TokenKind::RParen)?;
        let end_span = self.previous_span();

        let props_range = self.arena.alloc_named_exprs(props);
        let func_exp = FunctionExp {
            kind,
            props: props_range,
            span: start_span.merge(end_span),
        };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionExp(func_exp),
            start_span.merge(end_span),
        )))
    }

    /// Parse comma-separated patterns inside a variant pattern.
    ///
    /// Returns an empty Vec for unit variants (when immediately followed by `)`),
    /// or a Vec with one or more patterns for variants with fields.
    fn parse_variant_inner_patterns(&mut self) -> Result<Vec<MatchPattern>, ParseError> {
        let mut patterns = Vec::new();

        // Empty case: immediately followed by )
        if self.check(&TokenKind::RParen) {
            return Ok(patterns);
        }

        // Parse first pattern
        patterns.push(self.parse_match_pattern()?);

        // Parse additional patterns separated by commas
        while self.check(&TokenKind::Comma) {
            self.advance();
            // Allow trailing comma
            if self.check(&TokenKind::RParen) {
                break;
            }
            patterns.push(self.parse_match_pattern()?);
        }

        Ok(patterns)
    }
}
