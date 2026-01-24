//! Expression parsing.
//!
//! This module extends Parser with methods for parsing expressions,
//! including binary operators, unary operators, function calls,
//! lambda expressions, and primary expressions.

use crate::ir::{
    BinaryOp, BindingPattern, CallArg, Expr, ExprId, ExprKind, ExprRange,
    FunctionExp, FunctionExpKind, FunctionSeq, MatchArm, MatchPattern,
    NamedExpr, Param, ParamRange, SeqBinding, TokenKind, UnaryOp,
};
use crate::parser::{ParseError, Parser};

impl<'a> Parser<'a> {
    /// Parse an expression.
    /// Handles assignment at the top level: `identifier = expression`
    pub(crate) fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
        let left = self.parse_binary_or()?;

        // Check for assignment (= but not == or =>)
        if self.check(TokenKind::Eq) {
            let left_span = self.arena.get_expr(left).span;
            self.advance();
            let right = self.parse_expr()?;
            let right_span = self.arena.get_expr(right).span;
            let span = left_span.merge(right_span);
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Assign { target: left, value: right },
                span,
            )));
        }

        Ok(left)
    }

    /// Parse || (lowest precedence binary).
    fn parse_binary_or(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_binary_and()?;

        while self.check(TokenKind::PipePipe) {
            self.advance();
            let right = self.parse_binary_and()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::Or, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse && (logical and)
    fn parse_binary_and(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_or()?;

        while self.check(TokenKind::AmpAmp) {
            self.advance();
            let right = self.parse_bitwise_or()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::And, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse | (bitwise or)
    fn parse_bitwise_or(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_xor()?;

        while self.check(TokenKind::Pipe) {
            self.advance();
            let right = self.parse_bitwise_xor()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitOr, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse ^ (bitwise xor)
    fn parse_bitwise_xor(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_and()?;

        while self.check(TokenKind::Caret) {
            self.advance();
            let right = self.parse_bitwise_and()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitXor, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse & (bitwise and)
    fn parse_bitwise_and(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_equality()?;

        while self.check(TokenKind::Amp) {
            self.advance();
            let right = self.parse_equality()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitAnd, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse == and != (equality)
    fn parse_equality(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_comparison()?;

        while let Some(op) = self.match_equality_op() {
            self.advance();
            let right = self.parse_comparison()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse comparison operators (<, >, <=, >=).
    fn parse_comparison(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_range()?;

        while let Some(op) = self.match_comparison_op() {
            self.advance();
            let right = self.parse_range()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse range operators (.. and ..=).
    fn parse_range(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_shift()?;

        // Check for range operator
        if self.check(TokenKind::DotDot) || self.check(TokenKind::DotDotEq) {
            let inclusive = self.check(TokenKind::DotDotEq);
            self.advance();

            // Parse the end of the range (optional for open-ended ranges like 1..)
            let end = if self.check(TokenKind::Comma) || self.check(TokenKind::RParen) ||
                        self.check(TokenKind::RBracket) || self.is_at_end() {
                None
            } else {
                Some(self.parse_shift()?)
            };

            let span = if let Some(end_expr) = end {
                self.arena.get_expr(left).span.merge(self.arena.get_expr(end_expr).span)
            } else {
                self.arena.get_expr(left).span.merge(self.previous_span())
            };

            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Range { start: Some(left), end, inclusive },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse << and >> (shift operators).
    fn parse_shift(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_additive()?;

        while let Some(op) = self.match_shift_op() {
            self.advance();
            let right = self.parse_additive()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse + and -.
    fn parse_additive(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_multiplicative()?;

        while let Some(op) = self.match_additive_op() {
            self.advance();
            let right = self.parse_multiplicative()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse *, /, %.
    fn parse_multiplicative(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_unary()?;

        while let Some(op) = self.match_multiplicative_op() {
            self.advance();
            let right = self.parse_unary()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse unary operators.
    fn parse_unary(&mut self) -> Result<ExprId, ParseError> {
        if let Some(op) = self.match_unary_op() {
            let start = self.current_span();
            self.advance();
            let operand = self.parse_unary()?;

            let span = start.merge(self.arena.get_expr(operand).span);
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Unary { op, operand },
                span,
            )));
        }

        self.parse_call()
    }

    /// Parse function calls and field access.
    fn parse_call(&mut self) -> Result<ExprId, ParseError> {
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
                        crate::diagnostic::ErrorCode::E1011,
                        "function calls with multiple arguments require named arguments (.name: value)".to_string(),
                        call_span,
                    ));
                }

                // Choose representation based on whether we have named args
                if has_named {
                    // Use CallNamed for any call with named args
                    let args_range = self.arena.alloc_call_args(call_args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::CallNamed { func: expr, args: args_range },
                        call_span,
                    ));
                } else {
                    // Simple positional call (0 or 1 args)
                    let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                    let args_range = self.arena.alloc_expr_list(args);
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Call { func: expr, args: args_range },
                        call_span,
                    ));
                }
            } else if self.check(TokenKind::Dot) {
                // Field access
                self.advance();
                let field = self.expect_ident()?;

                let span = self.arena.get_expr(expr).span.merge(self.previous_span());
                expr = self.arena.alloc_expr(Expr::new(
                    ExprKind::Field { receiver: expr, field },
                    span,
                ));
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
                // Only valid if expr is a single identifier
                let expr_data = self.arena.get_expr(expr);
                if let ExprKind::Ident(name) = &expr_data.kind {
                    let param_span = expr_data.span;
                    let param_name = *name;
                    self.advance(); // consume ->
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

    /// Parse function_seq: run or try with sequential bindings and statements.
    /// Grammar: run(let x = a, x = x + 1, result) or try(let x = fallible()?, Ok(x))
    fn parse_function_seq(&mut self, is_try: bool) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span(); // span of 'run' or 'try'
        self.expect(TokenKind::LParen)?;
        self.skip_newlines();

        let mut bindings = Vec::new();
        let mut result_expr = None;

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            // Check if this is a let binding
            if self.check(TokenKind::Let) {
                let binding_span = self.current_span();
                self.advance(); // consume 'let'

                // Check for 'mut'
                let mutable = if self.check(TokenKind::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };

                // Parse binding pattern
                let pattern = self.parse_binding_pattern()?;

                // Optional type annotation
                let ty = if self.check(TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else {
                    None
                };

                // Expect '='
                self.expect(TokenKind::Eq)?;

                // Parse value
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
                // Parse an expression
                let expr_span = self.current_span();
                let expr = self.parse_expr()?;
                let end_span = self.arena.get_expr(expr).span;

                self.skip_newlines();

                // Check what comes after to determine if this is a statement or result
                if self.check(TokenKind::Comma) {
                    self.advance(); // consume comma
                    self.skip_newlines();

                    // If the next token is ), this was a trailing comma and expr is the result
                    if self.check(TokenKind::RParen) {
                        result_expr = Some(expr);
                    } else {
                        // There's more content, so this is a statement expression
                        bindings.push(SeqBinding::Stmt {
                            expr,
                            span: expr_span.merge(end_span),
                        });
                    }
                    continue;
                } else {
                    // No comma, this is the result expression
                    result_expr = Some(expr);
                }
            }

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(TokenKind::RParen)?;
        let end_span = self.previous_span();

        // Result expression is required
        let result = result_expr.ok_or_else(|| {
            ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("{} requires a result expression", if is_try { "try" } else { "run" }),
                end_span,
            )
        })?;

        let bindings_range = self.arena.alloc_seq_bindings(bindings);
        let span = start_span.merge(end_span);
        let func_seq = if is_try {
            FunctionSeq::Try { bindings: bindings_range, result, span }
        } else {
            FunctionSeq::Run { bindings: bindings_range, result, span }
        };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionSeq(func_seq),
            span,
        )))
    }

    /// Parse match as function_seq: match(scrutinee, Pattern -> expr, ...)
    fn parse_match_expr(&mut self) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span(); // span of 'match'
        self.expect(TokenKind::LParen)?;
        self.skip_newlines();

        // First argument is the scrutinee
        let scrutinee = self.parse_expr()?;

        self.skip_newlines();
        self.expect(TokenKind::Comma)?;
        self.skip_newlines();

        // Parse match arms: Pattern -> expr
        let mut arms = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arm_span = self.current_span();
            let pattern = self.parse_match_pattern()?;

            self.expect(TokenKind::Arrow)?;
            let body = self.parse_expr()?;
            let end_span = self.arena.get_expr(body).span;

            arms.push(MatchArm {
                pattern,
                guard: None, // TODO: add guard support
                body,
                span: arm_span.merge(end_span),
            });

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(TokenKind::RParen)?;
        let end_span = self.previous_span();

        if arms.is_empty() {
            return Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                "match requires at least one arm".to_string(),
                end_span,
            ));
        }

        let arms_range = self.arena.alloc_arms(arms);
        let span = start_span.merge(end_span);
        let func_seq = FunctionSeq::Match { scrutinee, arms: arms_range, span };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionSeq(func_seq),
            span,
        )))
    }

    /// Parse a match pattern (for match arms).
    fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        match self.current_kind() {
            TokenKind::Underscore => {
                self.advance();
                Ok(MatchPattern::Wildcard)
            }
            TokenKind::Int(n) => {
                self.advance();
                Ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Int(n),
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
                // Check if this is a variant pattern like Some(x) or just a binding
                if self.check(TokenKind::LParen) {
                    // Variant pattern with optional inner pattern
                    self.advance();
                    let inner = if self.check(TokenKind::RParen) {
                        None
                    } else {
                        let pat = self.parse_match_pattern()?;
                        Some(Box::new(pat))
                    };
                    self.expect(TokenKind::RParen)?;
                    Ok(MatchPattern::Variant { name, inner })
                } else {
                    // Simple binding
                    Ok(MatchPattern::Binding(name))
                }
            }
            // Option variants
            TokenKind::Some => {
                let name = self.interner().intern("Some");
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = if self.check(TokenKind::RParen) {
                    None
                } else {
                    let pat = self.parse_match_pattern()?;
                    Some(Box::new(pat))
                };
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::None => {
                let name = self.interner().intern("None");
                self.advance();
                Ok(MatchPattern::Variant { name, inner: None })
            }
            // Result variants
            TokenKind::Ok => {
                let name = self.interner().intern("Ok");
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = if self.check(TokenKind::RParen) {
                    None
                } else {
                    let pat = self.parse_match_pattern()?;
                    Some(Box::new(pat))
                };
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::Err => {
                let name = self.interner().intern("Err");
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = if self.check(TokenKind::RParen) {
                    None
                } else {
                    let pat = self.parse_match_pattern()?;
                    Some(Box::new(pat))
                };
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Variant { name, inner })
            }
            TokenKind::LParen => {
                // Tuple pattern
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    patterns.push(self.parse_match_pattern()?);
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(MatchPattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("expected match pattern, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    /// Parse function_exp: map, filter, fold, etc. with named properties.
    /// Grammar: kind(.prop1: expr1, .prop2: expr2, ...)
    fn parse_function_exp(&mut self, kind: FunctionExpKind) -> Result<ExprId, ParseError> {
        let start_span = self.previous_span(); // span of the keyword
        self.expect(TokenKind::LParen)?;
        self.skip_newlines();

        let mut props = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            // Require named property: .name: expr
            if !self.check(TokenKind::Dot) {
                return Err(ParseError::new(
                    crate::diagnostic::ErrorCode::E1013,
                    format!("`{}` requires named properties (.name: value)", kind.name()),
                    self.current_span(),
                ));
            }

            self.advance(); // consume '.'
            let name = self.expect_ident_or_keyword()?;
            let prop_span = self.previous_span();
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            let end_span = self.arena.get_expr(value).span;

            props.push(NamedExpr {
                name,
                value,
                span: prop_span.merge(end_span),
            });

            self.skip_newlines();

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
                self.skip_newlines();
            }
        }

        self.skip_newlines();
        self.expect(TokenKind::RParen)?;
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

    /// Parse call arguments, supporting both positional and named args.
    /// Returns (args, has_positional, has_named).
    fn parse_call_args(&mut self) -> Result<(Vec<CallArg>, bool, bool), ParseError> {
        let mut args = Vec::new();
        let mut has_positional = false;
        let mut has_named = false;

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            self.skip_newlines();

            let arg_span = self.current_span();

            // Check for named argument: .name: expr
            if self.check(TokenKind::Dot) {
                self.advance(); // consume '.'
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
                // Positional argument
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

    /// Parse primary expressions.
    fn parse_primary(&mut self) -> Result<ExprId, ParseError> {
        let span = self.current_span();

        // function_seq keywords (run, try)
        if let Some(is_try) = self.match_function_seq_kind() {
            self.advance();
            return self.parse_function_seq(is_try);
        }

        // match is also function_seq but parsed separately
        if self.check(TokenKind::Match) {
            self.advance();
            return self.parse_match_expr();
        }

        // function_exp keywords (map, filter, fold, etc.)
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

            // Identifier
            TokenKind::Ident(name) => {
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Soft keywords used as identifiers (when not followed by `(`)
            // These are context-sensitive: `len(` is a built-in call, but `let len = 5` is a variable
            TokenKind::Len | TokenKind::Min | TokenKind::Max | TokenKind::Compare |
            TokenKind::IsEmpty | TokenKind::IsSome | TokenKind::IsNone |
            TokenKind::IsOk | TokenKind::IsErr | TokenKind::Print | TokenKind::Panic |
            TokenKind::Assert | TokenKind::AssertEq | TokenKind::AssertNe => {
                // This branch is only reached when NOT followed by `(`, since
                // match_function_exp_kind handles the `keyword(` case first.
                let name_str = self.soft_keyword_to_name().expect("soft keyword matched but not in helper");
                let name = self.interner().intern(name_str);
                self.advance();
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // self - used in recurse pattern for recursive calls
            TokenKind::SelfLower => {
                self.advance();
                let name = self.interner().intern("self");
                Ok(self.arena.alloc_expr(Expr::new(ExprKind::Ident(name), span)))
            }

            // Type keywords used as function_val conversion functions: int(x), float(x), str(x), etc.
            // Per spec, these are prelude functions that can be called in expression context.
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

            // Variant constructors: Some(x), None, Ok(x), Err(x)
            TokenKind::Some => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let inner = self.parse_expr()?;
                let end_span = self.current_span();
                self.expect(TokenKind::RParen)?;
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
                let inner = if self.check(TokenKind::LParen) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
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
                let inner = if self.check(TokenKind::LParen) {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
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
            TokenKind::LParen => {
                self.advance();

                // Case 1: () -> body (lambda with no params)
                if self.check(TokenKind::RParen) {
                    self.advance();

                    // Check for arrow to determine if lambda or unit
                    if self.check(TokenKind::Arrow) {
                        self.advance();
                        // Optional return type
                        let ret_ty = if self.check_type_keyword() {
                            let ty = self.parse_type();
                            self.expect(TokenKind::Eq)?;
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

                    // Unit: ()
                    let end_span = self.previous_span();
                    return Ok(self.arena.alloc_expr(Expr::new(
                        ExprKind::Tuple(ExprRange::EMPTY),
                        span.merge(end_span),
                    )));
                }

                // Case 2: Check for typed lambda params: (ident : Type, ...)
                // If we see "ident :" pattern, parse as lambda params
                if self.is_typed_lambda_params() {
                    let params = self.parse_params()?;
                    self.expect(TokenKind::RParen)?;
                    self.expect(TokenKind::Arrow)?;

                    // Optional return type: (x: int) -> int = body
                    let ret_ty = if self.check_type_keyword() {
                        let ty = self.parse_type();
                        self.expect(TokenKind::Eq)?;
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

                // Case 3: Untyped - parse as expression(s), then check for ->
                let expr = self.parse_expr()?;

                if self.check(TokenKind::Comma) {
                    // Tuple or untyped multi-param lambda
                    let mut exprs = vec![expr];
                    while self.check(TokenKind::Comma) {
                        self.advance();
                        if self.check(TokenKind::RParen) {
                            break;
                        }
                        exprs.push(self.parse_expr()?);
                    }
                    self.expect(TokenKind::RParen)?;

                    // Check for arrow - if present, convert to lambda
                    if self.check(TokenKind::Arrow) {
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

                self.expect(TokenKind::RParen)?;

                // Check for arrow - single param untyped lambda: (x) -> body
                if self.check(TokenKind::Arrow) {
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

            // List literal
            TokenKind::LBracket => {
                self.advance();
                let mut exprs = Vec::new();

                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    exprs.push(self.parse_expr()?);
                    if !self.check(TokenKind::RBracket) {
                        self.expect(TokenKind::Comma)?;
                    }
                }

                self.expect(TokenKind::RBracket)?;
                let end_span = self.previous_span();
                let range = self.arena.alloc_expr_list(exprs);
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::List(range),
                    span.merge(end_span),
                )))
            }

            // If expression
            TokenKind::If => {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(TokenKind::Then)?;
                let then_branch = self.parse_expr()?;

                // Skip newlines before checking for else (allows multiline if-else)
                self.skip_newlines();

                let else_branch = if self.check(TokenKind::Else) {
                    self.advance();
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

            // Let expression: let [mut] pattern [: type] = init
            TokenKind::Let => {
                self.advance();

                // Check for 'mut' keyword
                let mutable = if self.check(TokenKind::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };

                // Parse binding pattern (simplified to just name for now)
                let pattern = self.parse_binding_pattern()?;

                // Optional type annotation
                let ty = if self.check(TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else {
                    None
                };

                // Expect '='
                self.expect(TokenKind::Eq)?;

                // Parse initializer
                let init = self.parse_expr()?;

                let end_span = self.arena.get_expr(init).span;
                Ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Let { pattern, ty, init, mutable },
                    span.merge(end_span),
                )))
            }

            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("expected expression, found {:?}", self.current_kind()),
                span,
            )),
        }
    }

    // =========================================================================
    // Operator Matching Helpers
    // =========================================================================


    fn match_equality_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::EqEq => Some(BinaryOp::Eq),
            TokenKind::NotEq => Some(BinaryOp::NotEq),
            _ => None,
        }
    }

    fn match_comparison_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Lt => Some(BinaryOp::Lt),
            TokenKind::LtEq => Some(BinaryOp::LtEq),
            TokenKind::Gt => Some(BinaryOp::Gt),
            TokenKind::GtEq => Some(BinaryOp::GtEq),
            _ => None,
        }
    }

    fn match_shift_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Shl => Some(BinaryOp::Shl),
            TokenKind::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    fn match_additive_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        }
    }

    fn match_multiplicative_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Mod),
            _ => None,
        }
    }

    fn match_unary_op(&self) -> Option<UnaryOp> {
        match self.current_kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        }
    }

    /// Match function_seq keywords. Returns Some(true) for try, Some(false) for run.
    fn match_function_seq_kind(&self) -> Option<bool> {
        match self.current_kind() {
            TokenKind::Run => Some(false),
            TokenKind::Try => Some(true),
            _ => None,
        }
    }

    /// Match function_exp keywords.
    fn match_function_exp_kind(&self) -> Option<FunctionExpKind> {
        // Pattern keywords are always keywords (map, filter, fold, etc.)
        match self.current_kind() {
            TokenKind::Map => return Some(FunctionExpKind::Map),
            TokenKind::Filter => return Some(FunctionExpKind::Filter),
            TokenKind::Fold => return Some(FunctionExpKind::Fold),
            TokenKind::Find => return Some(FunctionExpKind::Find),
            TokenKind::Collect => return Some(FunctionExpKind::Collect),
            TokenKind::Recurse => return Some(FunctionExpKind::Recurse),
            TokenKind::Parallel => return Some(FunctionExpKind::Parallel),
            TokenKind::Spawn => return Some(FunctionExpKind::Spawn),
            TokenKind::Timeout => return Some(FunctionExpKind::Timeout),
            TokenKind::Retry => return Some(FunctionExpKind::Retry),
            TokenKind::Cache => return Some(FunctionExpKind::Cache),
            TokenKind::Validate => return Some(FunctionExpKind::Validate),
            TokenKind::With => return Some(FunctionExpKind::With),
            _ => {}
        }

        // Built-in functions are context-sensitive: only keywords when followed by `(`
        // This allows `let len = 5` while still supporting `len(.collection: x)`
        if !self.next_is_lparen() {
            return None;
        }

        match self.current_kind() {
            TokenKind::Assert => Some(FunctionExpKind::Assert),
            TokenKind::AssertEq => Some(FunctionExpKind::AssertEq),
            TokenKind::AssertNe => Some(FunctionExpKind::AssertNe),
            TokenKind::Len => Some(FunctionExpKind::Len),
            TokenKind::IsEmpty => Some(FunctionExpKind::IsEmpty),
            TokenKind::IsSome => Some(FunctionExpKind::IsSome),
            TokenKind::IsNone => Some(FunctionExpKind::IsNone),
            TokenKind::IsOk => Some(FunctionExpKind::IsOk),
            TokenKind::IsErr => Some(FunctionExpKind::IsErr),
            TokenKind::Print => Some(FunctionExpKind::Print),
            TokenKind::Panic => Some(FunctionExpKind::Panic),
            TokenKind::Compare => Some(FunctionExpKind::Compare),
            TokenKind::Min => Some(FunctionExpKind::Min),
            TokenKind::Max => Some(FunctionExpKind::Max),
            _ => None,
        }
    }

    // =========================================================================
    // Expression Helper Methods
    // =========================================================================

    /// Parse a binding pattern (for let expressions).
    ///
    /// Currently supports:
    /// - Simple name: `x`
    /// - Wildcard: `_`
    /// - Tuple: `(a, b, c)`
    fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        // Check for soft keywords first (len, min, max, etc. can be used as variable names)
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
                // Tuple pattern
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    patterns.push(self.parse_binding_pattern()?);
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(BindingPattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                crate::diagnostic::ErrorCode::E1002,
                format!("expected binding pattern, found {:?}", self.current_kind()),
                self.current_span(),
            )),
        }
    }

    /// Check if we're looking at typed lambda params: (ident : Type, ...)
    /// This looks ahead to detect the "ident :" pattern that distinguishes
    /// typed lambda params from expressions.
    fn is_typed_lambda_params(&self) -> bool {
        // Look at first token - must be an identifier or soft keyword
        let is_ident_like = matches!(self.current_kind(), TokenKind::Ident(_))
            || self.soft_keyword_to_name().is_some();
        if !is_ident_like {
            return false;
        }
        // Look at second token - must be a colon
        self.next_is_colon()
    }

    /// Convert parsed expressions to lambda parameters.
    /// Each expression must be an identifier.
    fn exprs_to_params(&mut self, exprs: &[ExprId]) -> Result<ParamRange, ParseError> {
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
                        crate::diagnostic::ErrorCode::E1002,
                        "expected identifier for lambda parameter".to_string(),
                        expr.span,
                    ));
                }
            }
        }
        Ok(self.arena.alloc_params(params))
    }
}
