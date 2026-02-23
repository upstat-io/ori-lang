//! Match pattern parsing.
//!
//! Parses match patterns including wildcards, literals, bindings, at-patterns,
//! named/anonymous structs, lists, builtin variants, tuples, range patterns,
//! or-patterns, and guards.

use crate::recovery::TokenSet;
use crate::{one_of, ParseError, ParseOutcome, Parser};
use ori_ir::{
    Expr, ExprId, ExprKind, MatchPattern, MatchPatternId, MatchPatternRange, Name, TokenKind,
};

/// Tokens that start a literal pattern (including negative via `-`).
const PATTERN_LITERAL_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Minus)
    .with(TokenKind::Int(0))
    .with(TokenKind::True)
    .with(TokenKind::False)
    .with(TokenKind::String(Name::EMPTY))
    .with(TokenKind::Char('\0'));

/// Tokens that start a builtin variant pattern.
const PATTERN_VARIANT_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Some)
    .with(TokenKind::None)
    .with(TokenKind::Ok)
    .with(TokenKind::Err);

impl Parser<'_> {
    /// Parse a match pattern (for match arms).
    ///
    /// Supports: wildcard, literals (including negative), bindings, variants,
    /// tuples, structs, lists, ranges, or-patterns, at-patterns, and guards.
    pub(crate) fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        // Parse the base pattern first (via ParseOutcome, bridged to Result)
        let base = self.parse_match_pattern_base().into_result()?;

        // Check for or-pattern continuation: pattern | pattern | ...
        // Uses a Vec because patterns can recursively nest (same buffer).
        if self.cursor.check(&TokenKind::Pipe) {
            let mut alternatives = vec![self.arena.alloc_match_pattern(base)];
            while self.cursor.check(&TokenKind::Pipe) {
                self.cursor.advance();
                let alt = self.parse_match_pattern_base().into_result()?;
                alternatives.push(self.arena.alloc_match_pattern(alt));
            }
            let range = self.arena.alloc_match_pattern_list(alternatives);
            return Ok(MatchPattern::Or(range));
        }

        Ok(base)
    }

    /// Parse a base match pattern (without or-pattern handling).
    ///
    /// Uses `one_of!` for automatic backtracking across pattern alternatives.
    fn parse_match_pattern_base(&mut self) -> ParseOutcome<MatchPattern> {
        one_of!(
            self,
            self.parse_pattern_wildcard(),
            self.parse_pattern_literal(),
            self.parse_pattern_ident(),
            self.parse_pattern_struct(),
            self.parse_pattern_list(),
            self.parse_pattern_builtin_variant(),
            self.parse_pattern_tuple(),
        )
    }

    /// Parse wildcard pattern: `_`
    fn parse_pattern_wildcard(&mut self) -> ParseOutcome<MatchPattern> {
        if !self.cursor.check(&TokenKind::Underscore) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Underscore,
                self.cursor.current_span().start as usize,
            );
        }
        self.cursor.advance();
        ParseOutcome::consumed_ok(MatchPattern::Wildcard)
    }

    /// Parse literal patterns: integers (possibly negative), booleans, strings.
    /// Also handles range patterns: `1..10`, `1..=10`.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive literal and range pattern dispatch across all token kinds"
    )]
    fn parse_pattern_literal(&mut self) -> ParseOutcome<MatchPattern> {
        match *self.cursor.current_kind() {
            // Negative integer literal: -42
            TokenKind::Minus => {
                let start_span = self.cursor.current_span();
                self.cursor.advance();
                if let TokenKind::Int(n) = *self.cursor.current_kind() {
                    self.cursor.advance();
                    let Ok(value) = i64::try_from(n) else {
                        return ParseOutcome::consumed_err(
                            ParseError::new(
                                ori_diagnostic::ErrorCode::E1002,
                                "integer literal too large",
                                start_span,
                            ),
                            start_span,
                        );
                    };
                    let span = start_span.merge(self.cursor.previous_span());
                    ParseOutcome::consumed_ok(MatchPattern::Literal(
                        self.arena
                            .alloc_expr(Expr::new(ExprKind::Int(-value), span)),
                    ))
                } else {
                    ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "expected integer after `-` in pattern",
                            self.cursor.current_span(),
                        ),
                        start_span,
                    )
                }
            }

            // Positive integer literal: 42 (with possible range)
            TokenKind::Int(n) => {
                let pat_span = self.cursor.current_span();
                self.cursor.advance();
                let Ok(value) = i64::try_from(n) else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large",
                            pat_span,
                        ),
                        pat_span,
                    );
                };

                // Check for range pattern: 1..10 or 1..=10
                if self.cursor.check(&TokenKind::DotDot) || self.cursor.check(&TokenKind::DotDotEq)
                {
                    let inclusive = self.cursor.check(&TokenKind::DotDotEq);
                    self.cursor.advance();
                    let start_expr = self
                        .arena
                        .alloc_expr(Expr::new(ExprKind::Int(value), pat_span));

                    // Parse end of range (optional for open-ended ranges)
                    let end = if self.is_range_bound_start() {
                        match self.parse_range_bound() {
                            Ok(e) => Some(e),
                            Err(err) => {
                                return ParseOutcome::consumed_err(err, pat_span);
                            }
                        }
                    } else {
                        None
                    };

                    return ParseOutcome::consumed_ok(MatchPattern::Range {
                        start: Some(start_expr),
                        end,
                        inclusive,
                    });
                }

                ParseOutcome::consumed_ok(MatchPattern::Literal(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Int(value), self.cursor.previous_span())),
                ))
            }
            TokenKind::True => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(MatchPattern::Literal(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Bool(true), self.cursor.previous_span())),
                ))
            }
            TokenKind::False => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::Bool(false),
                    self.cursor.previous_span(),
                ))))
            }
            TokenKind::String(name) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(MatchPattern::Literal(self.arena.alloc_expr(Expr::new(
                    ExprKind::String(name),
                    self.cursor.previous_span(),
                ))))
            }
            TokenKind::Char(c) => {
                let pat_span = self.cursor.current_span();
                self.cursor.advance();

                // Check for range pattern: 'a'..'z' or 'a'..='z'
                if self.cursor.check(&TokenKind::DotDot) || self.cursor.check(&TokenKind::DotDotEq)
                {
                    let inclusive = self.cursor.check(&TokenKind::DotDotEq);
                    self.cursor.advance();
                    let start_expr = self
                        .arena
                        .alloc_expr(Expr::new(ExprKind::Char(c), pat_span));

                    let end = if self.is_range_bound_start() {
                        match self.parse_range_bound() {
                            Ok(e) => Some(e),
                            Err(err) => {
                                return ParseOutcome::consumed_err(err, pat_span);
                            }
                        }
                    } else {
                        None
                    };

                    return ParseOutcome::consumed_ok(MatchPattern::Range {
                        start: Some(start_expr),
                        end,
                        inclusive,
                    });
                }

                ParseOutcome::consumed_ok(MatchPattern::Literal(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Char(c), self.cursor.previous_span())),
                ))
            }
            _ => ParseOutcome::empty_err(
                PATTERN_LITERAL_TOKENS,
                self.cursor.current_span().start as usize,
            ),
        }
    }

    /// Parse identifier pattern: binding, at-pattern, named variant, or named struct.
    fn parse_pattern_ident(&mut self) -> ParseOutcome<MatchPattern> {
        let TokenKind::Ident(name) = *self.cursor.current_kind() else {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Ident(Name::EMPTY),
                self.cursor.current_span().start as usize,
            );
        };

        self.cursor.advance();

        // Check for at-pattern: x @ pattern
        if self.cursor.check(&TokenKind::At) {
            self.cursor.advance();
            let pattern = match self.parse_match_pattern_base().into_result() {
                Ok(p) => p,
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            };
            let pattern_id = self.arena.alloc_match_pattern(pattern);
            return ParseOutcome::consumed_ok(MatchPattern::At {
                name,
                pattern: pattern_id,
            });
        }

        // Check for variant pattern: Name(x) or struct literal: Point { x, y }
        if self.cursor.check(&TokenKind::LParen) {
            self.cursor.advance();
            let inner = match self.parse_variant_inner_patterns() {
                Ok(i) => i,
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            };
            match self.cursor.expect(&TokenKind::RParen) {
                Ok(_) => {}
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            }
            return ParseOutcome::consumed_ok(MatchPattern::Variant { name, inner });
        }

        if self.cursor.check(&TokenKind::LBrace) {
            // Named struct pattern: Point { x, y }
            match self.parse_struct_pattern_fields() {
                Ok(pat) => return ParseOutcome::consumed_ok(pat),
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            }
        }

        ParseOutcome::consumed_ok(MatchPattern::Binding(name))
    }

    /// Parse anonymous struct pattern: `{ x, y }`
    fn parse_pattern_struct(&mut self) -> ParseOutcome<MatchPattern> {
        if !self.cursor.check(&TokenKind::LBrace) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LBrace,
                self.cursor.current_span().start as usize,
            );
        }
        match self.parse_struct_pattern_fields() {
            Ok(pat) => ParseOutcome::consumed_ok(pat),
            Err(err) => ParseOutcome::consumed_err(err, self.cursor.current_span()),
        }
    }

    /// Parse list pattern: `[a, b, ..rest]`
    fn parse_pattern_list(&mut self) -> ParseOutcome<MatchPattern> {
        if !self.cursor.check(&TokenKind::LBracket) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LBracket,
                self.cursor.current_span().start as usize,
            );
        }
        self.cursor.advance();
        // Pattern elements use a Vec because patterns can recursively
        // nest (e.g., `[a, (b, c), [d, e]]`), sharing the same buffer.
        let mut elements: Vec<MatchPatternId> = Vec::new();
        let mut rest = None;

        while !self.cursor.check(&TokenKind::RBracket) && !self.cursor.is_at_end() {
            // Check for rest pattern: ..rest or ..
            if self.cursor.check(&TokenKind::DotDot) {
                self.cursor.advance();
                // Optional name after .. (Name::EMPTY for anonymous rest)
                if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                    rest = Some(name);
                    self.cursor.advance();
                } else {
                    // Anonymous rest pattern: use empty name as sentinel
                    rest = Some(Name::EMPTY);
                }
                // Rest must be last
                break;
            }

            let elem = match self.parse_match_pattern() {
                Ok(e) => e,
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            };
            elements.push(self.arena.alloc_match_pattern(elem));

            if !self.cursor.check(&TokenKind::RBracket) && !self.cursor.check(&TokenKind::DotDot) {
                match self.cursor.expect(&TokenKind::Comma) {
                    Ok(_) => {}
                    Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
                }
            }
        }

        match self.cursor.expect(&TokenKind::RBracket) {
            Ok(_) => {}
            Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
        }
        let elements = self.arena.alloc_match_pattern_list(elements);
        ParseOutcome::consumed_ok(MatchPattern::List { elements, rest })
    }

    /// Parse builtin variant patterns: `Some(x)`, `None`, `Ok(x)`, `Err(x)`
    fn parse_pattern_builtin_variant(&mut self) -> ParseOutcome<MatchPattern> {
        let (name_str, has_inner) = match *self.cursor.current_kind() {
            TokenKind::Some => ("Some", true),
            TokenKind::None => ("None", false),
            TokenKind::Ok => ("Ok", true),
            TokenKind::Err => ("Err", true),
            _ => {
                return ParseOutcome::empty_err(
                    PATTERN_VARIANT_TOKENS,
                    self.cursor.current_span().start as usize,
                )
            }
        };

        let name = self.cursor.interner().intern(name_str);
        self.cursor.advance();
        let inner = if has_inner {
            match self.cursor.expect(&TokenKind::LParen) {
                Ok(_) => {}
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            }
            let patterns = match self.parse_variant_inner_patterns() {
                Ok(p) => p,
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            };
            match self.cursor.expect(&TokenKind::RParen) {
                Ok(_) => {}
                Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
            }
            patterns
        } else {
            MatchPatternRange::EMPTY
        };
        ParseOutcome::consumed_ok(MatchPattern::Variant { name, inner })
    }

    /// Parse tuple pattern: `(a, b, c)`
    fn parse_pattern_tuple(&mut self) -> ParseOutcome<MatchPattern> {
        use crate::series::SeriesConfig;

        if !self.cursor.check(&TokenKind::LParen) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LParen,
                self.cursor.current_span().start as usize,
            );
        }

        self.cursor.advance();
        // Tuple patterns use a Vec because patterns can recursively
        // nest (e.g., `(a, (b, c))`), sharing the same buffer.
        let mut elements: Vec<MatchPatternId> = Vec::new();
        match self.series_direct(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(false);
            }
            let pat = p.parse_match_pattern()?;
            elements.push(p.arena.alloc_match_pattern(pat));
            Ok(true)
        }) {
            Ok(_) => {}
            Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
        }
        match self.cursor.expect(&TokenKind::RParen) {
            Ok(_) => {}
            Err(err) => return ParseOutcome::consumed_err(err, self.cursor.current_span()),
        }
        let range = self.arena.alloc_match_pattern_list(elements);
        ParseOutcome::consumed_ok(MatchPattern::Tuple(range))
    }

    /// Parse comma-separated patterns inside a variant pattern.
    ///
    /// Returns an empty range for unit variants (when immediately followed by `)`),
    /// or a range with one or more patterns for variants with fields.
    fn parse_variant_inner_patterns(&mut self) -> Result<MatchPatternRange, ParseError> {
        use crate::series::SeriesConfig;

        // Variant inner patterns use a Vec because patterns can recursively
        // nest (e.g., `Ok(Some(x))`), sharing the same buffer.
        let mut elements: Vec<MatchPatternId> = Vec::new();
        self.series_direct(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(false);
            }
            let pat = p.parse_match_pattern()?;
            elements.push(p.arena.alloc_match_pattern(pat));
            Ok(true)
        })?;

        if elements.is_empty() {
            Ok(MatchPatternRange::EMPTY)
        } else {
            Ok(self.arena.alloc_match_pattern_list(elements))
        }
    }

    /// Parse struct pattern fields: `{ x, y: pattern }` or `{ x, .. }`.
    fn parse_struct_pattern_fields(&mut self) -> Result<MatchPattern, ParseError> {
        self.cursor.advance(); // consume {

        let mut fields: Vec<(ori_ir::Name, Option<MatchPatternId>)> = Vec::new();
        let mut rest = false;

        self.brace_series(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(None);
            }

            // Check for `..` rest pattern
            if p.cursor.check(&TokenKind::DotDot) {
                p.cursor.advance();
                rest = true;
                return Ok(None);
            }

            let field_name = p.cursor.expect_ident()?;

            // Check for pattern binding: { x: pattern } vs shorthand { x }
            let pattern_id = if p.cursor.check(&TokenKind::Colon) {
                p.cursor.advance();
                let pat = p.parse_match_pattern()?;
                Some(p.arena.alloc_match_pattern(pat))
            } else {
                None // Shorthand: field name is also the binding
            };

            fields.push((field_name, pattern_id));
            Ok(Some(()))
        })?;

        Ok(MatchPattern::Struct { fields, rest })
    }

    /// Check if current token can start a range bound (integer, char, or minus).
    fn is_range_bound_start(&self) -> bool {
        matches!(
            self.cursor.current_kind(),
            TokenKind::Int(_) | TokenKind::Char(_) | TokenKind::Minus
        )
    }

    /// Parse an optional pattern guard: `if condition` or `.match(condition)` (legacy).
    ///
    /// Returns `Some(expr_id)` if a guard is present, `None` otherwise.
    pub(crate) fn parse_pattern_guard(&mut self) -> Result<Option<ExprId>, ParseError> {
        // New syntax: `if condition`
        if self.cursor.check(&TokenKind::If) {
            self.cursor.advance();
            let condition = self.parse_expr().into_result()?;
            return Ok(Some(condition));
        }

        // Legacy syntax: `.match(condition)` — kept for migration period
        if self.cursor.check(&TokenKind::Dot) && self.is_guard_syntax() {
            self.cursor.advance(); // consume `.`
            self.cursor.advance(); // consume `match`
            self.cursor.expect(&TokenKind::LParen)?;
            let condition = self.parse_expr().into_result()?;
            self.cursor.expect(&TokenKind::RParen)?;
            return Ok(Some(condition));
        }

        Ok(None)
    }

    /// Check if the current position has `.match(` syntax (legacy guard syntax).
    fn is_guard_syntax(&self) -> bool {
        if !self.cursor.check(&TokenKind::Dot) {
            return false;
        }
        // Look ahead: . match (
        matches!(self.cursor.peek_next_kind(), TokenKind::Match)
    }

    /// Parse a range bound (integer, possibly negative).
    fn parse_range_bound(&mut self) -> Result<ExprId, ParseError> {
        let start_span = self.cursor.current_span();

        if self.cursor.check(&TokenKind::Minus) {
            self.cursor.advance();
            if let TokenKind::Int(n) = *self.cursor.current_kind() {
                self.cursor.advance();
                let value = i64::try_from(n).map_err(|_| {
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "integer literal too large",
                        start_span,
                    )
                })?;
                let span = start_span.merge(self.cursor.previous_span());
                Ok(self
                    .arena
                    .alloc_expr(Expr::new(ExprKind::Int(-value), span)))
            } else {
                Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "expected integer after `-` in range pattern",
                    self.cursor.current_span(),
                ))
            }
        } else if let TokenKind::Int(n) = *self.cursor.current_kind() {
            self.cursor.advance();
            let value = i64::try_from(n).map_err(|_| {
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "integer literal too large",
                    start_span,
                )
            })?;
            Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Int(value), self.cursor.previous_span())))
        } else if let TokenKind::Char(c) = *self.cursor.current_kind() {
            self.cursor.advance();
            Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Char(c), self.cursor.previous_span())))
        } else {
            Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected integer or char literal in range pattern",
                self.cursor.current_span(),
            ))
        }
    }
}
