//! Brace-delimited primary expression parsing.
//!
//! Disambiguates a leading `{` into a block expression or a map literal and
//! parses each form. The `{`-vs-map decision lives in [`Parser::parse_block_or_map`].

use crate::{committed, require, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, TokenKind};

impl Parser<'_> {
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

            // `{ if :` / `{ type :` → map with a keyword-as-identifier key.
            // Keyword tokens are not `Ident`, so route a keyword usable as an
            // identifier (per grammar.ebnf § `map_key` `identifier`) to the map
            // path when followed by `:`; otherwise it is a block.
            _ if self.cursor.peek_is_ident_or_keyword(offset) => self.peek_colon_after(offset),

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
                // Regular entry: key: value — dispatch the `map_key` production
                // (grammar.ebnf § `map_key`) instead of a uniform `parse_expr`.
                let key = if p.cursor.check(&TokenKind::LBracket) {
                    // `[ expr ]` → computed key: the inner expression IS the key
                    // (never the `List` wrapper).
                    p.cursor.advance(); // [
                    let inner = p.parse_expr().into_result()?;
                    p.cursor.expect(&TokenKind::RBracket)?;
                    inner
                } else if p.cursor.peek_is_ident_or_keyword(0) {
                    // `identifier` → literal string key: `{foo: v}` keys on "foo",
                    // not the variable `foo` (covers keyword keys `{if: v}` too).
                    let key_span = p.cursor.current_span();
                    let name = p.cursor.expect_ident_or_keyword()?;
                    p.arena
                        .alloc_expr(Expr::new(ExprKind::String(name), key_span))
                } else {
                    // `string_literal` (already a correct `Str` via parse_expr)
                    // + int/bool/char literal keys (preserved behavior).
                    p.parse_expr().into_result()?
                };
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
