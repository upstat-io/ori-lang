//! Collection and grouping primary expression parsing.
//!
//! Handles parenthesized expressions (tuples, lambdas, grouped),
//! list literals, block expressions, and map literals.

use crate::{committed, require, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, ExprRange, ParamRange, ParsedTypeId, TokenKind};

impl Parser<'_> {
    /// Parse parenthesized expression, tuple, or lambda.
    ///
    /// Guard: returns `EmptyErr` if not at `(`.
    pub(super) fn parse_parenthesized(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::LParen) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LParen,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(
            crate::ErrorContext::Expression,
            Self::parse_parenthesized_body,
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "multi-case parenthesized expression parser covering tuples, lambdas, and grouped expressions"
    )]
    fn parse_parenthesized_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        self.cursor.advance(); // (
        self.cursor.skip_newlines();

        // Case 1: () -> body (lambda with no params)
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();

            if self.cursor.check(&TokenKind::Arrow) {
                self.cursor.advance();
                let ret_ty = if self.cursor.check_type_keyword() {
                    let ty = self.parse_type();
                    committed!(self.cursor.expect(&TokenKind::Eq));
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

            let end_span = self.cursor.previous_span();
            return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Tuple(ExprRange::EMPTY),
                span.merge(end_span),
            )));
        }

        // Case 2: Typed lambda params
        if self.is_typed_lambda_params() {
            let params = committed!(self.parse_params());
            committed!(self.cursor.expect(&TokenKind::RParen));
            committed!(self.cursor.expect(&TokenKind::Arrow));

            let ret_ty = if self.cursor.check_type_keyword() {
                let ty = self.parse_type();
                committed!(self.cursor.expect(&TokenKind::Eq));
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

        self.cursor.skip_newlines();
        if self.cursor.check(&TokenKind::Comma) {
            let mut exprs = vec![expr];
            while self.cursor.check(&TokenKind::Comma) {
                self.cursor.advance();
                self.cursor.skip_newlines();
                if self.cursor.check(&TokenKind::RParen) {
                    break;
                }
                exprs.push(require!(self, self.parse_expr(), "expression in tuple"));
                self.cursor.skip_newlines();
            }
            committed!(self.cursor.expect(&TokenKind::RParen));

            if self.cursor.check(&TokenKind::Arrow) {
                self.cursor.advance();
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

            let end_span = self.cursor.previous_span();
            let list = self.arena.alloc_expr_list_inline(&exprs);
            return ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::Tuple(list), span.merge(end_span))),
            );
        }

        committed!(self.cursor.expect(&TokenKind::RParen));

        if self.cursor.check(&TokenKind::Arrow) {
            self.cursor.advance();
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
    ///
    /// Guard: returns `EmptyErr` if not at `[`.
    pub(super) fn parse_list_literal(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::LBracket) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LBracket,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(
            crate::ErrorContext::ListLiteral,
            Self::parse_list_literal_body,
        )
    }

    fn parse_list_literal_body(&mut self) -> ParseOutcome<ExprId> {
        use ori_ir::ListElement;

        let span = self.cursor.current_span();
        self.cursor.advance(); // [

        // List elements use a Vec because nested lists share the same
        // `list_elements` buffer, causing same-buffer nesting conflicts
        // with direct arena push. The Vec overhead is acceptable since
        // list literals are less frequent than params/arms/generics.
        let mut has_spread = false;
        let mut elements: Vec<ListElement> = Vec::new();

        committed!(self.bracket_series_direct(|p| {
            if p.cursor.check(&TokenKind::RBracket) {
                return Ok(false);
            }

            let elem_span = p.cursor.current_span();
            if p.cursor.check(&TokenKind::DotDotDot) {
                // Spread element: ...expr
                p.cursor.advance(); // consume ...
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
            Ok(true)
        }));

        let end_span = self.cursor.previous_span();
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

    /// Disambiguate `{` — block expression vs map literal.
    ///
    /// Uses lookahead after `{` (skipping newlines) to decide:
    /// - `{ }` → empty map literal
    /// - `{ ident :` → map literal (key-value)
    /// - `{ "string" :` → map literal (string key)
    /// - `{ [ ...` → map literal (computed key)
    /// - `{ ... ident` → map literal (spread)
    /// - Everything else → block expression
    ///
    /// Guard: returns `EmptyErr` if not at `{`.
    pub(super) fn parse_block_or_map(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::LBrace) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LBrace,
                self.cursor.current_span().start as usize,
            );
        }

        if self.is_map_literal_start() {
            self.in_error_context(
                crate::ErrorContext::MapLiteral,
                Self::parse_map_literal_body,
            )
        } else {
            self.in_error_context(crate::ErrorContext::Expression, Self::parse_block_expr_body)
        }
    }

    /// Determine whether `{ ... }` is a map literal or a block expression.
    ///
    /// Peeks past `{` and any newlines to examine the first meaningful token(s).
    /// Returns `true` if this looks like a map literal.
    fn is_map_literal_start(&self) -> bool {
        // Skip `{` and any newlines to find the first meaningful token
        let mut offset = 1;
        while matches!(self.cursor.peek_kind_at(offset), TokenKind::Newline) {
            offset += 1;
        }

        let first = self.cursor.peek_kind_at(offset);

        match first {
            // `{ }` → empty map, `{ ... expr` → map spread
            TokenKind::RBrace | TokenKind::DotDotDot => true,

            // Tokens that could be map keys if followed by `:`.
            // `{ ident :` → map with identifier key
            // `{ "string" :` → map with string key
            // `{ 42 :` → map with integer key
            // `{ 'a' :` → map with char key
            // `{ true :` → map with bool key
            TokenKind::Ident(_)
            | TokenKind::String(_)
            | TokenKind::Int(_)
            | TokenKind::Char(_)
            | TokenKind::True
            | TokenKind::False => self.peek_colon_after(offset),

            // `{ [expr] :` → map where the key is a bracket expression.
            // Scan for matching `]` then check if `:` follows.
            TokenKind::LBracket => {
                let mut depth = 1u32;
                let mut scan = offset + 1;
                loop {
                    match self.cursor.peek_kind_at(scan) {
                        TokenKind::LBracket => {
                            depth += 1;
                            scan += 1;
                        }
                        TokenKind::RBracket => {
                            depth -= 1;
                            if depth == 0 {
                                scan += 1;
                                break;
                            }
                            scan += 1;
                        }
                        TokenKind::Eof => return false,
                        _ => scan += 1,
                    }
                }
                // Skip newlines after `]`, then check for `:`
                while matches!(self.cursor.peek_kind_at(scan), TokenKind::Newline) {
                    scan += 1;
                }
                matches!(self.cursor.peek_kind_at(scan), TokenKind::Colon)
            }

            // Everything else → block expression
            _ => false,
        }
    }

    /// Check if a colon follows the token at `offset` (skipping newlines).
    ///
    /// Used by `is_map_literal_start()` to detect `key:` patterns.
    fn peek_colon_after(&self, token_offset: usize) -> bool {
        let mut next = token_offset + 1;
        while matches!(self.cursor.peek_kind_at(next), TokenKind::Newline) {
            next += 1;
        }
        matches!(self.cursor.peek_kind_at(next), TokenKind::Colon)
    }

    /// Parse block expression body: `{ stmt; stmt; result }`.
    ///
    /// Produces `ExprKind::Block { stmts, result }`. The last expression without
    /// a trailing `;` becomes the result (block value). If all expressions have `;`,
    /// the result is `ExprId::INVALID` (unit block).
    fn parse_block_expr_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        self.cursor.advance(); // consume `{`

        let (stmts_vec, result, end_span) =
            require!(self, self.collect_block_stmts("block"), "block body");

        // Batch-push all statements after nested parsing is complete.
        // (Collected into Vec first to avoid interleaving with nested blocks
        // that share the same arena stmt list.)
        let stmt_start = self.arena.start_stmts();
        for stmt in stmts_vec {
            self.arena.push_stmt(stmt);
        }
        let stmts = self.arena.finish_stmts(stmt_start);

        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Block { stmts, result },
            span.merge(end_span),
        )))
    }

    fn parse_map_literal_body(&mut self) -> ParseOutcome<ExprId> {
        use ori_ir::{MapElement, MapEntry};

        let span = self.cursor.current_span();
        self.cursor.advance(); // {

        // Map elements use a Vec because nested maps share the same
        // `map_elements` buffer, causing same-buffer nesting conflicts
        // with direct arena push. Same reasoning as list literals.
        let mut has_spread = false;
        let mut elements: Vec<MapElement> = Vec::new();

        committed!(self.brace_series_direct(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(false);
            }

            let elem_span = p.cursor.current_span();
            if p.cursor.check(&TokenKind::DotDotDot) {
                // Spread element: ...expr
                p.cursor.advance(); // consume ...
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
                p.cursor.expect(&TokenKind::Colon)?;
                let value = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(value).span;
                elements.push(MapElement::Entry(MapEntry {
                    key,
                    value,
                    span: elem_span.merge(end_span),
                }));
            }
            Ok(true)
        }));

        let end_span = self.cursor.previous_span();
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
}
