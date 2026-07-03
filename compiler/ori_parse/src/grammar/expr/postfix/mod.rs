//! Postfix Expression Parsing
//!
//! Parses call, method call, field access, index expressions, and struct literals.

mod struct_lit;

use crate::context::ParseContext;
use crate::error::ErrorContext;
use crate::{chain, committed, ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, Param, ParsedTypeId, ParsedTypeRange, TokenKind};

/// A postfix operator, identified by the token that starts it. Single source of
/// truth for the postfix-token set: `POSTFIX_BITSET` (the O(1) fast-exit gate) is
/// derived from `from_tag`, and `apply_postfix_ops` dispatches through `from_tag`
/// with a compiler-enforced exhaustive match — so the gate and the dispatch
/// cannot drift (a new variant forces a `from_tag` arm + a dispatch arm, and the
/// bitset picks it up automatically). Mirrors the `OPER_TABLE` exhaustiveness
/// discipline in `operators.rs`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PostfixOp {
    /// `(` — function call.
    Call,
    /// `.` — field access / method call.
    Dot,
    /// `[` — index.
    Index,
    /// `{` — struct literal (only after a bare identifier in struct-lit context).
    StructLit,
    /// `?` — try.
    Try,
    /// `as` — cast.
    Cast,
    /// `->` — single-parameter lambda shorthand (only after a bare identifier).
    Lambda,
}

impl PostfixOp {
    /// Map a token tag to the postfix op it can start, if any. The SSOT for the
    /// postfix-token set — every other postfix-tag fact derives from this.
    const fn from_tag(tag: u8) -> Option<PostfixOp> {
        match tag {
            TokenKind::TAG_LPAREN => Some(PostfixOp::Call),
            TokenKind::TAG_DOT => Some(PostfixOp::Dot),
            TokenKind::TAG_LBRACKET => Some(PostfixOp::Index),
            TokenKind::TAG_LBRACE => Some(PostfixOp::StructLit),
            TokenKind::TAG_QUESTION => Some(PostfixOp::Try),
            TokenKind::TAG_AS => Some(PostfixOp::Cast),
            TokenKind::TAG_ARROW => Some(PostfixOp::Lambda),
            _ => None,
        }
    }

    /// The token tag that starts this op (inverse of `from_tag`); test-only —
    /// used to pin the `from_tag` ↔ dispatch ↔ bitset round-trip.
    #[cfg(test)]
    const fn start_tag(self) -> u8 {
        match self {
            PostfixOp::Call => TokenKind::TAG_LPAREN,
            PostfixOp::Dot => TokenKind::TAG_DOT,
            PostfixOp::Index => TokenKind::TAG_LBRACKET,
            PostfixOp::StructLit => TokenKind::TAG_LBRACE,
            PostfixOp::Try => TokenKind::TAG_QUESTION,
            PostfixOp::Cast => TokenKind::TAG_AS,
            PostfixOp::Lambda => TokenKind::TAG_ARROW,
        }
    }

    /// Every variant — the exhaustiveness fixture for the round-trip test.
    #[cfg(test)]
    const ALL: [PostfixOp; 7] = [
        PostfixOp::Call,
        PostfixOp::Dot,
        PostfixOp::Index,
        PostfixOp::StructLit,
        PostfixOp::Try,
        PostfixOp::Cast,
        PostfixOp::Lambda,
    ];
}

/// Bitset of tags that can start a postfix operation, derived from
/// `PostfixOp::from_tag` so it never drifts from the dispatch. Bit N is set iff
/// tag N starts a postfix op. Uses two u64s to cover tags 0-127.
const POSTFIX_BITSET: [u64; 2] = {
    let mut bits = [0u64; 2];
    let mut tag = 0u8;
    while tag < 128 {
        if PostfixOp::from_tag(tag).is_some() {
            bits[(tag / 64) as usize] |= 1u64 << (tag % 64);
        }
        tag += 1;
    }
    bits
};

/// O(1) bitset check for postfix-starting tokens.
#[inline]
fn is_postfix_tag(tag: u8) -> bool {
    let idx = tag as usize;
    if idx >= 128 {
        return false;
    }
    (POSTFIX_BITSET[idx / 64] >> (idx % 64)) & 1 != 0
}

