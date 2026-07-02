//! Collection and grouping primary expression parsing.
//!
//! Handles parenthesized expressions (tuples, lambdas, grouped) and list
//! literals. Brace-delimited forms (block expressions, map literals) live in
//! [`super::block_map`].

use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, ExprRange, ParamRange, ParsedTypeId, Span, TokenKind};

// Lambda return type parsing

impl Parser<'_> {
    /// Speculatively try to parse a return type annotation (`TYPE =`) in a lambda.
    ///
    /// After `->` in a lambda, the grammar allows an optional `TYPE =` before the
    /// body expression. This method tries to parse a type; if followed by `=`, it
    /// consumes both and returns the allocated type ID. Otherwise, it restores the
    /// cursor position and returns `INVALID`.
    ///
    /// This handles all type forms: primitives (`int`), function types (`(int) -> int`),
    /// list types (`[int]`), named types (`MyType`), etc.
    fn try_parse_lambda_return_type(&mut self) -> ParsedTypeId {
        let snapshot = self.snapshot();
        if let Some(ty) = self.parse_type() {
            if self.cursor.check(&TokenKind::Eq) {
                self.cursor.advance(); // consume `=`
                return self.arena.alloc_parsed_type(ty);
            }
        }
        // Not a return type annotation — restore and treat as body expression.
        // Arena nodes from the failed parse_type() are harmless (freed with arena).
        self.restore(snapshot);
        ParsedTypeId::INVALID
    }
}

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

    fn parse_parenthesized_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        self.cursor.advance(); // (
        self.cursor.skip_newlines();

        // Case 1: () -> body (lambda with no params)
        if self.cursor.check(&TokenKind::RParen) {
            self.cursor.advance();

            if self.cursor.check(&TokenKind::Arrow) {
                self.cursor.advance();
                let ret_ty = self.try_parse_lambda_return_type();
                self.cursor.skip_newlines(); // lambda body may follow `->`/`=` on the next line
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
        //
        // Spec: grammar.ebnf §lambda — `(` typed_param { `,` typed_param } `)`
        // `->` lambda_tail, where lambda_tail is `type "=" expression` (explicit
        // return) or `expression` (inferred return). parse_typed_lambda_body
        // disambiguates via the `=` delimiter. is_typed_lambda_params() only
        // matches on the first param's `:` shape; mixed forms like `(a: int, b)`
        // reach this branch and are rejected by the helper.
        if self.is_typed_lambda_params() {
            return self.parse_typed_lambda_body(span);
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
                self.cursor.skip_newlines(); // lambda body may follow `->` on the next line
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
            self.cursor.skip_newlines(); // lambda body may follow `->` on the next line
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

    /// Parse the typed-parameter-lambda branch of `parse_parenthesized_body`.
    ///
    /// Spec: `grammar.ebnf` §`lambda` / §`lambda_tail` — `(` `typed_param`
    /// { `,` `typed_param` } `)` `->` ( type `=` expression | expression ). The
    /// return type is explicit when `=` follows it, else inferred (`ret_ty` =
    /// `ParsedTypeId::INVALID`). All params MUST be typed; untyped params
    /// produce `E1018`.
    fn parse_typed_lambda_body(&mut self, start_span: Span) -> ParseOutcome<ExprId> {
        let params = committed!(self.parse_params());

        let params_slice = self.arena.get_params(params);
        for param in params_slice {
            if param.ty.is_none() {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1018,
                        "untyped parameter in typed lambda",
                        param.span,
                    )
                    .with_help(
                        "typed lambdas require all params typed: `(a: T, b: U) -> R = body`; for an untyped lambda drop all type annotations: `(a, b) -> body`",
                    ),
                    start_span,
                );
            }
        }

        committed!(self.cursor.expect(&TokenKind::RParen));
        committed!(self.cursor.expect(&TokenKind::Arrow));

        // Disambiguate explicit-return (`type = body`) from inferred-return
        // (`body`) via the `=` delimiter — shared with the `()`-lambda path.
        // Spec: grammar.ebnf §lambda_tail.
        let ret_ty = self.try_parse_lambda_return_type();

        self.cursor.skip_newlines(); // typed-lambda body may follow `=` on the next line
        let body = require!(self, self.parse_expr(), "lambda body");
        let end_span = self.arena.get_expr(body).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            },
            start_span.merge(end_span),
        )))
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
}

#[cfg(test)]
mod tests;
