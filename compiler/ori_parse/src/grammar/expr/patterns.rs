//! Pattern Parsing (`function_seq` and `function_exp`)
//!
//! Parses run, try, match, for patterns, and `function_exp` constructs.
//!
//! Match pattern parsing uses `one_of!` for automatic backtracking across
//! pattern alternatives (wildcard, literal, ident, struct, list, variant, tuple).

use crate::context::ParseContext;
use crate::recovery::TokenSet;
use crate::{committed, one_of, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    Expr, ExprId, ExprKind, FunctionExp, FunctionExpKind, FunctionSeq, MatchArm, MatchPattern,
    MatchPatternId, MatchPatternRange, Name, NamedExpr, ParsedTypeId, SeqBinding, TokenKind,
};

/// Kind of `function_seq` expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionSeqKind {
    Run,
    Try,
}

// === Token sets for match pattern EmptyErr reporting ===

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
    /// Parse `run(...)` expression.
    ///
    /// Called after `run` keyword has been consumed by `parse_primary`.
    pub(crate) fn parse_run(&mut self) -> ParseOutcome<ExprId> {
        self.parse_function_seq_internal(FunctionSeqKind::Run)
    }

    /// Parse `try(...)` expression.
    ///
    /// Called after `try` keyword has been consumed by `parse_primary`.
    pub(crate) fn parse_try(&mut self) -> ParseOutcome<ExprId> {
        self.parse_function_seq_internal(FunctionSeqKind::Try)
    }

    /// Internal implementation for parsing run/try expressions.
    ///
    /// For `run()`, supports `pre_check:` and `post_check:` named properties:
    /// ```text
    /// run(
    ///     pre_check: condition | "message",
    ///     let x = value,
    ///     result,
    ///     post_check: r -> r > 0 | "message",
    /// )
    /// ```
    fn parse_function_seq_internal(&mut self, kind: FunctionSeqKind) -> ParseOutcome<ExprId> {
        let is_try = matches!(kind, FunctionSeqKind::Try);
        let is_run = !is_try;
        let start_span = self.cursor.previous_span();
        committed!(self.cursor.expect(&TokenKind::LParen));
        self.cursor.skip_newlines();

        let pre_check_start = self.arena.start_checks();
        let mut bindings = Vec::new();
        let mut result_expr = None;

        // Phase 1: Parse pre_checks (only for run, not try)
        if is_run {
            while self.is_check_start("pre_check") {
                let check = committed!(self.parse_named_check("pre_check"));
                self.arena.push_check(check);
                self.cursor.skip_newlines();
                if !self.cursor.check(&TokenKind::RParen) {
                    committed!(self.cursor.expect(&TokenKind::Comma));
                    self.cursor.skip_newlines();
                }
            }
        }
        let pre_check_range = self.arena.finish_checks(pre_check_start);

        // Phase 2: Parse bindings and result expression
        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            self.cursor.skip_newlines();

            // Check for post_check (only for run) — switches to phase 3
            if is_run && self.is_check_start("post_check") {
                break;
            }

            if self.cursor.check(&TokenKind::Let) {
                let binding_span = self.cursor.current_span();
                self.cursor.advance();

                // Per spec: mutable by default, $ prefix for immutable
                let mutable = if self.cursor.check(&TokenKind::Dollar) {
                    self.cursor.advance();
                    false
                } else if self.cursor.check(&TokenKind::Mut) {
                    self.cursor.advance();
                    true
                } else {
                    true
                };

                let pattern = committed!(self.parse_binding_pattern());
                let pattern_id = self.arena.alloc_binding_pattern(pattern);

                let ty = if self.cursor.check(&TokenKind::Colon) {
                    self.cursor.advance();
                    self.parse_type()
                        .map_or(ParsedTypeId::INVALID, |t| self.arena.alloc_parsed_type(t))
                } else {
                    ParsedTypeId::INVALID
                };

                committed!(self.cursor.expect(&TokenKind::Eq));
                let value = require!(self, self.parse_expr(), "expression after `=`");
                let end_span = self.arena.get_expr(value).span;

                bindings.push(SeqBinding::Let {
                    pattern: pattern_id,
                    ty,
                    value,
                    mutable,
                    span: binding_span.merge(end_span),
                });
            } else {
                let expr_span = self.cursor.current_span();
                let expr = require!(self, self.parse_expr(), "expression");
                let end_span = self.arena.get_expr(expr).span;

                self.cursor.skip_newlines();

                if self.cursor.check(&TokenKind::Comma) {
                    self.cursor.advance();
                    self.cursor.skip_newlines();

                    if self.cursor.check(&TokenKind::RParen) {
                        result_expr = Some(expr);
                    } else if is_run && self.is_check_start("post_check") {
                        // Result followed by post_check
                        result_expr = Some(expr);
                    } else {
                        bindings.push(SeqBinding::Stmt {
                            expr,
                            span: expr_span.merge(end_span),
                        });
                    }
                    continue;
                }
                result_expr = Some(expr);
            }

            self.cursor.skip_newlines();

            if !self.cursor.check(&TokenKind::RParen) {
                committed!(self.cursor.expect(&TokenKind::Comma));
                self.cursor.skip_newlines();
            }
        }

        // Phase 3: Parse post_checks (only for run, not try)
        let post_check_start = self.arena.start_checks();
        if is_run {
            while self.is_check_start("post_check") {
                let check = committed!(self.parse_named_check("post_check"));
                self.arena.push_check(check);
                self.cursor.skip_newlines();
                if !self.cursor.check(&TokenKind::RParen) {
                    committed!(self.cursor.expect(&TokenKind::Comma));
                    self.cursor.skip_newlines();
                }
            }
        }
        let post_check_range = self.arena.finish_checks(post_check_start);

        self.cursor.skip_newlines();
        committed!(self.cursor.expect(&TokenKind::RParen));
        let end_span = self.cursor.previous_span();

        let Some(result) = result_expr else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    format!(
                        "{} requires a result expression",
                        if is_try { "try" } else { "run" }
                    ),
                    end_span,
                ),
                start_span,
            );
        };

        let bindings_range = self.arena.alloc_seq_bindings(bindings);
        let span = start_span.merge(end_span);
        let func_seq = if is_try {
            FunctionSeq::Try {
                bindings: bindings_range,
                result,
                span,
            }
        } else {
            FunctionSeq::Run {
                pre_checks: pre_check_range,
                bindings: bindings_range,
                result,
                post_checks: post_check_range,
                span,
            }
        };

        let func_seq_id = self.arena.alloc_function_seq(func_seq);
        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq_id), span)),
        )
    }

    /// Check if the current position starts a check property (`pre_check:` or `post_check:`).
    fn is_check_start(&self, name: &str) -> bool {
        if let TokenKind::Ident(n) = self.cursor.current_kind() {
            self.cursor.interner().lookup(*n) == name && self.cursor.next_is_colon()
        } else {
            false
        }
    }

    /// Parse a named check (`pre_check: expr | "msg"` or `post_check: expr | "msg"`).
    ///
    /// Assumes the cursor is at the check name identifier. Consumes the name, colon,
    /// expression, and optional `| "message"`.
    fn parse_named_check(&mut self, _check_name: &str) -> Result<ori_ir::CheckExpr, ParseError> {
        let check_span = self.cursor.current_span();
        self.cursor.advance(); // consume check name
        self.cursor.expect(&TokenKind::Colon)?;

        // Parse condition with `|` as separator (not bitwise OR).
        // In check context, `x > 0 | "msg"` means condition `x > 0` with message `"msg"`.
        let expr = self
            .with_context(ParseContext::PIPE_IS_SEPARATOR, Self::parse_expr)
            .into_result()?;

        // Optional custom message: `| "message"`
        let message = if self.cursor.check(&TokenKind::Pipe) {
            self.cursor.advance();
            Some(self.parse_expr().into_result()?)
        } else {
            None
        };

        let end = message.map_or_else(
            || self.arena.get_expr(expr).span,
            |m| self.arena.get_expr(m).span,
        );

        Ok(ori_ir::CheckExpr {
            expr,
            message,
            span: check_span.merge(end),
        })
    }

    /// Parse match as `function_seq`: match(scrutinee, Pattern -> expr, ...)
    ///
    /// Called after `match` keyword has been consumed by `parse_primary`.
    pub(crate) fn parse_match_expr(&mut self) -> ParseOutcome<ExprId> {
        self.in_error_context(
            crate::ErrorContext::MatchExpression,
            Self::parse_match_expr_body,
        )
    }

    fn parse_match_expr_body(&mut self) -> ParseOutcome<ExprId> {
        let start_span = self.cursor.previous_span();
        committed!(self.cursor.expect(&TokenKind::LParen));
        self.cursor.skip_newlines();

        let scrutinee = require!(self, self.parse_expr(), "match scrutinee");

        self.cursor.skip_newlines();
        committed!(self.cursor.expect(&TokenKind::Comma));
        self.cursor.skip_newlines();

        // Match arms use a Vec because nested match expressions share
        // the same `arms` buffer, causing same-buffer nesting conflicts.
        let mut arms: Vec<MatchArm> = Vec::new();
        committed!(self.paren_series_direct(|p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(false);
            }

            let arm_span = p.cursor.current_span();
            let pattern = p.parse_match_pattern()?;

            // Check for guard: pattern.match(condition)
            let guard = p.parse_pattern_guard()?;

            p.cursor.expect(&TokenKind::Arrow)?;
            let body = p.parse_expr().into_result()?;
            let end_span = p.arena.get_expr(body).span;

            arms.push(MatchArm {
                pattern,
                guard,
                body,
                span: arm_span.merge(end_span),
            });
            Ok(true)
        }));
        let end_span = self.cursor.previous_span();

        if arms.is_empty() {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "match requires at least one arm".to_string(),
                    end_span,
                ),
                start_span,
            );
        }

        let arms_range = self.arena.alloc_arms(arms);
        let span = start_span.merge(end_span);
        let func_seq = FunctionSeq::Match {
            scrutinee,
            arms: arms_range,
            span,
        };

        let func_seq_id = self.arena.alloc_function_seq(func_seq);
        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq_id), span)),
        )
    }

    /// Parse for pattern: for(over: items, [map: transform,] match: Pattern -> expr, default: value)
    ///
    /// Called after `for` keyword has been consumed by `parse_primary`.
    pub(crate) fn parse_for_pattern(&mut self) -> ParseOutcome<ExprId> {
        let start_span = self.cursor.previous_span();
        committed!(self.cursor.expect(&TokenKind::LParen));
        self.cursor.skip_newlines();

        let mut over: Option<ExprId> = None;
        let mut map: Option<ExprId> = None;
        let mut match_arm: Option<MatchArm> = None;
        let mut default: Option<ExprId> = None;

        while !self.cursor.check(&TokenKind::RParen) && !self.cursor.is_at_end() {
            self.cursor.skip_newlines();

            if !self.cursor.is_named_arg_start() {
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1013,
                        "`for` pattern requires named properties (over:, match:, default:)"
                            .to_string(),
                        self.cursor.current_span(),
                    ),
                    start_span,
                );
            }

            let name = committed!(self.cursor.expect_ident_or_keyword());
            committed!(self.cursor.expect(&TokenKind::Colon));
            let name_str = self.cursor.interner().lookup(name);

            match name_str {
                "over" => {
                    over = Some(require!(
                        self,
                        self.parse_expr(),
                        "`over:` expression in for pattern"
                    ));
                }
                "map" => {
                    map = Some(require!(
                        self,
                        self.parse_expr(),
                        "`map:` expression in for pattern"
                    ));
                }
                "match" => {
                    let arm_span = self.cursor.current_span();
                    let pattern = committed!(self.parse_match_pattern());
                    let guard = committed!(self.parse_pattern_guard());
                    committed!(self.cursor.expect(&TokenKind::Arrow));
                    let body = require!(self, self.parse_expr(), "match body in for pattern");
                    let end_span = self.arena.get_expr(body).span;
                    match_arm = Some(MatchArm {
                        pattern,
                        guard,
                        body,
                        span: arm_span.merge(end_span),
                    });
                }
                "default" => {
                    default = Some(require!(
                        self,
                        self.parse_expr(),
                        "`default:` expression in for pattern"
                    ));
                }
                unknown => {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1013,
                            format!("`for` pattern does not accept property `{unknown}`"),
                            self.cursor.previous_span(),
                        ),
                        start_span,
                    );
                }
            }

            self.cursor.skip_newlines();
            if !self.cursor.check(&TokenKind::RParen) {
                committed!(self.cursor.expect(&TokenKind::Comma));
                self.cursor.skip_newlines();
            }
        }

        self.cursor.skip_newlines();
        committed!(self.cursor.expect(&TokenKind::RParen));
        let end_span = self.cursor.previous_span();
        let span = start_span.merge(end_span);

        let Some(over) = over else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    "`for` pattern requires `over:` property".to_string(),
                    span,
                ),
                start_span,
            );
        };
        let Some(arm) = match_arm else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    "`for` pattern requires `match:` property".to_string(),
                    span,
                ),
                start_span,
            );
        };
        let Some(default) = default else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    "`for` pattern requires `default:` property".to_string(),
                    span,
                ),
                start_span,
            );
        };

        let func_seq = FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            span,
        };

        let func_seq_id = self.arena.alloc_function_seq(func_seq);
        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq_id), span)),
        )
    }

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

    // === Extracted pattern sub-parsers ===

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
                                "integer literal too large".to_string(),
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
                            "expected integer after `-` in pattern".to_string(),
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
                            "integer literal too large".to_string(),
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

    /// Parse `function_exp`: map, filter, fold, etc. with named properties.
    ///
    /// Called after the `function_exp` keyword has been consumed by `parse_primary`.
    pub(crate) fn parse_function_exp(&mut self, kind: FunctionExpKind) -> ParseOutcome<ExprId> {
        let start_span = self.cursor.previous_span();
        committed!(self.cursor.expect(&TokenKind::LParen));
        self.cursor.skip_newlines();

        // Named exprs use a Vec because function expressions can nest
        // (e.g., `parallel(f: timeout(t: 5s, ...))`), sharing the same buffer.
        let mut props: Vec<NamedExpr> = Vec::new();
        committed!(self.paren_series_direct(|p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(false);
            }

            if !p.cursor.is_named_arg_start() {
                return Err(ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    format!("`{}` requires named properties (name: value)", kind.name()),
                    p.cursor.current_span(),
                ));
            }

            let name = p.cursor.expect_ident_or_keyword()?;
            let prop_span = p.cursor.previous_span();
            p.cursor.expect(&TokenKind::Colon)?;
            let value = p.parse_expr().into_result()?;
            let end_span = p.arena.get_expr(value).span;

            props.push(NamedExpr {
                name,
                value,
                span: prop_span.merge(end_span),
            });
            Ok(true)
        }));
        let end_span = self.cursor.previous_span();

        let props_range = self.arena.alloc_named_exprs(props);
        let func_exp = FunctionExp {
            kind,
            props: props_range,
            span: start_span.merge(end_span),
        };

        let func_exp_id = self.arena.alloc_function_exp(func_exp);
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionExp(func_exp_id),
            start_span.merge(end_span),
        )))
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

    /// Parse struct pattern fields: `{ x, y: pattern, ... }`
    fn parse_struct_pattern_fields(&mut self) -> Result<MatchPattern, ParseError> {
        self.cursor.advance(); // consume {

        let fields: Vec<(ori_ir::Name, Option<MatchPatternId>)> = self.brace_series(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
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

            Ok(Some((field_name, pattern_id)))
        })?;

        Ok(MatchPattern::Struct { fields })
    }

    /// Check if current token can start a range bound (integer, char, or minus).
    fn is_range_bound_start(&self) -> bool {
        matches!(
            self.cursor.current_kind(),
            TokenKind::Int(_) | TokenKind::Char(_) | TokenKind::Minus
        )
    }

    /// Parse an optional pattern guard: `.match(condition)`
    ///
    /// Returns `Some(expr_id)` if a guard is present, `None` otherwise.
    pub(crate) fn parse_pattern_guard(&mut self) -> Result<Option<ExprId>, ParseError> {
        // Check for .match(condition) syntax
        if !self.cursor.check(&TokenKind::Dot) {
            return Ok(None);
        }

        // Peek ahead to see if it's .match specifically
        if !self.is_guard_syntax() {
            return Ok(None);
        }

        // Consume the `.`
        self.cursor.advance();

        // Expect `match` identifier
        if !self.cursor.check(&TokenKind::Match) {
            // Not a guard, could be a field access (but that's not valid here)
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected `match` after `.` in pattern guard".to_string(),
                self.cursor.current_span(),
            ));
        }
        self.cursor.advance();

        // Expect (condition)
        self.cursor.expect(&TokenKind::LParen)?;
        let condition = self.parse_expr().into_result()?;
        self.cursor.expect(&TokenKind::RParen)?;

        Ok(Some(condition))
    }

    /// Check if the current position has `.match(` syntax (guard syntax).
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
                        "integer literal too large".to_string(),
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
                    "expected integer after `-` in range pattern".to_string(),
                    self.cursor.current_span(),
                ))
            }
        } else if let TokenKind::Int(n) = *self.cursor.current_kind() {
            self.cursor.advance();
            let value = i64::try_from(n).map_err(|_| {
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "integer literal too large".to_string(),
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
                "expected integer or char literal in range pattern".to_string(),
                self.cursor.current_span(),
            ))
        }
    }
}