impl Parser<'_> {
    /// Parse function calls and field access.
    ///
    /// Returns `EmptyErr` if no primary expression is found (propagated from `parse_primary`).
    /// Returns `ConsumedErr` if postfix parsing fails after consuming tokens.
    #[inline]
    pub(crate) fn parse_call(&mut self) -> ParseOutcome<ExprId> {
        let expr = chain!(self, self.parse_primary());
        let result = committed!(self.apply_postfix_ops(expr));
        ParseOutcome::consumed_ok(result)
    }

    /// Apply postfix operators to an expression.
    ///
    /// This is factored out from `parse_call()` to be reusable in `parse_unary()`
    /// for cases like `-100 as float` where negative integer folding produces
    /// an expression that still needs postfix operator handling.
    #[inline]
    pub(crate) fn apply_postfix_ops(&mut self, mut expr: ExprId) -> Result<ExprId, ParseError> {
        loop {
            // Skip newlines to allow method chaining across lines
            self.cursor.skip_newlines();

            // Fast exit: O(1) bitset check — if current tag can't start any
            // postfix op, break immediately without testing each alternative.
            let tag = self.cursor.current_tag();
            if !is_postfix_tag(tag) {
                break;
            }
            // `is_postfix_tag` guarantees membership, so `from_tag` is total here;
            // the `else` is unreachable in practice (both derive from `from_tag`).
            let Some(op) = PostfixOp::from_tag(tag) else {
                break;
            };

            // Exhaustive over `PostfixOp` — a new variant is a compile error here
            // (registration-sync discipline).
            match op {
                PostfixOp::Call => {
                    self.cursor.advance();
                    expr = self.in_error_context_result(ErrorContext::FunctionCall, |p| {
                        p.parse_postfix_call(expr)
                    })?;
                }
                PostfixOp::Dot => {
                    self.cursor.advance();
                    expr = self.parse_postfix_dot(expr)?;
                }
                PostfixOp::Index => {
                    self.cursor.advance();
                    let index = self.in_error_context_result(
                        ErrorContext::IndexExpression,
                        Self::parse_index_expr,
                    )?;
                    self.cursor.expect(&TokenKind::RBracket)?;
                    let span = self
                        .arena
                        .get_expr(expr)
                        .span
                        .merge(self.cursor.previous_span());
                    expr = self.arena.alloc_expr(Expr::new(
                        ExprKind::Index {
                            receiver: expr,
                            index,
                        },
                        span,
                    ));
                }
                PostfixOp::StructLit => {
                    if !self.allows_struct_lit() {
                        break;
                    }
                    let expr_data = self.arena.get_expr(expr);
                    if let ExprKind::Ident(name) = &expr_data.kind {
                        let struct_name = *name;
                        let start_span = expr_data.span;
                        self.cursor.advance();
                        expr = self.in_error_context_result(ErrorContext::StructLiteral, |p| {
                            p.parse_postfix_struct_lit(struct_name, start_span)
                        })?;
                    } else {
                        break;
                    }
                }
                PostfixOp::Try => {
                    self.cursor.advance();
                    let span = self
                        .arena
                        .get_expr(expr)
                        .span
                        .merge(self.cursor.previous_span());
                    expr = self.arena.alloc_expr(Expr::new(ExprKind::Try(expr), span));
                }
                PostfixOp::Cast => {
                    self.cursor.advance();
                    expr = self.in_error_context_result(ErrorContext::TypeAnnotation, |p| {
                        p.parse_postfix_cast(expr)
                    })?;
                }
                PostfixOp::Lambda => {
                    let expr_data = self.arena.get_expr(expr);
                    if let ExprKind::Ident(name) = &expr_data.kind {
                        let param_span = expr_data.span;
                        let param_name = *name;
                        self.cursor.advance();
                        expr = self.in_error_context_result(ErrorContext::Closure, |p| {
                            p.parse_postfix_lambda(param_name, param_span)
                        })?;
                    }
                    break;
                }
            }
        }

        Ok(expr)
    }

    /// Parse a function call after the opening `(` has been consumed.
    fn parse_postfix_call(&mut self, func: ExprId) -> Result<ExprId, ParseError> {
        let (call_args, has_named) = self.parse_call_args()?;
        self.cursor.expect(&TokenKind::RParen)?;

        let call_span = self
            .arena
            .get_expr(func)
            .span
            .merge(self.cursor.previous_span());

        if has_named {
            let args_range = self.arena.alloc_call_args(call_args);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::CallNamed {
                    func,
                    args: args_range,
                },
                call_span,
            )))
        } else {
            let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
            let args_list = self.arena.alloc_expr_list_inline(&args);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Call {
                    func,
                    args: args_list,
                },
                call_span,
            )))
        }
    }

    /// Parse dot access (field, method call, or method-style match) after `.` consumed.
    fn parse_postfix_dot(&mut self, receiver: ExprId) -> Result<ExprId, ParseError> {
        // Method-style match: expr.match(pattern -> body, ...)
        if self.cursor.check(&TokenKind::Match) {
            let start_span = self.arena.get_expr(receiver).span;
            self.cursor.advance();
            self.cursor.expect(&TokenKind::LParen)?;
            return self.parse_match_arms_with_scrutinee(receiver, start_span);
        }

        let field = self.cursor.expect_member_name()?;

        // Speculative call-site type args (`receiver.method<T>(args)`); see
        // parse_call_site_type_args's doc for the commit-on-`(` disambiguation.
        let call_type_args = self.parse_call_site_type_args();

        if self.cursor.check(&TokenKind::LParen) {
            // Method call
            self.cursor.advance();
            let node = self.in_error_context_result(ErrorContext::MethodCall, |p| {
                let (call_args, has_named) = p.parse_call_args()?;
                p.cursor.expect(&TokenKind::RParen)?;

                let span = p
                    .arena
                    .get_expr(receiver)
                    .span
                    .merge(p.cursor.previous_span());

                if has_named {
                    let args_range = p.arena.alloc_call_args(call_args);
                    Ok(p.arena.alloc_expr(Expr::new(
                        ExprKind::MethodCallNamed {
                            receiver,
                            method: field,
                            args: args_range,
                        },
                        span,
                    )))
                } else {
                    let args: Vec<ExprId> = call_args.into_iter().map(|a| a.value).collect();
                    let args_list = p.arena.alloc_expr_list_inline(&args);
                    Ok(p.arena.alloc_expr(Expr::new(
                        ExprKind::MethodCall {
                            receiver,
                            method: field,
                            args: args_list,
                        },
                        span,
                    )))
                }
            })?;
            // Record the call-site type arguments in the arena side-table keyed by
            // the method-call node (no-op when none were written).
            self.arena.set_method_call_type_args(node, call_type_args);
            Ok(node)
        } else {
            // Field access — no context needed (single token, can't fail after member name)
            let span = self
                .arena
                .get_expr(receiver)
                .span
                .merge(self.cursor.previous_span());
            Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Field { receiver, field }, span)))
        }
    }

    /// Speculatively parse a call-site type-argument list (`<T, U>`) after a member
    /// name. Returns the parsed range ONLY when it is immediately followed by `(`
    /// (i.e. a generic method call); otherwise restores the cursor and returns
    /// `EMPTY` so `<` is parsed as the less-than operator. Reuses the snapshot
    /// (SN-3) + `IN_TYPE` context (CF-1) machinery and the existing
    /// `parse_optional_generic_args_range`. The trailing-`(` requirement is the
    /// parse-time disambiguation half; the resolve-time comparison fallback is
    /// not yet wired.
    fn parse_call_site_type_args(&mut self) -> ParsedTypeRange {
        self.try_parse_type_args(&TokenKind::LParen)
    }

    /// Shared speculative `<type_args>`-commit core for BOTH call-site turbofish
    /// seams: the postfix `receiver.method<T>(args)` form (commit on `(`) and the
    /// primary-position type-path `Type<args>.method(...)` form (commit on `.`).
    /// When the current token is `<`, snapshot (SN-3) → parse a `type_args` list in
    /// `IN_TYPE` context (CF-1) → commit the parsed range ONLY when it is immediately
    /// followed by `commit_on`; otherwise restore so `<` stays the less-than operator.
    /// One disambiguation mechanism covers both positions (per the call-site
    /// method-generics grammar-alignment proposal).
    pub(crate) fn try_parse_type_args(&mut self, commit_on: &TokenKind) -> ParsedTypeRange {
        if !self.cursor.check(&TokenKind::Lt) {
            return ParsedTypeRange::EMPTY;
        }
        let snap = self.snapshot();
        let parsed = self.with_context(ParseContext::IN_TYPE, |p| {
            p.parse_optional_generic_args_range()
        });
        if !parsed.is_empty() && self.cursor.check(commit_on) {
            parsed
        } else {
            self.restore(snap);
            ParsedTypeRange::EMPTY
        }
    }

    /// Parse a type cast (`as type` or `as? type`) after `as` consumed.
    fn parse_postfix_cast(&mut self, expr: ExprId) -> Result<ExprId, ParseError> {
        let fallible = if self.cursor.check(&TokenKind::Question) {
            self.cursor.advance();
            true
        } else {
            false
        };

        let ty = self.parse_type().ok_or_else(|| {
            ParseError::new(
                ori_diagnostic::ErrorCode::E1002,
                "expected type after `as`".to_string(),
                self.cursor.current_span(),
            )
        })?;

        let ty_id = self.arena.alloc_parsed_type(ty);
        let span = self
            .arena
            .get_expr(expr)
            .span
            .merge(self.cursor.previous_span());
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Cast {
                expr,
                ty: ty_id,
                fallible,
            },
            span,
        )))
    }

    /// Parse a single-param lambda shorthand (`x -> body`) after `->` consumed.
    fn parse_postfix_lambda(
        &mut self,
        param_name: ori_ir::Name,
        param_span: ori_ir::Span,
    ) -> Result<ExprId, ParseError> {
        self.cursor.skip_newlines(); // `a ->\n body` shorthand-lambda body may follow on the next line
        let body = self.parse_expr().into_result()?;
        let end_span = self.arena.get_expr(body).span;
        let params = self.arena.alloc_params(vec![Param {
            name: param_name,
            pattern: None,
            ty: None,
            default: None,
            is_variadic: false,
            span: param_span,
        }]);
        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Lambda {
                params,
                ret_ty: ParsedTypeId::INVALID,
                body,
            },
            param_span.merge(end_span),
        )))
    }

    /// Parse an index expression, where `#` represents the length of the receiver.
    ///
    /// Inside `[...]`, the `#` symbol is parsed as `ExprKind::HashLength`,
    /// which is resolved to the receiver's length during evaluation.
    fn parse_index_expr(&mut self) -> Result<ExprId, ParseError> {
        use crate::context::ParseContext;
        self.with_context(ParseContext::IN_INDEX, Self::parse_expr)
            .into_result()
    }
}

#[cfg(test)]
mod tests;
