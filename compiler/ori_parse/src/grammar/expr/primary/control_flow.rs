//! Control flow primary expression parsing.
//!
//! Handles break, continue, return, if/then/else, loop, and for loop expressions.

use crate::context::ParseContext;
use crate::recovery::TokenSet;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, Name, TokenKind};

/// Tokens that start a control flow expression.
const CONTROL_FLOW_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Break)
    .with(TokenKind::Continue)
    .with(TokenKind::Return);

impl Parser<'_> {
    /// Parse optional label: `:identifier` (no space around colon).
    ///
    /// Called immediately after consuming the keyword (`break`, `continue`, `for`, `loop`).
    /// Returns `Name::EMPTY` if no label is present.
    fn parse_optional_label(&mut self) -> Name {
        if self.cursor.check(&TokenKind::Colon) && self.cursor.current_flags().is_adjacent() {
            self.cursor.advance(); // consume ':'
            match self.cursor.expect_ident() {
                Ok(name) => name,
                Err(err) => {
                    self.deferred_errors
                        .push(err.with_context("expected label identifier after `:`"));
                    Name::EMPTY
                }
            }
        } else {
            Name::EMPTY
        }
    }

    /// Parse control flow primaries: `break`, `continue`, `return`.
    ///
    /// Returns `EmptyErr` if the current token is not a control flow keyword.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive control flow keyword dispatch with argument parsing"
    )]
    pub(super) fn parse_control_flow_primary(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        match *self.cursor.current_kind() {
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
                self.cursor.advance();
                let label = self.parse_optional_label();
                let value = if !self.cursor.check(&TokenKind::Comma)
                    && !self.cursor.check(&TokenKind::RParen)
                    && !self.cursor.check(&TokenKind::RBrace)
                    && !self.cursor.check(&TokenKind::RBracket)
                    && !self.cursor.check(&TokenKind::Newline)
                    && !self.cursor.check(&TokenKind::Semicolon)
                    && !self.cursor.check(&TokenKind::Else)
                    && !self.cursor.check(&TokenKind::Then)
                    && !self.cursor.check(&TokenKind::Do)
                    && !self.cursor.check(&TokenKind::Yield)
                    && !self.cursor.is_at_end()
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
                ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Break { label, value },
                    span.merge(end_span),
                )))
            }
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
                self.cursor.advance();
                let label = self.parse_optional_label();
                let value = if !self.cursor.check(&TokenKind::Comma)
                    && !self.cursor.check(&TokenKind::RParen)
                    && !self.cursor.check(&TokenKind::RBrace)
                    && !self.cursor.check(&TokenKind::RBracket)
                    && !self.cursor.check(&TokenKind::Newline)
                    && !self.cursor.check(&TokenKind::Semicolon)
                    && !self.cursor.check(&TokenKind::Else)
                    && !self.cursor.check(&TokenKind::Then)
                    && !self.cursor.check(&TokenKind::Do)
                    && !self.cursor.check(&TokenKind::Yield)
                    && !self.cursor.is_at_end()
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
                ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                    ExprKind::Continue { label, value },
                    span.merge(end_span),
                )))
            }
            TokenKind::Return => {
                self.cursor.advance();
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
            _ => ParseOutcome::empty_err(
                CONTROL_FLOW_TOKENS,
                self.cursor.current_span().start as usize,
            ),
        }
    }

    /// Parse if expression.
    ///
    /// Guard: returns `EmptyErr` if not at `if`.
    pub(super) fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::If) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::If,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(crate::ErrorContext::IfExpression, Self::parse_if_expr_body)
    }

    fn parse_if_expr_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        self.cursor.advance();

        // Parse condition without struct literals (for consistency and future safety).
        // While Ori uses `then` instead of `{` after conditions, disallowing struct
        // literals in conditions is a common pattern that prevents potential ambiguities.
        let cond = require!(
            self,
            self.with_context(ParseContext::NO_STRUCT_LIT, Self::parse_expr),
            "condition in if expression"
        );

        committed!(self.cursor.expect(&TokenKind::Then));
        self.cursor.skip_newlines();
        let then_branch = require!(self, self.parse_expr(), "then branch");

        self.cursor.skip_newlines();

        let else_branch = if self.cursor.check(&TokenKind::Else) {
            self.cursor.advance();
            self.cursor.skip_newlines();
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

    /// Parse for loop: `for x in items do body` or `for x in items yield body`
    ///
    /// Also supports optional guard: `for x in items if condition do body`
    ///
    /// Guard: returns `EmptyErr` if not at `for`.
    pub(super) fn parse_for_loop(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::For) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::For,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(crate::ErrorContext::ForLoop, Self::parse_for_loop_body)
    }

    fn parse_for_loop_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::For));

        // Parse optional label: for:label
        let label = self.parse_optional_label();

        // Parse binding name or wildcard (_)
        let binding = if self.cursor.check(&TokenKind::Underscore) {
            self.cursor.advance();
            self.cursor.interner().intern("_")
        } else {
            committed!(self.cursor.expect_ident())
        };

        // Expect `in` keyword
        committed!(self.cursor.expect(&TokenKind::In));

        // Parse iterator expression
        let iter = require!(self, self.parse_expr(), "iterator expression");

        // Check for optional guard: `if condition`
        let guard = if self.cursor.check(&TokenKind::If) {
            self.cursor.advance();
            require!(self, self.parse_expr(), "guard condition")
        } else {
            ExprId::INVALID
        };

        // Expect `do` or `yield`
        let is_yield = if self.cursor.check(&TokenKind::Do) {
            self.cursor.advance();
            false
        } else if self.cursor.check(&TokenKind::Yield) {
            self.cursor.advance();
            true
        } else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "expected `do` or `yield` after for loop iterator",
                    self.cursor.current_span(),
                ),
                span,
            );
        };

        self.cursor.skip_newlines();

        // Parse body expression with IN_LOOP context (enables break/continue)
        let body = require!(
            self,
            self.with_context(ParseContext::IN_LOOP, Self::parse_expr),
            "loop body"
        );

        let end_span = self.arena.get_expr(body).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::For {
                label,
                binding,
                iter,
                guard,
                body,
                is_yield,
            },
            span.merge(end_span),
        )))
    }

    /// Parse loop expression: `loop { body }`.
    ///
    /// Supports optional label: `loop:label { body }`.
    /// The body is evaluated repeatedly until a `break` is encountered.
    ///
    /// Guard: returns `EmptyErr` if not at `loop`.
    pub(super) fn parse_loop_expr(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::Loop) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Loop,
                self.cursor.current_span().start as usize,
            );
        }

        let span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Loop));

        let label = self.parse_optional_label();

        if self.cursor.check(&TokenKind::LParen) {
            let paren_span = self.cursor.current_span();
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "`loop()` syntax has been removed",
                    paren_span,
                )
                .with_help("Use block syntax instead: `loop { break value }`"),
                span,
            );
        }

        // loop { body }
        let body = require!(
            self,
            self.with_context(ParseContext::IN_LOOP, Self::parse_expr),
            "loop body"
        );

        let end_span = self.arena.get_expr(body).span;

        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Loop { label, body },
            span.merge(end_span),
        )))
    }
}
