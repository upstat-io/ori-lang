//! Shared block-statement parsing helpers.
//!
//! Provides [`collect_block_stmts()`] and [`parse_let_stmt()`] — shared logic
//! for parsing statement sequences inside `{ ... }` blocks. Used by both
//! `parse_block_expr_body()` (block expressions) and `parse_try_block()`
//! (try expressions) to eliminate duplicated parsing logic.

use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{BindingPattern, ExprId, Mutability, ParsedTypeId, Span, Stmt, StmtKind, TokenKind};

impl Parser<'_> {
    /// Collect statements inside a `{ ... }` block until `}`.
    ///
    /// Assumes `{` has already been consumed. Skips initial newlines,
    /// parses a sequence of let bindings and expression statements, and
    /// consumes the closing `}`.
    ///
    /// Returns `(stmts, result, end_span)` where:
    /// - `stmts`: collected statement nodes (caller must batch-push to arena)
    /// - `result`: last expression without `;`, or `ExprId::INVALID` for unit blocks
    /// - `end_span`: span of the closing `}`
    pub(super) fn collect_block_stmts(
        &mut self,
        block_name: &str,
    ) -> ParseOutcome<(Vec<Stmt>, ExprId, Span)> {
        self.cursor.skip_newlines();

        // Pre-compute require! context string once (avoids per-iteration allocation).
        let expr_context = format!("expression in {block_name}");

        let mut stmts: Vec<Stmt> = Vec::new();
        let mut last_expr: Option<ExprId> = None;

        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.check(&TokenKind::RBrace) {
                break;
            }

            let item_span = self.cursor.current_span();

            if self.cursor.check(&TokenKind::Let) {
                // Flush any pending expression as a statement
                if let Some(prev) = last_expr.take() {
                    let prev_span = self.arena.get_expr(prev).span;
                    stmts.push(Stmt::new(StmtKind::Expr(prev), prev_span));
                }

                self.cursor.advance(); // consume `let`
                match self.parse_let_stmt(item_span, block_name) {
                    Ok(stmt) => stmts.push(stmt),
                    Err(err) => return ParseOutcome::consumed_err(err, item_span),
                }
            } else {
                let expr = require!(self, self.parse_expr(), &expr_context);

                // Flush any pending expression as a statement
                if let Some(prev) = last_expr.take() {
                    let prev_span = self.arena.get_expr(prev).span;
                    stmts.push(Stmt::new(StmtKind::Expr(prev), prev_span));
                }

                self.cursor.skip_newlines();

                if self.cursor.check(&TokenKind::Semicolon) {
                    self.cursor.advance();
                    let expr_span = self.arena.get_expr(expr).span;
                    stmts.push(Stmt::new(StmtKind::Expr(expr), expr_span));
                } else if self.cursor.check(&TokenKind::RBrace) || self.cursor.is_at_end() {
                    // Expression at end without `;` → this is the result
                    last_expr = Some(expr);
                } else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            format!("expected `;` or `}}` after expression in {block_name}"),
                            self.cursor.current_span(),
                        )
                        .with_help(format!(
                            "Add `;` to make this a statement, or `}}` to end the {block_name}"
                        )),
                        item_span,
                    );
                }
            }

            self.cursor.skip_newlines();
        }

        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RBrace));

        let result = last_expr.unwrap_or(ExprId::INVALID);
        ParseOutcome::consumed_ok((stmts, result, end_span))
    }

    /// Parse a let binding inside a block, producing a `Stmt`.
    ///
    /// Assumes the `let` keyword has already been consumed. Handles binding
    /// pattern, optional type annotation, `=`, initializer expression, and
    /// trailing semicolons.
    fn parse_let_stmt(&mut self, let_span: Span, block_name: &str) -> Result<Stmt, ParseError> {
        // Don't consume `$` here — let parse_binding_pattern() handle it
        // so that BindingPattern::Name.mutable is set correctly for both
        // simple bindings (`let $x = 5`) and destructuring (`let ($a, b) = ...`).
        let pattern = self.parse_binding_pattern()?;

        // Derive statement-level mutability from the pattern.
        // For simple Name patterns, this comes from the `$` prefix.
        // For compound patterns (tuple, struct, list), default to mutable
        // since per-binding mutability is tracked on sub-patterns.
        let mutable = match &pattern {
            BindingPattern::Name { mutable, .. } => *mutable,
            _ => Mutability::Mutable,
        };
        let pattern_id = self.arena.alloc_binding_pattern(pattern);

        let ty = if self.cursor.check(&TokenKind::Colon) {
            self.cursor.advance();
            self.parse_type()
                .map_or(ParsedTypeId::INVALID, |t| self.arena.alloc_parsed_type(t))
        } else {
            ParsedTypeId::INVALID
        };

        self.cursor.expect(&TokenKind::Eq)?;
        let init = self.parse_expr().into_result()?;
        let end_span = self.arena.get_expr(init).span;

        self.cursor.skip_newlines();

        if self.cursor.check(&TokenKind::Semicolon) {
            self.cursor.advance();
        } else if !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!("expected `;` or `}}` after let binding in {block_name}"),
                self.cursor.current_span(),
            )
            .with_help("Add `;` after the let binding: `let x = value;`"));
        }

        Ok(Stmt::new(
            StmtKind::Let {
                pattern: pattern_id,
                ty,
                init,
                mutable,
            },
            let_span.merge(end_span),
        ))
    }
}
