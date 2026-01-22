//! Expression parsing: operators, calls, lambdas, control flow.

use crate::errors::Diagnostic;
use crate::syntax::{
    TokenKind,
    Expr, ExprKind, ExprId, ExprRange, PatternKind,
    BinaryOp, UnaryOp,
    expr::{Param, MapEntry, PatternArgs, PatternArg, BindingPattern},
};
use super::Parser;

impl<'src, 'i> Parser<'src, 'i> {
    /// Parse any expression.
    pub(crate) fn expression(&mut self) -> Result<ExprId, Diagnostic> {
        self.parse_precedence(14) // Lowest precedence (assignment level)
    }

    fn parse_precedence(&mut self, max_prec: u8) -> Result<ExprId, Diagnostic> {
        let mut left = self.unary()?;

        // Check for lambda: x -> body or (a, b) -> body
        if self.check(&TokenKind::Arrow) {
            return self.parse_lambda(left);
        }

        // Check for assignment: target = value
        // Only parse assignment at the lowest precedence level (14)
        if max_prec >= 14 && self.check(&TokenKind::Eq) {
            self.advance();
            self.skip_newlines();
            let value = self.expression()?;
            let span = self.arena.get(left).span.merge(self.arena.get(value).span);
            return Ok(self.arena.alloc(Expr::new(
                ExprKind::Assign { target: left, value },
                span,
            )));
        }

        while let Some((op, prec)) = self.binary_op() {
            if prec > max_prec {
                break;
            }

            self.advance();
            self.skip_newlines();

            let right = if op.is_left_assoc() {
                self.parse_precedence(prec - 1)?
            } else {
                self.parse_precedence(prec)?
            };

            let span = self.arena.get(left).span.merge(self.arena.get(right).span);
            left = self.arena.alloc(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    fn parse_lambda(&mut self, params_expr: ExprId) -> Result<ExprId, Diagnostic> {
        let start = self.arena.get(params_expr).span;
        self.consume(&TokenKind::Arrow, "expected '->'")?;
        self.skip_newlines();

        let params = self.expr_to_lambda_params(params_expr)?;
        let params_range = self.arena.alloc_params(params);

        let body = self.expression()?;
        let span = start.merge(self.arena.get(body).span);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Lambda {
                params: params_range,
                ret_ty: None,
                body,
            },
            span,
        )))
    }

    fn expr_to_lambda_params(&self, expr: ExprId) -> Result<Vec<Param>, Diagnostic> {
        let expr_data = self.arena.get(expr);
        match &expr_data.kind {
            ExprKind::Ident(name) => Ok(vec![Param {
                name: *name,
                ty: None,
                default: None,
                span: expr_data.span,
            }]),

            ExprKind::Tuple(elements) => {
                let mut params = Vec::new();
                for elem_id in self.arena.get_expr_list(*elements) {
                    let elem = self.arena.get(*elem_id);
                    match &elem.kind {
                        ExprKind::Ident(name) => {
                            params.push(Param {
                                name: *name,
                                ty: None,
                                default: None,
                                span: elem.span,
                            });
                        }
                        _ => {
                            return Err(Diagnostic {
                                severity: crate::errors::Severity::Error,
                                code: None,
                                message: "expected parameter name in lambda".to_string(),
                                span: elem.span,
                                labels: vec![],
                                notes: vec![],
                                suggestions: vec![],
                            });
                        }
                    }
                }
                Ok(params)
            }

            ExprKind::Unit => Ok(vec![]),

            _ => Err(Diagnostic {
                severity: crate::errors::Severity::Error,
                code: None,
                message: "invalid lambda parameters".to_string(),
                span: expr_data.span,
                labels: vec![],
                notes: vec![],
                suggestions: vec![],
            }),
        }
    }

