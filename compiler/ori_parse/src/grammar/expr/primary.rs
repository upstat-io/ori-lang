//! Primary Expression Parsing
//!
//! Parses literals, identifiers, variant constructors, parenthesized expressions,
//! lists, if expressions, and let expressions.
//!
//! Uses `one_of!` macro for token-dispatched alternatives with automatic
//! backtracking. Each sub-parser returns `EmptyErr` when its leading token
//! doesn't match, enabling clean ordered alternation.

use crate::recovery::TokenSet;
use crate::{committed, one_of, require, ParseError, ParseOutcome, Parser};
use ori_ir::{
    BindingPattern, DurationUnit, Expr, ExprId, ExprKind, ExprRange, FieldBinding, FunctionExpKind,
    Name, Param, ParamRange, ParsedTypeId, SizeUnit, TemplatePart, TokenKind,
};
use tracing::{debug, trace};

// These constants define which tokens each sub-parser expects. When a sub-parser
// fails without consuming input, it returns EmptyErr with its token set. The
// `one_of!` macro accumulates these sets across alternatives, producing error
// messages like "expected integer, identifier, `(`, or `[`".

/// Tokens that start a literal expression.
const LITERAL_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Int(0))
    .with(TokenKind::Float(0))
    .with(TokenKind::True)
    .with(TokenKind::False)
    .with(TokenKind::String(Name::EMPTY))
    .with(TokenKind::Char('\0'))
    .with(TokenKind::Duration(0, DurationUnit::Nanoseconds))
    .with(TokenKind::Size(0, SizeUnit::Bytes));

/// Tokens that start an identifier-like expression (idents + soft keywords).
///
/// Note: `Cache`, `Catch`, `Parallel`, `Spawn`, `Recurse`, `Timeout` are NOT
/// listed here — the lexer only produces these tokens when followed by `(`,
/// so they never appear in identifier position.
const IDENT_LIKE_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Ident(Name::EMPTY))
    .with(TokenKind::Print)
    .with(TokenKind::Panic)
    .with(TokenKind::SelfLower)
    .with(TokenKind::IntType)
    .with(TokenKind::FloatType)
    .with(TokenKind::StrType)
    .with(TokenKind::BoolType)
    .with(TokenKind::CharType)
    .with(TokenKind::ByteType)
    .with(TokenKind::Unsafe)
    .with(TokenKind::Suspend)
    .with(TokenKind::Extern);

/// Tokens that start a variant constructor.
const VARIANT_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Some)
    .with(TokenKind::None)
    .with(TokenKind::Ok)
    .with(TokenKind::Err);

/// Tokens that start a control flow expression.
const CONTROL_FLOW_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Break)
    .with(TokenKind::Continue)
    .with(TokenKind::Return);

/// Tokens that start a template literal expression.
const TEMPLATE_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::TemplateFull(Name::EMPTY))
    .with(TokenKind::TemplateHead(Name::EMPTY));

/// Tokens for miscellaneous single-token primaries.
const MISC_PRIMARY_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Dollar)
    .with(TokenKind::Hash);

