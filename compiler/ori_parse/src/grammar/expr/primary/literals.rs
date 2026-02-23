//! Literal and identifier primary expression parsing.
//!
//! Handles literal tokens (int, float, string, char, bool, duration, size),
//! identifier-like tokens (idents, soft keywords, type keywords), variant
//! constructors (Some, None, Ok, Err), and miscellaneous single-token
//! primaries ($name, #length).

use crate::recovery::TokenSet;
use crate::{committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{DurationUnit, Expr, ExprId, ExprKind, Name, SizeUnit, TokenKind};
use tracing::{debug, trace};

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
///
/// Note: `Unsafe` is NOT listed here — it is parsed as `unsafe { block_body }`
/// expression form, not as an identifier. See `parse_unsafe_expr()`.
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
    .with(TokenKind::Suspend)
    .with(TokenKind::Extern);

/// Tokens that start a variant constructor.
const VARIANT_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Some)
    .with(TokenKind::None)
    .with(TokenKind::Ok)
    .with(TokenKind::Err);

/// Tokens for miscellaneous single-token primaries.
const MISC_PRIMARY_TOKENS: TokenSet = TokenSet::new()
    .with(TokenKind::Dollar)
    .with(TokenKind::Hash);

impl Parser<'_> {
    /// Parse literal tokens: `Int`, `Float`, `True`, `False`, `String`, `Char`,
    /// `Duration`, `Size`.
    ///
    /// Returns `EmptyErr` if the current token is not a literal.
    pub(super) fn parse_literal_primary(&mut self) -> ParseOutcome<ExprId> {
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
    /// (`Print`, `Panic`, `SelfLower`, `Suspend`, `Extern`),
    /// and type conversion keywords (`IntType`, `FloatType`, etc.).
    ///
    /// Note: `Cache`, `Catch`, `Parallel`, `Spawn`, `Recurse`, `Timeout` are
    /// handled by the lexer's `(` lookahead — they appear as `Ident` tokens
    /// when not in keyword position, so no conversion is needed here.
    ///
    /// Returns `EmptyErr` if the current token is not identifier-like.
    pub(super) fn parse_ident_primary(&mut self) -> ParseOutcome<ExprId> {
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
    pub(super) fn parse_variant_primary(&mut self) -> ParseOutcome<ExprId> {
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
    pub(super) fn parse_misc_primary(&mut self) -> ParseOutcome<ExprId> {
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
}
