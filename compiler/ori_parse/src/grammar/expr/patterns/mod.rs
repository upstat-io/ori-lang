//! Pattern Parsing (try, match, `function_exp`)
//!
//! Parses try blocks, match expressions, for patterns, and `function_exp` constructs.
//!
//! Match pattern parsing uses `one_of!` for automatic backtracking across
//! pattern alternatives (wildcard, literal, ident, struct, list, variant, tuple).

mod match_patterns;

use crate::context::ParseContext;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    Expr, ExprId, ExprKind, FunctionExp, FunctionExpKind, FunctionSeq, MatchArm, NamedExpr,
    ParsedTypeRange, TokenKind,
};

impl Parser<'_> {
    /// Parse try expression: `try { block }`.
    ///
    /// Called after `try` keyword has been consumed by `parse_primary`.
    pub(crate) fn parse_try(&mut self) -> ParseOutcome<ExprId> {
        if self.cursor.check(&TokenKind::LParen) {
            let span = self.cursor.current_span();
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "`try()` syntax has been removed",
                    span,
                )
                .with_help("Use block syntax instead: `try { let x = expr?; Ok(x) }`"),
                span,
            );
        }
        self.parse_try_block()
    }

    /// Parse `try { stmts; result }` — parses block contents as `FunctionSeq::Try`
    /// with auto-unwrap semantics on let bindings.
    fn parse_try_block(&mut self) -> ParseOutcome<ExprId> {
        let start_span = self.cursor.previous_span();
        committed!(self.cursor.expect(&TokenKind::LBrace));

        let (stmts_vec, result, end_span) = require!(
            self,
            self.collect_block_stmts("try block"),
            "try block body"
        );

        let span = start_span.merge(end_span);

        // Batch-push statements to arena after nested parsing completes.
        let stmt_start = self.arena.start_stmts();
        for stmt in stmts_vec {
            self.arena.push_stmt(stmt);
        }
        let stmts = self.arena.finish_stmts(stmt_start);

        let func_seq = FunctionSeq::Try {
            stmts,
            result,
            span,
        };
        let func_seq_id = self.arena.alloc_function_seq(func_seq);
        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq_id), span)),
        )
    }

    /// Parse match expression: `match expr { pattern -> body, ... }`
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

        if self.cursor.check(&TokenKind::LParen) {
            let span = self.cursor.current_span();
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "`match()` syntax has been removed",
                    span,
                )
                .with_help("Use brace syntax instead: `match expr { pattern -> body, ... }`"),
                span,
            );
        }

        // match expr { arm, arm, ... }
        // Disable struct literals so `match x { ... }` doesn't parse `x { ... }` as a struct
        self.cursor.skip_newlines();
        let scrutinee = require!(
            self,
            self.with_context(ParseContext::NO_STRUCT_LIT, Self::parse_expr),
            "match scrutinee"
        );

        self.cursor.skip_newlines();

        let result = committed!(self.parse_match_arms_brace(scrutinee, start_span));
        ParseOutcome::consumed_ok(result)
    }

    /// Parse match arms enclosed in braces: `{ pattern -> body, ... }`.
    ///
    /// Arms are comma-separated (per match-arm-comma-separator-proposal).
    fn parse_match_arms_brace(
        &mut self,
        scrutinee: ExprId,
        start_span: ori_ir::Span,
    ) -> Result<ExprId, ParseError> {
        self.cursor.expect(&TokenKind::LBrace)?;
        self.cursor.skip_newlines();

        let mut arms: Vec<MatchArm> = Vec::new();
        self.brace_series_direct(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(false);
            }

            let arm_span = p.cursor.current_span();
            let pattern = p.parse_match_pattern()?;
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
        })?;
        let end_span = self.cursor.previous_span();

        if arms.is_empty() {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "match requires at least one arm",
                end_span,
            ));
        }

        let arms_range = self.arena.alloc_arms(arms);
        let span = start_span.merge(end_span);
        let func_seq = FunctionSeq::Match {
            scrutinee,
            arms: arms_range,
            span,
        };

        let func_seq_id = self.arena.alloc_function_seq(func_seq);
        Ok(self
            .arena
            .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq_id), span)))
    }

    /// Parse match arms with a known scrutinee and construct a Match expression.
    ///
    /// Expects `(` has already been consumed. Parses comma-separated match arms
    /// and the closing `)`. Used by both `match(scrutinee, arms...)` and
    /// `scrutinee.match(arms...)` method-style syntax.
    pub(crate) fn parse_match_arms_with_scrutinee(
        &mut self,
        scrutinee: ExprId,
        start_span: ori_ir::Span,
    ) -> Result<ExprId, ParseError> {
        self.cursor.skip_newlines();

        // Match arms use a Vec because nested match expressions share
        // the same `arms` buffer, causing same-buffer nesting conflicts.
        let mut arms: Vec<MatchArm> = Vec::new();
        self.paren_series_direct(|p| {
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
        })?;
        let end_span = self.cursor.previous_span();

        if arms.is_empty() {
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "match requires at least one arm",
                end_span,
            ));
        }

        let arms_range = self.arena.alloc_arms(arms);
        let span = start_span.merge(end_span);
        let func_seq = FunctionSeq::Match {
            scrutinee,
            arms: arms_range,
            span,
        };

        let func_seq_id = self.arena.alloc_function_seq(func_seq);
        Ok(self
            .arena
            .alloc_expr(Expr::new(ExprKind::FunctionSeq(func_seq_id), span)))
    }

    /// Parse for pattern: for(over: items, [map: transform,] match: Pattern -> expr, default: value)
    ///
    /// Called after `for` keyword has been consumed by `parse_primary`.
    #[expect(
        clippy::too_many_lines,
        reason = "multi-clause for-pattern parser handling over/map/match/default props"
    )]
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
                        "`for` pattern requires named properties (over:, match:, default:)",
                        self.cursor.current_span(),
                    ),
                    start_span,
                );
            }

            let prop = committed!(self.cursor.expect_ident_or_keyword());
            committed!(self.cursor.expect(&TokenKind::Colon));

            if prop == self.known.over {
                over = Some(require!(
                    self,
                    self.parse_expr(),
                    "`over:` expression in for pattern"
                ));
            } else if prop == self.known.map {
                map = Some(require!(
                    self,
                    self.parse_expr(),
                    "`map:` expression in for pattern"
                ));
            } else if prop == self.known.match_ {
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
            } else if prop == self.known.default {
                default = Some(require!(
                    self,
                    self.parse_expr(),
                    "`default:` expression in for pattern"
                ));
            } else {
                let unknown = self.cursor.interner().lookup(prop);
                return ParseOutcome::consumed_err(
                    ParseError::new(
                        ori_diagnostic::ErrorCode::E1013,
                        format!("`for` pattern does not accept property `{unknown}`"),
                        self.cursor.previous_span(),
                    ),
                    start_span,
                );
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
                    "`for` pattern requires `over:` property",
                    span,
                ),
                start_span,
            );
        };
        let Some(arm) = match_arm else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    "`for` pattern requires `match:` property",
                    span,
                ),
                start_span,
            );
        };
        let Some(default) = default else {
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1013,
                    "`for` pattern requires `default:` property",
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
            type_args: ParsedTypeRange::EMPTY,
            span: start_span.merge(end_span),
        };

        let func_exp_id = self.arena.alloc_function_exp(func_exp);
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionExp(func_exp_id),
            start_span.merge(end_span),
        )))
    }

    /// Parse a channel expression: `channel<int>(buffer: 10)` or `channel(buffer: 10)`.
    ///
    /// Called after the channel identifier has been consumed by `parse_primary`.
    /// Parses optional generic type arguments, then named properties in parens.
    pub(crate) fn parse_channel_expr(&mut self, kind: FunctionExpKind) -> ParseOutcome<ExprId> {
        let start_span = self.cursor.previous_span();

        // Parse optional generic type arguments: <int>, <str>, <Result<int, str>>
        let type_args = self.parse_optional_generic_args_range();

        committed!(self.cursor.expect(&TokenKind::LParen));
        self.cursor.skip_newlines();

        let mut props: Vec<NamedExpr> = Vec::new();
        committed!(self.paren_series_direct(|p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(false);
            }

            if !p.cursor.is_named_arg_start() {
                return Err(crate::ParseError::new(
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
            type_args,
            span: start_span.merge(end_span),
        };

        let func_exp_id = self.arena.alloc_function_exp(func_exp);
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::FunctionExp(func_exp_id),
            start_span.merge(end_span),
        )))
    }
}