    fn binary_op(&self) -> Option<(BinaryOp, u8)> {
        let op = match self.current_kind() {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Mod,
            TokenKind::Div => BinaryOp::FloorDiv,
            TokenKind::EqEq => BinaryOp::Eq,
            TokenKind::NotEq => BinaryOp::Ne,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::LtEq => BinaryOp::Le,
            TokenKind::Shl => BinaryOp::Shl,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::GtEq => BinaryOp::Ge,
            TokenKind::Shr => BinaryOp::Shr,
            TokenKind::AmpAmp => BinaryOp::And,
            TokenKind::PipePipe => BinaryOp::Or,
            TokenKind::Amp => BinaryOp::BitAnd,
            TokenKind::Pipe => BinaryOp::BitOr,
            TokenKind::Caret => BinaryOp::BitXor,
            TokenKind::DotDot => BinaryOp::Range,
            TokenKind::DotDotEq => BinaryOp::RangeInc,
            TokenKind::DoubleQuestion => BinaryOp::Coalesce,
            _ => return None,
        };
        Some((op, op.precedence()))
    }

    fn unary(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();

        let op = match self.current_kind() {
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        };

        if let Some(op) = op {
            self.advance();
            let operand = self.unary()?;
            let span = start.merge(self.arena.get(operand).span);
            Ok(self.arena.alloc(Expr::new(
                ExprKind::Unary { op, operand },
                span,
            )))
        } else {
            self.postfix()
        }
    }

