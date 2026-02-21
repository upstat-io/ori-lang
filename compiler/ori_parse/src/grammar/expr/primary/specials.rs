//! Special primary expression parsing.
//!
//! Handles template literals, unsafe blocks, and capability provision (`with`).

use crate::recovery::TokenSet;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, Name, TemplatePart, TokenKind};

/// Tokens that start a template literal expression.
const TEMPLATE_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::TemplateFull(Name::EMPTY))
    .with(TokenKind::TemplateHead(Name::EMPTY));

impl Parser<'_> {
    /// Parse template literal: `` `text` `` or `` `text{expr}more{expr:fmt}end` ``
    ///
    /// Template literals use backticks and support interpolation with `{expr}`.
    /// An optional format spec can follow the expression: `{expr:format_spec}`.
    ///
    /// Returns `EmptyErr` if the current token is not `TemplateFull` or `TemplateHead`.
    pub(super) fn parse_template_literal(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        match *self.cursor.current_kind() {
            // No interpolation: `text`
            TokenKind::TemplateFull(name) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::TemplateFull(name), span)),
                )
            }
            // Interpolation: `head{expr}middle{expr}tail`
            TokenKind::TemplateHead(head) => {
                self.cursor.advance();

                // Template parts use a Vec because nested templates share the
                // same `template_parts` buffer, causing same-buffer nesting
                // conflicts with direct arena push.
                let mut parts = Vec::new();

                loop {
                    // Parse interpolated expression
                    let expr = require!(
                        self,
                        self.parse_expr(),
                        "expression in template interpolation"
                    );

                    // Check for optional format spec
                    let format_spec = if let TokenKind::FormatSpec(n) = *self.cursor.current_kind()
                    {
                        self.cursor.advance();
                        n
                    } else {
                        Name::EMPTY
                    };

                    // Expect TemplateMiddle or TemplateTail
                    match *self.cursor.current_kind() {
                        TokenKind::TemplateMiddle(text) => {
                            parts.push(TemplatePart {
                                expr,
                                format_spec,
                                text_after: text,
                            });
                            self.cursor.advance();
                            // Continue loop for next interpolation
                        }
                        TokenKind::TemplateTail(text) => {
                            parts.push(TemplatePart {
                                expr,
                                format_spec,
                                text_after: text,
                            });
                            let end_span = self.cursor.current_span();
                            self.cursor.advance();
                            let parts_range = self.arena.alloc_template_parts(parts);
                            return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                                ExprKind::TemplateLiteral {
                                    head,
                                    parts: parts_range,
                                },
                                span.merge(end_span),
                            )));
                        }
                        _ => {
                            // Error: expected template continuation
                            return ParseOutcome::consumed_err(
                                ParseError::new(
                                    ori_diagnostic::ErrorCode::E1002,
                                    "expected `}` to close template interpolation",
                                    self.cursor.current_span(),
                                ),
                                span,
                            );
                        }
                    }
                }
            }
            _ => {
                ParseOutcome::empty_err(TEMPLATE_TOKENS, self.cursor.current_span().start as usize)
            }
        }
    }

    /// Parse unsafe block expression: `unsafe { block_body }`.
    ///
    /// Per proposal: `unsafe` is block-only (no parenthesized form).
    /// The inner block discharges the `Unsafe` capability within its scope.
    /// Grammar: `unsafe_expr = "unsafe" block_expr .`
    pub(super) fn parse_unsafe_expr(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::Unsafe) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Unsafe,
                self.cursor.current_span().start as usize,
            );
        }
        let span = self.cursor.current_span();
        self.cursor.advance(); // consume `unsafe`

        if !self.cursor.check(&TokenKind::LBrace) {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "expected `{` after `unsafe`",
                    self.cursor.current_span(),
                )
                .with_help("Use block syntax: `unsafe { expr }`"),
                span,
            );
        }

        // Parse the block body — reuse the standard block parser
        let body = require!(self, self.parse_block_or_map(), "unsafe block body");
        let end_span = self.arena.get_expr(body).span;

        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::Unsafe(body), span.merge(end_span))),
        )
    }

    /// Parse capability provision: `with Capability = Provider in body`
    pub(super) fn parse_with_capability(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::With));

        // Parse capability name
        let capability = committed!(self.cursor.expect_ident());

        committed!(self.cursor.expect(&TokenKind::Eq));

        // Parse provider expression
        let provider = require!(self, self.parse_expr(), "capability provider expression");

        // Expect `in` keyword
        committed!(self.cursor.expect(&TokenKind::In));
        self.cursor.skip_newlines();

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
}
