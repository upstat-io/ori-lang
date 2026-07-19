//! Parses statement sequences inside brace-delimited blocks.

use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{BindingPattern, ExprId, Mutability, ParsedTypeId, Span, Stmt, StmtKind, TokenKind};

impl Parser<'_> {
    /// Parses the statements and final result expression of a brace-delimited block.
    pub(super) fn collect_block_stmts(
        &mut self,
        block_name: &str,
    ) -> ParseOutcome<(Vec<Stmt>, ExprId, Span)> {
        self.cursor.skip_newlines();

        let mut stmts: Vec<Stmt> = Vec::new();
        let mut last_expr: Option<ExprId> = None;

        while !self.cursor.check(&TokenKind::RBrace) && !self.cursor.is_at_end() {
            self.cursor.skip_newlines();
            if self.cursor.check(&TokenKind::RBrace) {
                break;
            }

            let item_span = self.cursor.current_span();

            if self.cursor.check(&TokenKind::Let) {
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
                let expr = require!(
                    self,
                    self.parse_expr(),
                    &format!("expression in {block_name}")
                );

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
                    last_expr = Some(expr);
                } else if self.cursor.previous_non_newline_is_rbrace() {
                    let expr_span = self.arena.get_expr(expr).span;
                    stmts.push(Stmt::new(StmtKind::Expr(expr), expr_span));
                } else {
                    let unexpected = self.cursor.current_kind().display_name();
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            format!(
                                "expected a statement boundary after expression in {block_name}, found {unexpected}"
                            ),
                            self.cursor.current_span(),
                        )
                        .with_help(format!(
                            "Add `;` before {unexpected} to start another statement, or make `}}` immediately follow the final expression (Spec: Clause 11.12)"
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
        let pattern = self.parse_binding_pattern()?;

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