    fn postfix(&mut self) -> Result<ExprId, Diagnostic> {
        let mut expr = self.primary()?;

        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    self.advance();
                    let field = self.parse_name()?;

                    if self.check(&TokenKind::LParen) {
                        self.advance();
                        let args = self.parse_call_args()?;
                        self.consume(&TokenKind::RParen, "expected ')'")?;
                        let span = self.arena.get(expr).span.merge(self.current_span());
                        expr = self.arena.alloc(Expr::new(
                            ExprKind::MethodCall {
                                receiver: expr,
                                method: field,
                                args,
                            },
                            span,
                        ));
                    } else {
                        let span = self.arena.get(expr).span.merge(self.current_span());
                        expr = self.arena.alloc(Expr::new(
                            ExprKind::Field { receiver: expr, field },
                            span,
                        ));
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.expression()?;
                    self.consume(&TokenKind::RBracket, "expected ']'")?;
                    let span = self.arena.get(expr).span.merge(self.current_span());
                    expr = self.arena.alloc(Expr::new(
                        ExprKind::Index { receiver: expr, index },
                        span,
                    ));
                }
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    let span = self.arena.get(expr).span.merge(self.current_span());
                    expr = self.arena.alloc(Expr::new(
                        ExprKind::Call { func: expr, args },
                        span,
                    ));
                }
                TokenKind::Question => {
                    self.advance();
                    let span = self.arena.get(expr).span.merge(self.current_span());
                    expr = self.arena.alloc(Expr::new(ExprKind::Try(expr), span));
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<ExprId, Diagnostic> {
        let span = self.current_span();

        match self.current_kind().clone() {
            // Literals
            TokenKind::Int(n) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Int(n), span)))
            }
            TokenKind::Float(bits) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Float(f64::from_bits(bits)), span)))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::String(name), span)))
            }
            TokenKind::Char(c) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Char(c), span)))
            }
            TokenKind::True => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Bool(true), span)))
            }
            TokenKind::False => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Bool(false), span)))
            }
            TokenKind::Duration(value, unit) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Duration { value, unit },
                    span,
                )))
            }
            TokenKind::Size(value, unit) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Size { value, unit },
                    span,
                )))
            }

            // Identifiers and references
            TokenKind::Ident(name) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::At => {
                self.advance();
                let name = self.parse_name()?;
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::FunctionRef(name),
                    span.merge(end_span),
                )))
            }
            TokenKind::Dollar => {
                self.advance();
                let name = self.parse_name()?;
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Config(name),
                    span.merge(end_span),
                )))
            }
            TokenKind::SelfLower => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::SelfRef, span)))
            }
            TokenKind::Hash => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::HashLength, span)))
            }

            // Constructors
            TokenKind::Ok => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = self.expression()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Some(expr)
                } else {
                    None
                };
                Ok(self.arena.alloc(Expr::new(ExprKind::Ok(inner), span)))
            }
            TokenKind::Err => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = self.expression()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Some(expr)
                } else {
                    None
                };
                Ok(self.arena.alloc(Expr::new(ExprKind::Err(inner), span)))
            }
            TokenKind::Some => {
                self.advance();
                self.consume(&TokenKind::LParen, "expected '('")?;
                let inner = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')'")?;
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Some(inner),
                    span.merge(end_span),
                )))
            }
            TokenKind::None => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::None, span)))
            }

            // Control flow
            TokenKind::If => self.parse_if(),
            TokenKind::For => self.parse_for(),
            TokenKind::Loop => self.parse_loop(),
            TokenKind::Let => self.parse_let(),
            TokenKind::Break => {
                self.advance();
                // Optional break value
                let value = if !self.at_end() && !self.check(&TokenKind::Newline) &&
                    !self.check(&TokenKind::Comma) && !self.check(&TokenKind::RParen) &&
                    !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::RBracket) &&
                    !self.check(&TokenKind::Else) {
                    // Check if the next token could start an expression
                    match self.current_kind() {
                        TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::String(_) |
                        TokenKind::Char(_) | TokenKind::True | TokenKind::False |
                        TokenKind::Ident(_) | TokenKind::LParen | TokenKind::LBracket |
                        TokenKind::LBrace => Some(self.expression()?),
                        _ => None,
                    }
                } else {
                    None
                };
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(ExprKind::Break(value), span.merge(end_span))))
            }
            TokenKind::Continue => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Continue, span)))
            }

            // Pattern expressions
            TokenKind::Run => self.parse_pattern(PatternKind::Run),
            TokenKind::Try => self.parse_pattern(PatternKind::Try),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Map => self.parse_pattern(PatternKind::Map),
            TokenKind::Filter => self.parse_pattern(PatternKind::Filter),
            TokenKind::Fold => self.parse_pattern(PatternKind::Fold),
            TokenKind::Find => self.parse_pattern(PatternKind::Find),
            TokenKind::Collect => self.parse_pattern(PatternKind::Collect),
            TokenKind::Recurse => self.parse_pattern(PatternKind::Recurse),
            TokenKind::Parallel => self.parse_pattern(PatternKind::Parallel),
            TokenKind::Timeout => self.parse_pattern(PatternKind::Timeout),
            TokenKind::Retry => self.parse_pattern(PatternKind::Retry),
            TokenKind::Cache => self.parse_pattern(PatternKind::Cache),
            TokenKind::Validate => self.parse_pattern(PatternKind::Validate),

            // Collections
            TokenKind::LBracket => self.parse_list(),
            TokenKind::LBrace => self.parse_map_or_struct(),
            TokenKind::LParen => self.parse_paren_or_tuple(),

            // Builtin functions that are lexed as keywords
            TokenKind::Assert => {
                let name = self.interner.intern("assert");
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::StrType => {
                let name = self.interner.intern("str");
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::IntType => {
                let name = self.interner.intern("int");
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::FloatType => {
                let name = self.interner.intern("float");
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Ident(name), span)))
            }

            _ => Err(self.error("expected expression")),
        }
    }

    // ===== Control flow =====

    fn parse_if(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::If, "expected 'if'")?;

        let cond = self.expression()?;
        self.consume(&TokenKind::Then, "expected 'then'")?;
        self.skip_newlines();

        let then_branch = self.expression()?;

        let else_branch = if self.check(&TokenKind::Else) {
            self.advance();
            self.skip_newlines();
            Some(self.expression()?)
        } else {
            None
        };

        let end_span = else_branch
            .map(|e| self.arena.get(e).span)
            .unwrap_or(self.arena.get(then_branch).span);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::If { cond, then_branch, else_branch },
            start.merge(end_span),
        )))
    }

    fn parse_for(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::For, "expected 'for'")?;

        let binding = self.parse_name()?;
        self.consume(&TokenKind::In, "expected 'in'")?;
        let iter = self.expression()?;

        let guard = if self.check(&TokenKind::If) {
            self.advance();
            Some(self.expression()?)
        } else {
            None
        };

        let is_yield = if self.check(&TokenKind::Do) {
            self.advance();
            false
        } else if self.check(&TokenKind::Yield) {
            self.advance();
            true
        } else {
            return Err(self.error("expected 'do' or 'yield'"));
        };

        self.skip_newlines();
        let body = self.expression()?;
        let end_span = self.arena.get(body).span;

        Ok(self.arena.alloc(Expr::new(
            ExprKind::For { binding, iter, guard, body, is_yield },
            start.merge(end_span),
        )))
    }

    fn parse_loop(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Loop, "expected 'loop'")?;
        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        let body = self.expression()?;

        self.skip_newlines();
        self.consume(&TokenKind::RParen, "expected ')'")?;

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Loop { body },
            start.merge(self.current_span()),
        )))
    }

    fn parse_let(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Let, "expected 'let'")?;

        let mutable = if self.check(&TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };

        let pattern = self.parse_binding_pattern()?;

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let init = self.expression()?;
        let end_span = self.arena.get(init).span;

        let ty_id = ty.map(|t| self.arena.alloc_type_expr(t));
        Ok(self.arena.alloc(Expr::new(
            ExprKind::Let {
                pattern,
                ty: ty_id,
                init,
                mutable,
            },
            start.merge(end_span),
        )))
    }

    // ===== Patterns =====

    fn parse_pattern(&mut self, kind: PatternKind) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.advance(); // Consume pattern keyword

        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        let args = self.parse_pattern_args(kind)?;

        self.skip_newlines();
        self.consume(&TokenKind::RParen, "expected ')'")?;

        let args_id = self.arena.alloc_pattern_args(args);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Pattern { kind, args: args_id },
            start.merge(self.current_span()),
        )))
    }

    fn parse_pattern_args(&mut self, kind: PatternKind) -> Result<PatternArgs, Diagnostic> {
        let start = self.current_span();
        let mut named = Vec::new();
        let mut positional = Vec::new();

        let is_positional = matches!(kind, PatternKind::Run);

        loop {
            if self.check(&TokenKind::RParen) {
                break;
            }

            if is_positional || !self.check(&TokenKind::Dot) {
                // Use try_expression for positional argument recovery
                let expr = self.try_expression();
                positional.push(expr);
            } else {
                // For named args, try to parse the full structure
                // If any part fails, skip to next comma or closing paren
                match self.parse_named_pattern_arg(start) {
                    Ok(arg) => named.push(arg),
                    Err(diag) => {
                        self.diagnostics.push(diag);
                        self.recover_to_expr_sync();
                    }
                }
            }

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        let positional_range = self.arena.alloc_expr_list(positional);

        Ok(PatternArgs {
            named,
            positional: positional_range,
            span: start.merge(self.current_span()),
        })
    }

    /// Parse a named pattern argument: .name: value
    fn parse_named_pattern_arg(&mut self, start: crate::syntax::Span) -> Result<PatternArg, Diagnostic> {
        self.consume(&TokenKind::Dot, "expected '.'")?;
        let name = self.parse_name()?;
        self.consume(&TokenKind::Colon, "expected ':'")?;
        self.skip_newlines();
        let value = self.expression()?;
        let arg_span = start.merge(self.arena.get(value).span);
        Ok(PatternArg { name, value, span: arg_span })
    }

    // ===== Collections =====

    fn parse_list(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::LBracket, "expected '['")?;
        self.skip_newlines();

        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) && !self.at_end() {
            // Use try_expression for recovery - continue parsing on error
            elements.push(self.try_expression());

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        self.consume(&TokenKind::RBracket, "expected ']'")?;
        let range = self.arena.alloc_expr_list(elements);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::List(range),
            start.merge(self.current_span()),
        )))
    }

    fn parse_map_or_struct(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::LBrace, "expected '{'")?;
        self.skip_newlines();

        if self.check(&TokenKind::RBrace) {
            self.advance();
            return Ok(self.arena.alloc(Expr::new(
                ExprKind::Map(crate::syntax::MapEntryRange::EMPTY),
                start.merge(self.current_span()),
            )));
        }

        let mut entries = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.at_end() {
            // Use try_expression for key recovery
            let key = self.try_expression();

            if self.check(&TokenKind::Colon) {
                self.advance();
                self.skip_newlines();
                // Use try_expression for value recovery
                let value = self.try_expression();
                let entry_span = self.arena.get(key).span.merge(self.arena.get(value).span);
                entries.push(MapEntry { key, value, span: entry_span });
            } else {
                let span = self.arena.get(key).span;
                entries.push(MapEntry { key, value: key, span });
            }

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        let range = self.arena.alloc_map_entries(entries);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Map(range),
            start.merge(self.current_span()),
        )))
    }

    fn parse_paren_or_tuple(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        if self.check(&TokenKind::RParen) {
            self.advance();
            return Ok(self.arena.alloc(Expr::new(
                ExprKind::Unit,
                start.merge(self.current_span()),
            )));
        }

        // Use try_expression for first element recovery
        let first = self.try_expression();

        if self.check(&TokenKind::Comma) {
            let mut elements = vec![first];
            while self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
                if self.check(&TokenKind::RParen) {
                    break;
                }
                // Use try_expression for subsequent elements
                elements.push(self.try_expression());
            }
            self.consume(&TokenKind::RParen, "expected ')'")?;
            let range = self.arena.alloc_expr_list(elements);
            Ok(self.arena.alloc(Expr::new(
                ExprKind::Tuple(range),
                start.merge(self.current_span()),
            )))
        } else {
            self.consume(&TokenKind::RParen, "expected ')'")?;
            Ok(first)
        }
    }

    pub(crate) fn parse_call_args(&mut self) -> Result<ExprRange, Diagnostic> {
        self.skip_newlines();
        let mut args = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.at_end() {
            // Use try_expression for argument recovery
            args.push(self.try_expression());

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        Ok(self.arena.alloc_expr_list(args))
    }

    pub(crate) fn parse_binding_pattern(&mut self) -> Result<BindingPattern, Diagnostic> {
        match self.current_kind().clone() {
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

                while !self.check(&TokenKind::RParen) && !self.at_end() {
                    patterns.push(self.parse_binding_pattern()?);
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RParen, "expected ')'")?;
                Ok(BindingPattern::Tuple(patterns))
            }
            TokenKind::LBrace => {
                self.advance();
                let mut fields = Vec::new();

                while !self.check(&TokenKind::RBrace) && !self.at_end() {
                    let name = self.parse_name()?;
                    let pattern = if self.check(&TokenKind::Colon) {
                        self.advance();
                        Some(self.parse_binding_pattern()?)
                    } else {
                        None
                    };
                    fields.push((name, pattern));

                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RBrace, "expected '}'")?;
                Ok(BindingPattern::Struct { fields })
            }
            TokenKind::LBracket => {
                // List destructuring: [a, b] or [head, ..tail]
                self.advance();
                let mut elements = Vec::new();
                let mut rest = None;

                while !self.check(&TokenKind::RBracket) && !self.at_end() {
                    // Check for rest pattern: ..name
                    if self.check(&TokenKind::DotDot) {
                        self.advance();
                        let rest_name = self.parse_name()?;
                        rest = Some(rest_name);
                        // Rest pattern must be last
                        break;
                    }

                    elements.push(self.parse_binding_pattern()?);

                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(BindingPattern::List { elements, rest })
            }
            _ => Err(self.error("expected binding pattern")),
        }
    }
}