impl Parser<'_> {
    /// Parse primary expressions with outcome tracking.
    ///
    /// Uses `one_of!` for token-dispatched alternatives with automatic backtracking.
    /// Context-sensitive keywords that need multi-token lookahead remain as an if-chain
    /// before the `one_of!` dispatch.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive primary expression token dispatch — one branch per token kind"
    )]
    pub(crate) fn parse_primary(&mut self) -> ParseOutcome<ExprId> {
        debug!(
            pos = self.cursor.position(),
            tag = self.cursor.current_tag(),
            kind = self.cursor.current_kind().display_name(),
            span_start = self.cursor.current_span().start,
            span_end = self.cursor.current_span().end,
            "parse_primary"
        );

        // === Context-sensitive keywords requiring multi-token lookahead ===
        //
        // These stay as an if-chain because they need `next_is_lparen()`,
        // `is_with_capability_syntax()`, or `match_function_exp_kind()` before
        // deciding, and they advance before calling the sub-parser.
        if self.cursor.check(&TokenKind::Run) {
            trace!("parse_primary -> Run");
            self.cursor.advance();
            return self.parse_run();
        }
        if self.cursor.check(&TokenKind::Try) {
            trace!("parse_primary -> Try");
            self.cursor.advance();
            return self.parse_try();
        }
        if self.cursor.check(&TokenKind::Match) {
            trace!("parse_primary -> Match");
            self.cursor.advance();
            return self.parse_match_expr();
        }
        if self.cursor.check(&TokenKind::For) && self.cursor.next_is_lparen() {
            self.cursor.advance();
            return self.parse_for_pattern();
        }
        if self.cursor.check(&TokenKind::For) {
            return self.parse_for_loop();
        }
        if self.cursor.check(&TokenKind::With) && self.cursor.is_with_capability_syntax() {
            return self.parse_with_capability();
        }
        // Channel constructors are context-sensitive identifiers (not lexer keywords).
        // Detect `channel(`, `channel<`, `channel_in(`, etc. and parse as function_exp
        // with optional generic type arguments.
        if let TokenKind::Ident(name) = *self.cursor.current_kind() {
            if let Some(channel_kind) = self.match_channel_kind(name) {
                let next = self.cursor.peek_next_kind();
                if matches!(next, TokenKind::LParen | TokenKind::Lt) {
                    self.cursor.advance();
                    return self.parse_channel_expr(channel_kind);
                }
            }
        }
        if let Some(kind) = self.match_function_exp_kind() {
            self.cursor.advance();
            return self.parse_function_exp(kind);
        }

        // === Fast path: tag-based direct dispatch ===
        //
        // For the most common primary tokens, dispatch directly to the correct
        // sub-parser without going through one_of!'s snapshot/restore/TokenSet
        // machinery. Each sub-parser has its own guard that returns EmptyErr
        // if the token doesn't match, so correctness is preserved — but we
        // know these tags map 1:1 to a specific sub-parser, so the guard
        // always succeeds and we skip the overhead of probing alternatives.
        trace!(
            tag = self.cursor.current_tag(),
            "parse_primary fast-path dispatch"
        );
        match self.cursor.current_tag() {
            TokenKind::TAG_INT
            | TokenKind::TAG_FLOAT
            | TokenKind::TAG_STRING
            | TokenKind::TAG_CHAR
            | TokenKind::TAG_TRUE
            | TokenKind::TAG_FALSE
            | TokenKind::TAG_DURATION
            | TokenKind::TAG_SIZE => {
                return self.parse_literal_primary();
            }
            TokenKind::TAG_IDENT
            | TokenKind::TAG_UNSAFE
            | TokenKind::TAG_SUSPEND
            | TokenKind::TAG_EXTERN => return self.parse_ident_primary(),
            TokenKind::TAG_LPAREN => return self.parse_parenthesized(),
            TokenKind::TAG_LBRACKET => return self.parse_list_literal(),
            TokenKind::TAG_LBRACE => return self.parse_map_literal(),
            TokenKind::TAG_IF => return self.parse_if_expr(),
            TokenKind::TAG_LET => return self.parse_let_expr(),
            TokenKind::TAG_LOOP => return self.parse_loop_expr(),
            TokenKind::TAG_SOME | TokenKind::TAG_NONE | TokenKind::TAG_OK | TokenKind::TAG_ERR => {
                return self.parse_variant_primary()
            }
            TokenKind::TAG_DOLLAR | TokenKind::TAG_HASH => {
                return self.parse_misc_primary();
            }
            TokenKind::TAG_BREAK | TokenKind::TAG_CONTINUE | TokenKind::TAG_RETURN => {
                return self.parse_control_flow_primary();
            }
            TokenKind::TAG_TEMPLATE_FULL | TokenKind::TAG_TEMPLATE_HEAD => {
                return self.parse_template_literal();
            }
            // Error tokens from the lexer — silently consume and produce Error expr.
            // The real diagnostic was already emitted by the lex error pipeline.
            TokenKind::TAG_ERROR => {
                let span = self.cursor.current_span();
                self.cursor.advance();
                return ParseOutcome::consumed_ok(
                    self.arena.alloc_expr(Expr::new(ExprKind::Error, span)),
                );
            }
            _ => {
                trace!(
                    tag = self.cursor.current_tag(),
                    kind = self.cursor.current_kind().display_name(),
                    "parse_primary fast-path: no match, falling through to one_of!"
                );
            }
        }

        // === Fallback: full one_of! dispatch ===
        //
        // Handles soft keywords and other rare cases not covered by the fast
        // path (e.g., `print`/`panic` as identifiers, `for` as loop).
        // Also provides accumulated expected-token error messages on failure.
        one_of!(
            self,
            self.parse_literal_primary(),
            self.parse_ident_primary(),
            self.parse_variant_primary(),
            self.parse_misc_primary(),
            self.parse_parenthesized(),
            self.parse_list_literal(),
            self.parse_map_literal(),
            self.parse_if_expr(),
            self.parse_let_expr(),
            self.parse_loop_expr(),
            self.parse_for_loop(),
            self.parse_control_flow_primary(),
            self.parse_template_literal(),
        )
    }

    // === Extracted sub-parsers for one_of! dispatch ===

    /// Parse literal tokens: `Int`, `Float`, `True`, `False`, `String`, `Char`,
    /// `Duration`, `Size`.
    ///
    /// Returns `EmptyErr` if the current token is not a literal.
    fn parse_literal_primary(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        match *self.cursor.current_kind() {
            TokenKind::Int(n) => {
                self.cursor.advance();
                let Ok(value) = i64::try_from(n) else {
                    return ParseOutcome::consumed_err(
                        ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large",
                            span,
                        ),
                        span,
                    );
                };
                ParseOutcome::consumed_ok(
                    self.arena.alloc_expr(Expr::new(ExprKind::Int(value), span)),
                )
            }
            TokenKind::Float(bits) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Float(bits), span)),
                )
            }
            TokenKind::True => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena.alloc_expr(Expr::new(ExprKind::Bool(true), span)),
                )
            }
            TokenKind::False => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Bool(false), span)),
                )
            }
            TokenKind::String(name) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::String(name), span)),
                )
            }
            TokenKind::Char(c) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(ExprKind::Char(c), span)))
            }
            TokenKind::Duration(value, unit) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Duration { value, unit }, span)),
                )
            }
            TokenKind::Size(value, unit) => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Size { value, unit }, span)),
                )
            }
            _ => ParseOutcome::empty_err(LITERAL_TOKENS, self.cursor.current_span().start as usize),
        }
    }

    /// Parse identifier-like tokens: `Ident`, soft keywords used as identifiers
    /// (`Print`, `Panic`, `SelfLower`, `Unsafe`, `Suspend`, `Extern`),
    /// and type conversion keywords (`IntType`, `FloatType`, etc.).
    ///
    /// Note: `Cache`, `Catch`, `Parallel`, `Spawn`, `Recurse`, `Timeout` are
    /// handled by the lexer's `(` lookahead — they appear as `Ident` tokens
    /// when not in keyword position, so no conversion is needed here.
    ///
    /// Returns `EmptyErr` if the current token is not identifier-like.
    fn parse_ident_primary(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();

        trace!(
            kind = self.cursor.current_kind().display_name(),
            span_start = span.start,
            "parse_ident_primary"
        );

        // Map token to (intern_str, should_advance_first) — all follow the same pattern:
        // intern the name, advance, return Ident expression.
        let name = match *self.cursor.current_kind() {
            TokenKind::Ident(name) => {
                self.cursor.advance();
                return ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ident(name), span)),
                );
            }
            TokenKind::Print => "print",
            TokenKind::Panic => "panic",
            TokenKind::SelfLower => "self",
            TokenKind::IntType => "int",
            TokenKind::FloatType => "float",
            TokenKind::StrType => "str",
            TokenKind::BoolType => "bool",
            TokenKind::CharType => "char",
            TokenKind::ByteType => "byte",
            TokenKind::Unsafe => "unsafe",
            TokenKind::Suspend => "suspend",
            TokenKind::Extern => "extern",
            _ => {
                debug!(
                    kind = self.cursor.current_kind().display_name(),
                    tag = self.cursor.current_tag(),
                    pos = self.cursor.position(),
                    span_start = self.cursor.current_span().start,
                    "parse_ident_primary: unhandled token kind"
                );
                return ParseOutcome::empty_err(
                    IDENT_LIKE_TOKENS,
                    self.cursor.current_span().start as usize,
                );
            }
        };

        let interned = self.cursor.interner().intern(name);
        self.cursor.advance();
        ParseOutcome::consumed_ok(
            self.arena
                .alloc_expr(Expr::new(ExprKind::Ident(interned), span)),
        )
    }

    /// Parse variant constructors: `Some(expr)`, `None`, `Ok(expr)`, `Err(expr)`.
    ///
    /// Returns `EmptyErr` if the current token is not a variant keyword.
    fn parse_variant_primary(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        match *self.cursor.current_kind() {
            TokenKind::Some => {
                self.cursor.advance();
                committed!(self.cursor.expect(&TokenKind::LParen));
                let inner = require!(self, self.parse_expr(), "expression inside `Some(...)`");
                let end_span = self.cursor.current_span();
                committed!(self.cursor.expect(&TokenKind::RParen));
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Some(inner), span.merge(end_span))),
                )
            }
            TokenKind::None => {
                self.cursor.advance();
                ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(ExprKind::None, span)))
            }
            TokenKind::Ok => {
                self.cursor.advance();
                let inner = if self.cursor.check(&TokenKind::LParen) {
                    self.cursor.advance();
                    let expr = require!(self, self.parse_expr(), "expression inside `Ok(...)`");
                    committed!(self.cursor.expect(&TokenKind::RParen));
                    expr
                } else {
                    ExprId::INVALID
                };
                let end_span = self.cursor.previous_span();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Ok(inner), span.merge(end_span))),
                )
            }
            TokenKind::Err => {
                self.cursor.advance();
                let inner = if self.cursor.check(&TokenKind::LParen) {
                    self.cursor.advance();
                    let expr = require!(self, self.parse_expr(), "expression inside `Err(...)`");
                    committed!(self.cursor.expect(&TokenKind::RParen));
                    expr
                } else {
                    ExprId::INVALID
                };
                let end_span = self.cursor.previous_span();
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Err(inner), span.merge(end_span))),
                )
            }
            _ => ParseOutcome::empty_err(VARIANT_TOKENS, self.cursor.current_span().start as usize),
        }
    }

    /// Parse miscellaneous single-token primaries: `$name` (const ref), `#` (hash length).
    ///
    /// Returns `EmptyErr` if the current token is not `$` or `#`.
    fn parse_misc_primary(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        match *self.cursor.current_kind() {
            TokenKind::Dollar => {
                self.cursor.advance();
                let name = committed!(self.cursor.expect_ident());
                let full_span = span.merge(self.cursor.previous_span());
                ParseOutcome::consumed_ok(
                    self.arena
                        .alloc_expr(Expr::new(ExprKind::Const(name), full_span)),
                )
            }
            TokenKind::Hash => {
                if self.context.in_index() {
                    self.cursor.advance();
                    ParseOutcome::consumed_ok(
                        self.arena.alloc_expr(Expr::new(ExprKind::HashLength, span)),
                    )
                } else {
                    ParseOutcome::empty_err(
                        MISC_PRIMARY_TOKENS,
                        self.cursor.current_span().start as usize,
                    )
                }
            }
            _ => ParseOutcome::empty_err(
                MISC_PRIMARY_TOKENS,
                self.cursor.current_span().start as usize,
            ),
        }
    }

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
    fn parse_control_flow_primary(&mut self) -> ParseOutcome<ExprId> {
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

    // === Guarded existing sub-parsers ===
    //
    // These already exist but now have a guard that returns EmptyErr when the
    // leading token doesn't match. This makes them safe for one_of! dispatch.

    /// Parse parenthesized expression, tuple, or lambda.
    ///
    /// Guard: returns `EmptyErr` if not at `(`.
    fn parse_parenthesized(&mut self) -> ParseOutcome<ExprId> {
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
    fn parse_list_literal(&mut self) -> ParseOutcome<ExprId> {
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

    /// Parse map literal: `{ key: value, ... }`, `{ ...base, key: value }`, or `{}`.
    ///
    /// Guard: returns `EmptyErr` if not at `{`.
    fn parse_map_literal(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::LBrace) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::LBrace,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(
            crate::ErrorContext::MapLiteral,
            Self::parse_map_literal_body,
        )
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

    /// Parse if expression.
    ///
    /// Guard: returns `EmptyErr` if not at `if`.
    fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::If) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::If,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(crate::ErrorContext::IfExpression, Self::parse_if_expr_body)
    }

    fn parse_if_expr_body(&mut self) -> ParseOutcome<ExprId> {
        use crate::ParseContext;

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

    /// Parse let expression.
    ///
    /// Per spec (05-variables.md): Bindings are mutable by default.
    /// - `let x = ...` → mutable (default)
    /// - `let $x = ...` → immutable ($ prefix)
    ///
    /// Guard: returns `EmptyErr` if not at `let`.
    fn parse_let_expr(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::Let) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Let,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(crate::ErrorContext::LetPattern, Self::parse_let_expr_body)
    }

    fn parse_let_expr_body(&mut self) -> ParseOutcome<ExprId> {
        let span = self.cursor.current_span();
        self.cursor.advance();

        // Per spec: mutable by default, $ prefix for immutable
        // - `let x = ...` → mutable (default)
        // - `let $x = ...` → immutable
        let mutable = if self.cursor.check(&TokenKind::Dollar) {
            self.cursor.advance();
            false
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
        let init = require!(self, self.parse_expr(), "initializer expression");

        let end_span = self.arena.get_expr(init).span;
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Let {
                pattern: pattern_id,
                ty,
                init,
                mutable,
            },
            span.merge(end_span),
        )))
    }

    /// Parse a binding pattern.
    ///
    /// Per grammar: `binding_pattern = [ "$" ] identifier | "_" | "{" ... "}" | ...`
    /// The `$` prefix marks the binding as immutable.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive binding pattern dispatch across destructuring, name, and wildcard forms"
    )]
    pub(crate) fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        // Handle $ prefix for immutable bindings: $x, $name, etc.
        if self.cursor.check(&TokenKind::Dollar) {
            self.cursor.advance();
            if let Some(name_str) = self.cursor.soft_keyword_to_name() {
                let name = self.cursor.interner().intern(name_str);
                self.cursor.advance();
                return Ok(BindingPattern::Name {
                    name,
                    mutable: false,
                });
            }
            if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                self.cursor.advance();
                return Ok(BindingPattern::Name {
                    name,
                    mutable: false,
                });
            }
            return Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "expected identifier after $, found {}",
                    self.cursor.current_kind().display_name()
                ),
                self.cursor.current_span(),
            ));
        }

        if let Some(name_str) = self.cursor.soft_keyword_to_name() {
            let name = self.cursor.interner().intern(name_str);
            self.cursor.advance();
            return Ok(BindingPattern::Name {
                name,
                mutable: true,
            });
        }

        match *self.cursor.current_kind() {
            TokenKind::Ident(name) => {
                self.cursor.advance();
                Ok(BindingPattern::Name {
                    name,
                    mutable: true,
                })
            }
            TokenKind::Underscore => {
                self.cursor.advance();
                Ok(BindingPattern::Wildcard)
            }
            TokenKind::LParen => {
                use crate::series::SeriesConfig;
                self.cursor.advance();
                let patterns: Vec<BindingPattern> =
                    self.series(&SeriesConfig::comma(TokenKind::RParen).no_newlines(), |p| {
                        if p.cursor.check(&TokenKind::RParen) {
                            Ok(None)
                        } else {
                            Ok(Some(p.parse_binding_pattern()?))
                        }
                    })?;
                self.cursor.expect(&TokenKind::RParen)?;
                Ok(BindingPattern::Tuple(patterns))
            }
            TokenKind::LBrace => {
                use crate::series::SeriesConfig;
                self.cursor.advance();
                let fields: Vec<FieldBinding> =
                    self.series(&SeriesConfig::comma(TokenKind::RBrace).no_newlines(), |p| {
                        if p.cursor.check(&TokenKind::RBrace) {
                            return Ok(None);
                        }

                        // Per grammar: field_binding = [ "$" ] identifier [ ":" binding_pattern ]
                        let mutable = if p.cursor.check(&TokenKind::Dollar) {
                            p.cursor.advance();
                            false
                        } else {
                            true
                        };

                        let field_name = p.cursor.expect_ident()?;

                        let pattern = if p.cursor.check(&TokenKind::Colon) {
                            p.cursor.advance();
                            Some(p.parse_binding_pattern()?)
                        } else {
                            None // Shorthand: { x } binds field x to variable x
                        };

                        Ok(Some(FieldBinding {
                            name: field_name,
                            mutable,
                            pattern,
                        }))
                    })?;
                self.cursor.expect(&TokenKind::RBrace)?;
                Ok(BindingPattern::Struct { fields })
            }
            TokenKind::LBracket => {
                // List pattern is special: has optional ..rest at the end
                // Cannot use simple series combinator
                self.cursor.advance();
                let mut elements = Vec::new();
                let mut rest = None;

                while !self.cursor.check(&TokenKind::RBracket) && !self.cursor.is_at_end() {
                    if self.cursor.check(&TokenKind::DotDot) {
                        self.cursor.advance();
                        if let TokenKind::Ident(name) = *self.cursor.current_kind() {
                            rest = Some(name);
                            self.cursor.advance();
                        }
                        break;
                    }
                    elements.push(self.parse_binding_pattern()?);
                    if !self.cursor.check(&TokenKind::RBracket)
                        && !self.cursor.check(&TokenKind::DotDot)
                    {
                        self.cursor.expect(&TokenKind::Comma)?;
                    }
                }
                self.cursor.expect(&TokenKind::RBracket)?;
                Ok(BindingPattern::List { elements, rest })
            }
            _ => Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                format!(
                    "expected binding pattern, found {}",
                    self.cursor.current_kind().display_name()
                ),
                self.cursor.current_span(),
            )),
        }
    }

    /// Parse template literal: `` `text` `` or `` `text{expr}more{expr:fmt}end` ``
    ///
    /// Template literals use backticks and support interpolation with `{expr}`.
    /// An optional format spec can follow the expression: `{expr:format_spec}`.
    ///
    /// Returns `EmptyErr` if the current token is not `TemplateFull` or `TemplateHead`.
    fn parse_template_literal(&mut self) -> ParseOutcome<ExprId> {
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

    /// Parse capability provision: `with Capability = Provider in body`
    fn parse_with_capability(&mut self) -> ParseOutcome<ExprId> {
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

    /// Parse for loop: `for x in items do body` or `for x in items yield body`
    ///
    /// Also supports optional guard: `for x in items if condition do body`
    ///
    /// Guard: returns `EmptyErr` if not at `for`.
    fn parse_for_loop(&mut self) -> ParseOutcome<ExprId> {
        if !self.cursor.check(&TokenKind::For) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::For,
                self.cursor.current_span().start as usize,
            );
        }
        self.in_error_context(crate::ErrorContext::ForLoop, Self::parse_for_loop_body)
    }

    fn parse_for_loop_body(&mut self) -> ParseOutcome<ExprId> {
        use crate::context::ParseContext;

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

    /// Parse loop expression: `loop(body)` or `loop:label(body)`
    ///
    /// The body is evaluated repeatedly until a `break` is encountered.
    ///
    /// Guard: returns `EmptyErr` if not at `loop`.
    fn parse_loop_expr(&mut self) -> ParseOutcome<ExprId> {
        use crate::context::ParseContext;

        if !self.cursor.check(&TokenKind::Loop) {
            return ParseOutcome::empty_err_expected(
                &TokenKind::Loop,
                self.cursor.current_span().start as usize,
            );
        }

        let span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::Loop));

        // Parse optional label: loop:label
        let label = self.parse_optional_label();

        committed!(self.cursor.expect(&TokenKind::LParen));
        self.cursor.skip_newlines();

        // Parse body expression with IN_LOOP context (enables break/continue)
        let body = require!(
            self,
            self.with_context(ParseContext::IN_LOOP, Self::parse_expr),
            "loop body"
        );

        self.cursor.skip_newlines();
        let end_span = self.cursor.current_span();
        committed!(self.cursor.expect(&TokenKind::RParen));

        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Loop { label, body },
            span.merge(end_span),
        )))
    }

    /// Check if typed lambda params.
    pub(crate) fn is_typed_lambda_params(&self) -> bool {
        let is_ident_like = matches!(self.cursor.current_kind(), TokenKind::Ident(_))
            || self.cursor.soft_keyword_to_name().is_some();
        if !is_ident_like {
            return false;
        }
        self.cursor.next_is_colon()
    }

    /// Convert expressions to lambda parameters.
    pub(crate) fn exprs_to_params(&mut self, exprs: &[ExprId]) -> Result<ParamRange, ParseError> {
        let mut params = Vec::new();
        for &expr_id in exprs {
            let expr = self.arena.get_expr(expr_id);
            match &expr.kind {
                ExprKind::Ident(name) => {
                    params.push(Param {
                        name: *name,
                        pattern: None,
                        ty: None,
                        default: None,
                        is_variadic: false,
                        span: expr.span,
                    });
                }
                _ => {
                    return Err(ParseError::new(
                        ori_diagnostic::ErrorCode::E1002,
                        "expected identifier for lambda parameter",
                        expr.span,
                    ));
                }
            }
        }
        Ok(self.arena.alloc_params(params))
    }

    /// Check if an identifier name maps to a channel constructor kind.
    fn match_channel_kind(&self, name: Name) -> Option<FunctionExpKind> {
        if name == self.known.channel {
            Some(FunctionExpKind::Channel)
        } else if name == self.known.channel_in {
            Some(FunctionExpKind::ChannelIn)
        } else if name == self.known.channel_out {
            Some(FunctionExpKind::ChannelOut)
        } else if name == self.known.channel_all {
            Some(FunctionExpKind::ChannelAll)
        } else {
            None
        }
    }
}
