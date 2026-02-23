//! Primary Expression Parsing
//!
//! Parses literals, identifiers, variant constructors, parenthesized expressions,
//! lists, if expressions, and let expressions.
//!
//! Uses `one_of!` macro for token-dispatched alternatives with automatic
//! backtracking. Each sub-parser returns `EmptyErr` when its leading token
//! doesn't match, enabling clean ordered alternation.

mod bindings;
mod collections;
mod control_flow;
mod helpers;
mod literals;
mod specials;

use crate::{one_of, ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, TokenKind};
use tracing::{debug, trace};

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
            let span = self.cursor.current_span();
            self.cursor.advance();
            return ParseOutcome::consumed_err(
                ParseError::new(
                    ori_diagnostic::ErrorCode::E1002,
                    "`run()` syntax has been removed",
                    span,
                )
                .with_help("Use block expressions instead: `{ stmt; stmt; result }`"),
                span,
            );
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
            TokenKind::TAG_IDENT | TokenKind::TAG_SUSPEND | TokenKind::TAG_EXTERN => {
                return self.parse_ident_primary()
            }
            TokenKind::TAG_UNSAFE => return self.parse_unsafe_expr(),
            TokenKind::TAG_LPAREN => return self.parse_parenthesized(),
            TokenKind::TAG_LBRACKET => return self.parse_list_literal(),
            TokenKind::TAG_LBRACE => return self.parse_block_or_map(),
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
            self.parse_block_or_map(),
            self.parse_if_expr(),
            self.parse_let_expr(),
            self.parse_loop_expr(),
            self.parse_for_loop(),
            self.parse_control_flow_primary(),
            self.parse_template_literal(),
        )
    }
}
