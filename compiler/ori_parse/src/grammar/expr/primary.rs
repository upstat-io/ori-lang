//! Primary Expression Parsing
//!
//! Parses literals, identifiers, variant constructors, parenthesized expressions,
//! lists, if expressions, and let expressions.

use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    BindingPattern, Expr, ExprId, ExprKind, ExprRange, Param, ParamRange, ParsedTypeId, TokenKind,
};

impl Parser<'_> {
    /// Parse primary expressions with outcome tracking.
    ///
    /// Returns `EmptyErr` if the current token is not a valid expression start
    /// (no tokens consumed — enables backtracking in `one_of!` chains).
    /// Returns `ConsumedOk` on successful parse, `ConsumedErr` on error after
    /// consuming tokens.
    pub(crate) fn parse_primary(&mut self) -> ParseOutcome<ExprId> {
        let span = self.current_span();

        // function_seq keywords (run, try)
        if self.check(&TokenKind::Run) {
            self.advance();
            return self.parse_run();
        }
        if self.check(&TokenKind::Try) {
            self.advance();
            return self.parse_try();
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

        // for loop: for x in items do body  OR  for x in items yield body
        if self.check(&TokenKind::For) {
            return self.parse_for_loop();
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

        match *self.current_kind() {
            // Literals
            TokenKind::Int(n) => {
                self.advance();
                let Ok(value) = i64::try_from(n) else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large".to_string(),
                            span,
                        ),
                        span,
                    );
                };
                ParseOutcome::consumed_ok(
                    self.arena.alloc_expr(Expr::new(ExprKind::Int(value), span)),
                )
            }
            TokenKind::Float(bits) => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Float(bits), span)),
                )
            }
            TokenKind::True => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena.alloc_expr(Expr::new(ExprKind::Bool(true), span)),
                )
            }
            TokenKind::False => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Bool(false), span)),
                )
            }
            TokenKind::String(name) => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::String(name), span)),
                )
            }
            TokenKind::Char(c) => {
                self.advance();
                ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(ExprKind::Char(c), span)))
            }
            TokenKind::Duration(value, unit) => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Duration { value, unit }, span)),
                )
            }
            TokenKind::Size(value, unit) => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Size { value, unit }, span)),
                )
            }

            // Constant reference: $name
            TokenKind::Dollar => {
                self.advance();
                let name = committed!(self.expect_ident());
                let full_span = span.merge(self.previous_span());
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Const(name), full_span)),
                )
            }

            // Hash length symbol: # (only valid inside index brackets)
            TokenKind::Hash => {
                if self.context.in_index() {
                    self.advance();
                    ParseOutcome::consumed_ok(
                        self.arena.alloc_expr(Expr::new(ExprKind::HashLength, span)),
                    )
                } else {
                    // Don't advance — this isn't a valid expression start in this context.
                    // Return EmptyErr so backtracking can try alternatives.
                    ParseOutcome::empty_err_expected(&TokenKind::Hash, self.position())
                }
            }

            // Identifier
            TokenKind::Ident(name) => {
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }

            // Built-in I/O primitives as soft keywords
            TokenKind::Print => {
                let name = self.interner().intern("print");
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::Panic => {
                let name = self.interner().intern("panic");
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::Catch => {
                let name = self.interner().intern("catch");
                self.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }

            // self - identifier for method receiver or recursive self-reference
            TokenKind::SelfLower => {
                self.advance();
                let name = self.interner().intern("self");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }

            // Type conversion functions
            TokenKind::IntType => {
                self.advance();
                let name = self.interner().intern("int");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::FloatType => {
                self.advance();
                let name = self.interner().intern("float");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::StrType => {
                self.advance();
                let name = self.interner().intern("str");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::BoolType => {
                self.advance();
                let name = self.interner().intern("bool");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::CharType => {
                self.advance();
                let name = self.interner().intern("char");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::ByteType => {
                self.advance();
                let name = self.interner().intern("byte");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }

            // Context-sensitive keywords usable as identifiers when not followed by (
            TokenKind::Timeout => {
                self.advance();
                let name = self.interner().intern("timeout");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::Parallel => {
                self.advance();
                let name = self.interner().intern("parallel");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::Cache => {
                self.advance();
                let name = self.interner().intern("cache");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::Spawn => {
                self.advance();
                let name = self.interner().intern("spawn");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            TokenKind::Recurse => {
                self.advance();
                let name = self.interner().intern("recurse");
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                )
            }
            // Variant constructors
            TokenKind::Some => {
                self.advance();
                committed!(self.expect(&TokenKind::LParen));
                let inner = require!(self, self.parse_expr(), "expression inside `Some(...)`");
                let end_span = self.current_span();
                committed!(self.expect(&TokenKind::RParen));
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Some(inner), span.merge(end_span))),
                )
            }
            TokenKind::None => {
                self.advance();
                ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(ExprKind::None, span)))
            }
            TokenKind::Ok => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = require!(self, self.parse_expr(), "expression inside `Ok(...)`");
                    committed!(self.expect(&TokenKind::RParen));
                    expr
                } else {
                    ExprId::INVALID
                };
                let end_span = self.previous_span();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ok(inner), span.merge(end_span))),
                )
            }
            TokenKind::Err => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = require!(self, self.parse_expr(), "expression inside `Err(...)`");
                    committed!(self.expect(&TokenKind::RParen));
                    expr
                } else {
                    ExprId::INVALID
                };
                let end_span = self.previous_span();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Err(inner), span.merge(end_span))),
                )
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

            // Break expression (only valid inside loops)
            TokenKind::Break => {
                if !self.context.in_loop() {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "`break` outside of loop",
                            span,
                        )
                        .with_context("break can only be used inside a loop or for expression"),
                        span,
                    );
                }
                self.advance();
                // Optional value: break or break value
                // Value is present if there's an expression following
                // Terminators: delimiters, control-flow keywords, newlines, EOF
                let value = if !self.check(&TokenKind::Comma)
                    && !self.check(&TokenKind::RParen)
                    && !self.check(&TokenKind::RBrace)
                    && !self.check(&TokenKind::RBracket)
                    && !self.check(&TokenKind::Newline)
                    && !self.check(&TokenKind::Else)
                    && !self.check(&TokenKind::Then)
                    && !self.check(&TokenKind::Do)
                    && !self.check(&TokenKind::Yield)
                    && !self.is_at_end()
                {
                    require!(self, self.parse_expr(), "expression after `break`")
                } else {
                    ExprId::INVALID
                };
                let end_span = if value.is_present() {
                    self.arena.get_expr(value).span
                } else {
                    span
                };
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Break(value), span.merge(end_span))),
                )
            }

            // Continue expression (only valid inside loops)
            TokenKind::Continue => {
                if !self.context.in_loop() {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "`continue` outside of loop",
                            span,
                        )
                        .with_context("continue can only be used inside a loop or for expression"),
                        span,
                    );
                }
                self.advance();
                // Optional value: continue or continue value (valid in for...yield)
                // Terminators: delimiters, control-flow keywords, newlines, EOF
                let value = if !self.check(&TokenKind::Comma)
                    && !self.check(&TokenKind::RParen)
                    && !self.check(&TokenKind::RBrace)
                    && !self.check(&TokenKind::RBracket)
                    && !self.check(&TokenKind::Newline)
                    && !self.check(&TokenKind::Else)
                    && !self.check(&TokenKind::Then)
                    && !self.check(&TokenKind::Do)
                    && !self.check(&TokenKind::Yield)
                    && !self.is_at_end()
                {
                    require!(self, self.parse_expr(), "expression after `continue`")
                } else {
                    ExprId::INVALID
                };
                let end_span = if value.is_present() {
                    self.arena.get_expr(value).span
                } else {
                    span
                };
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Continue(value), span.merge(end_span))),
                )
            }

            // Return is not valid in Ori - provide helpful error for users from other languages
            TokenKind::Return => {
                self.advance();
                ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1015,
                        "`return` is not valid in Ori",
                        span,
                    )
                    .with_context(
                        "Ori is expression-based: the last expression in a block is its value",
                    )
                    .with_help("For early error exit, use the `?` operator: `let x = fallible()?`")
                    .with_help("For loop exit with value, use `break value`"),
                    span,
                )
            }

            // Loop expression: loop(body)
            TokenKind::Loop => self.parse_loop_expr(),

            // Float with duration suffix is an error (e.g., 1.5s, 2.5ms)
            // Spec: duration-size-types-proposal.md § Numeric Prefix
            // "Floating-point prefixes are NOT supported"
            TokenKind::FloatDurationError => {
                self.advance();
                ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E0911,
                        "floating-point duration literal not supported",
                        span,
                    )
                    .with_context(
                        "use integer with smaller unit (e.g., `1500ms` instead of `1.5s`)",
                    ),
                    span,
                )
            }

            // Float with size suffix is an error (e.g., 1.5kb, 2.5mb)
            TokenKind::FloatSizeError => {
                self.advance();
                ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E0911,
                        "floating-point size literal not supported",
                        span,
                    )
                    .with_context(
                        "use integer with smaller unit (e.g., `1536kb` instead of `1.5mb`)",
                    ),
                    span,
                )
            }

            _ => ParseOutcome::empty_err_expected(
                &TokenKind::Ident(ori_ir::Name::EMPTY),
                self.position(),
            ),
        }
    }

    /// Parse parenthesized expression, tuple, or lambda.
    fn parse_parenthesized(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(
            crate::ErrorContext::Expression,
            Self::parse_parenthesized_body,
        )
    }

    fn parse_parenthesized_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.current_span();
        self.advance(); // (
        self.skip_newlines();

        // Case 1: () -> body (lambda with no params)
        if self.check(&TokenKind::RParen) {
            self.advance();

            if self.check(&TokenKind::Arrow) {
                self.advance();
                let ret_ty = if self.check_type_keyword() {
                    let ty = self.parse_type();
                    committed!(self.expect(&TokenKind::Eq));
                    ty.map_or(ParsedTypeId::INVALID, |t| self.arena.alloc_parsed_type(t))
                } else {
                    ParsedTypeId::INVALID
                };
                let body = require!(self, self.parse_expr(), "lambda body");
                let end_span = self.arena.get_expr(body).span;
                return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Lambda {
                        params: ParamRange::EMPTY,
                        ret_ty,
                        body,
                    },
                    span.merge(end_span),
                )));
            }

            let end_span = self.previous_span();
            return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Tuple(ExprRange::EMPTY),
                span.merge(end_span),
            )));
        }

        // Case 2: Typed lambda params
        if self.is_typed_lambda_params() {
            let params = committed!(self.parse_params());
            committed!(self.expect(&TokenKind::RParen));
            committed!(self.expect(&TokenKind::Arrow));

            let ret_ty = if self.check_type_keyword() {
                let ty = self.parse_type();
                committed!(self.expect(&TokenKind::Eq));
                ty.map_or(ParsedTypeId::INVALID, |t| self.arena.alloc_parsed_type(t))
            } else {
                ParsedTypeId::INVALID
            };

            let body = require!(self, self.parse_expr(), "lambda body");
            let end_span = self.arena.get_expr(body).span;
            return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Lambda {
                    params,
                    ret_ty,
                    body,
                },
                span.merge(end_span),
            )));
        }

        // Case 3: Untyped - parse as expression(s)
        let expr = require!(self, self.parse_expr(), "expression");

        self.skip_newlines();
        if self.check(&TokenKind::Comma) {
            let mut exprs = vec![expr];
            while self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
                if self.check(&TokenKind::RParen) {
                    break;
                }
                exprs.push(require!(self, self.parse_expr(), "expression in tuple"));
                self.skip_newlines();
            }
            committed!(self.expect(&TokenKind::RParen));

            if self.check(&TokenKind::Arrow) {
                self.advance();
                let params = committed!(self.exprs_to_params(&exprs));
                let body = require!(self, self.parse_expr(), "lambda body");
                let end_span = self.arena.get_expr(body).span;
                return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Lambda {
                        params,
                        ret_ty: ParsedTypeId::INVALID,
                        body,
                    },
                    span.merge(end_span),
                )));
            }

            let end_span = self.previous_span();
            let list = self.arena.alloc_expr_list_inline(&exprs);
            return ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::Tuple(list), span.merge(end_span))),
            );
        }

        committed!(self.expect(&TokenKind::RParen));

        if self.check(&TokenKind::Arrow) {
            self.advance();
            let params = committed!(self.exprs_to_params(&[expr]));
            let body = require!(self, self.parse_expr(), "lambda body");
            let end_span = self.arena.get_expr(body).span;
            return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Lambda {
                    params,
                    ret_ty: ParsedTypeId::INVALID,
                    body,
                },
                span.merge(end_span),
            )));
        }

        ParseOutcome::consumed_ok(expr)
    }

    /// Parse list literal.
    fn parse_list_literal(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(
            crate::ErrorContext::ListLiteral,
            Self::parse_list_literal_body,
        )
    }

    fn parse_list_literal_body(&mut self) -> ParseOutcome<ExprId> {
        use ori_ir::ListElement;

        let span = self.current_span();
        self.advance(); // [

        // Track whether we see any spread elements
        let mut has_spread = false;
        let mut elements: Vec<ListElement> = Vec::new();

        committed!(self.bracket_series(|p| {
            if p.check(&TokenKind::RBracket) {
                return Ok(None);
            }

            let elem_span = p.current_span();
            if p.check(&TokenKind::DotDotDot) {
                // Spread element: ...expr
                p.advance(); // consume ...
                has_spread = true;
                let expr = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(expr).span;
                elements.push(ListElement::Spread {
                    expr,
                    span: elem_span.merge(end_span),
                });
            } else {
                // Regular expression element
                let expr = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(expr).span;
                elements.push(ListElement::Expr {
                    expr,
                    span: elem_span.merge(end_span),
                });
            }
            Ok(Some(()))
        }));

        let end_span = self.previous_span();
        let full_span = span.merge(end_span);

        if has_spread {
            // Use ListWithSpread for lists containing spread elements
            let range = self.arena.alloc_list_elements(elements);
            ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::ListWithSpread(range), full_span)),
            )
        } else {
            // Use optimized List for simple cases without spread
            let exprs: Vec<ExprId> = elements
                .into_iter()
                .map(|e| match e {
                    ListElement::Expr { expr, .. } => expr,
                    ListElement::Spread { .. } => unreachable!(),
                })
                .collect();
            let list = self.arena.alloc_expr_list_inline(&exprs);
            ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::List(list), full_span)),
            )
        }
    }

    /// Parse map literal: `{ key: value, ... }`, `{ ...base, key: value }`, or `{}`.
    fn parse_map_literal(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(
            crate::ErrorContext::MapLiteral,
            Self::parse_map_literal_body,
        )
    }

    fn parse_map_literal_body(&mut self) -> ParseOutcome<ExprId> {
        use ori_ir::{MapElement, MapEntry};

        let span = self.current_span();
        self.advance(); // {

        // Track whether we see any spread elements
        let mut has_spread = false;
        let mut elements: Vec<MapElement> = Vec::new();

        committed!(self.brace_series(|p| {
            if p.check(&TokenKind::RBrace) {
                return Ok(None);
            }

            let elem_span = p.current_span();
            if p.check(&TokenKind::DotDotDot) {
                // Spread element: ...expr
                p.advance(); // consume ...
                has_spread = true;
                let expr = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(expr).span;
                elements.push(MapElement::Spread {
                    expr,
                    span: elem_span.merge(end_span),
                });
            } else {
                // Regular entry: key: value
                let key = p.parse_expr().into_result()?;
                p.expect(&TokenKind::Colon)?;
                let value = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(value).span;
                elements.push(MapElement::Entry(MapEntry {
                    key,
                    value,
                    span: elem_span.merge(end_span),
                }));
            }
            Ok(Some(()))
        }));

        let end_span = self.previous_span();
        let full_span = span.merge(end_span);

        if has_spread {
            // Use MapWithSpread for maps containing spread elements
            let range = self.arena.alloc_map_elements(elements);
            ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::MapWithSpread(range), full_span)),
            )
        } else {
            // Use optimized Map for simple cases without spread
            let entries: Vec<MapEntry> = elements
                .into_iter()
                .map(|e| match e {
                    MapElement::Entry(entry) => entry,
                    MapElement::Spread { .. } => unreachable!(),
                })
                .collect();
            let range = self.arena.alloc_map_entries(entries);
            ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::Map(range), full_span)),
            )
        }
    }

    /// Parse if expression.
    fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(crate::ErrorContext::IfExpression, Self::parse_if_expr_body)
    }

    fn parse_if_expr_body(&mut self) -> ParseOutcome<ExprId> {
        use crate::ParseContext;

        let span = self.current_span();
        self.advance();

        // Parse condition without struct literals (for consistency and future safety).
        // While Ori uses `then` instead of `{` after conditions, disallowing struct
        // literals in conditions is a common pattern that prevents potential ambiguities.
        let cond = require!(
            self,
            self.with_context(ParseContext::NO_STRUCT_LIT, Self::parse_expr),
            "condition in if expression"
        );

        committed!(self.expect(&TokenKind::Then));
        self.skip_newlines();
        let then_branch = require!(self, self.parse_expr(), "then branch");

        self.skip_newlines();

        let else_branch = if self.check(&TokenKind::Else) {
            self.advance();
            self.skip_newlines();
            require!(self, self.parse_expr(), "else branch")
        } else {
            ExprId::INVALID
        };

        let end_span = if else_branch.is_present() {
            self.arena.get_expr(else_branch).span
        } else {
            self.arena.get_expr(then_branch).span
        };

        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            },
            span.merge(end_span),
        )))
    }

    /// Parse let expression.
    ///
    /// Per spec (05-variables.md): Bindings are mutable by default.
    /// - `let x = ...` → mutable (default)
    /// - `let $x = ...` → immutable ($ prefix)
    /// - `let mut x = ...` → mutable (legacy, redundant)
    fn parse_let_expr(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(crate::ErrorContext::LetPattern, Self::parse_let_expr_body)
    }

    fn parse_let_expr_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.current_span();
        self.advance();

        // Per spec: mutable by default, $ prefix for immutable
        // - `let x = ...` → mutable (default)
        // - `let $x = ...` → immutable (spec syntax)
        // - `let mut x = ...` → mutable (legacy, same as default)
        let mutable = if self.check(&TokenKind::Dollar) {
            self.advance();
            false // $ prefix means immutable
        } else if self.check(&TokenKind::Mut) {
            self.advance();
            true // mut keyword (legacy, redundant since default is mutable)
        } else {
            true // default is mutable per spec
        };

        let pattern = committed!(self.parse_binding_pattern());
        let pattern_id = self.arena.alloc_binding_pattern(pattern);

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            self.parse_type()
                .map_or(ParsedTypeId::INVALID, |t| self.arena.alloc_parsed_type(t))
        } else {
            ParsedTypeId::INVALID
        };

        committed!(self.expect(&TokenKind::Eq));
        let init = require!(self, self.parse_expr(), "initializer expression");

        let end_span = self.arena.get_expr(init).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Let {
                pattern: pattern_id,
                ty,
                init,
                mutable,
            },
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

        match *self.current_kind() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(BindingPattern::Name(name))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(BindingPattern::Wildcard)
            }
            TokenKind::LParen => {
                use crate::series::SeriesConfig;
                self.advance();
                let patterns: Vec<BindingPattern> =
                    self.series(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
                        if p.check(&TokenKind::RParen) {
                            Ok(None)
                        } else {
                            Ok(Some(p.parse_binding_pattern()?))
                        }
                    })?;
                self.expect(&TokenKind::RParen)?;
                Ok(BindingPattern::Tuple(patterns))
            }
            TokenKind::LBrace => {
                use crate::series::SeriesConfig;
                use ori_ir::Name;
                self.advance();
                let fields: Vec<(Name, Option<BindingPattern>)> =
                    self.series(&SeriesConfig::comma(TokenKind::RBrace).no_newlines(), |p| {
                        if p.check(&TokenKind::RBrace) {
                            return Ok(None);
                        }

                        let field_name = p.expect_ident()?;

                        let binding = if p.check(&TokenKind::Colon) {
                            p.advance();
                            Some(p.parse_binding_pattern()?)
                        } else {
                            None // Shorthand: { x } binds field x to variable x
                        };

                        Ok(Some((field_name, binding)))
                    })?;
                self.expect(&TokenKind::RBrace)?;
                Ok(BindingPattern::Struct { fields })
            }
            TokenKind::LBracket => {
                // List pattern is special: has optional ..rest at the end
                // Cannot use simple series combinator
                self.advance();
                let mut elements = Vec::new();
                let mut rest = None;

                while !self.check(&TokenKind::RBracket) && !self.is_at_end() {
                    if self.check(&TokenKind::DotDot) {
                        self.advance();
                        if let TokenKind::Ident(name) = *self.current_kind() {
                            rest = Some(name);
                            self.advance();
                        }
                        break;
                    }
                    elements.push(self.parse_binding_pattern()?);
                    if !self.check(&TokenKind::RBracket) && !self.check(&TokenKind::DotDot) {
                        self.expect(&TokenKind::Comma)?;
                    }
                }
                self.expect(&TokenKind::RBracket)?;
                Ok(BindingPattern::List { elements, rest })
            }
            _ => Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "expected binding pattern, found {}",
                    self.current_kind().display_name()
                ),
                self.current_span(),
            )),
        }
    }

    /// Parse capability provision: `with Capability = Provider in body`
    fn parse_with_capability(&mut self) -> ParseOutcome<ExprId> {
        let span = self.current_span();
        committed!(self.expect(&TokenKind::With));

        // Parse capability name
        let capability = committed!(self.expect_ident());

        committed!(self.expect(&TokenKind::Eq));

        // Parse provider expression
        let provider = require!(self, self.parse_expr(), "capability provider expression");

        // Expect `in` keyword
        committed!(self.expect(&TokenKind::In));
        self.skip_newlines();

        // Parse body expression
        let body = require!(self, self.parse_expr(), "body expression after `in`");

        let end_span = self.arena.get_expr(body).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            },
            span.merge(end_span),
        )))
    }

    /// Parse for loop: `for x in items do body` or `for x in items yield body`
    ///
    /// Also supports optional guard: `for x in items if condition do body`
    fn parse_for_loop(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(crate::ErrorContext::ForLoop, Self::parse_for_loop_body)
    }

    fn parse_for_loop_body(&mut self) -> ParseOutcome<ExprId> {
        use crate::context::ParseContext;

        let span = self.current_span();
        committed!(self.expect(&TokenKind::For));

        // Parse binding name or wildcard (_)
        let binding = if self.check(&TokenKind::Underscore) {
            self.advance();
            self.interner().intern("_")
        } else {
            committed!(self.expect_ident())
        };

        // Expect `in` keyword
        committed!(self.expect(&TokenKind::In));

        // Parse iterator expression
        let iter = require!(self, self.parse_expr(), "iterator expression");

        // Check for optional guard: `if condition`
        let guard = if self.check(&TokenKind::If) {
            self.advance();
            require!(self, self.parse_expr(), "guard condition")
        } else {
            ExprId::INVALID
        };

        // Expect `do` or `yield`
        let is_yield = if self.check(&TokenKind::Do) {
            self.advance();
            false
        } else if self.check(&TokenKind::Yield) {
            self.advance();
            true
        } else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "expected `do` or `yield` after for loop iterator".to_string(),
                    self.current_span(),
                ),
                span,
            );
        };

        self.skip_newlines();

        // Parse body expression with IN_LOOP context (enables break/continue)
        let body = require!(
            self,
            self.with_context(ParseContext::IN_LOOP, Self::parse_expr),
            "loop body"
        );

        let end_span = self.arena.get_expr(body).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            },
            span.merge(end_span),
        )))
    }

    /// Parse loop expression: `loop(body)`
    ///
    /// The body is evaluated repeatedly until a `break` is encountered.
    fn parse_loop_expr(&mut self) -> ParseOutcome<ExprId> {
        use crate::context::ParseContext;

        let span = self.current_span();
        committed!(self.expect(&TokenKind::Loop));
        committed!(self.expect(&TokenKind::LParen));
        self.skip_newlines();

        // Parse body expression with IN_LOOP context (enables break/continue)
        let body = require!(
            self,
            self.with_context(ParseContext::IN_LOOP, Self::parse_expr),
            "loop body"
        );

        self.skip_newlines();
        let end_span = self.current_span();
        committed!(self.expect(&TokenKind::RParen));

        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::Loop { body }, span.merge(end_span))),
        )
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
                        "expected identifier for lambda parameter".to_string(),
                        expr.span,
                    ));
                }
            }
        }
        Ok(self.arena.alloc_params(params))
    }
}
