//! Expression parsing.
//!
//! This module extends Parser with methods for parsing expressions,
//! including binary operators, unary operators, function calls,
//! lambda expressions, and primary expressions.
//!
//! # Module Structure
//!
//! - `mod.rs`: Entry point (`parse_expr`) and binary operator precedence chain
//! - `operators.rs`: Operator matching helpers
//! - `primary.rs`: Literals, identifiers, variant constructors
//! - `postfix.rs`: Call, method call, field, index
//! - `patterns.rs`: run, try, match, for, `function_exp`

mod operators;
mod patterns;
mod postfix;
mod primary;

use crate::stack::ensure_sufficient_stack;
use crate::{ParseError, ParseResult, Parser};
use ori_ir::{BinaryOp, Expr, ExprId, ExprKind, TokenKind, UnaryOp};

/// Generate a binary operator precedence level parsing function.
///
/// Two forms:
/// - `(fn_name, next, token: tok, op: op)` — single-token operator levels
/// - `(fn_name, next, matcher)` — multi-token levels using a matcher method
macro_rules! parse_binary_level {
    // Single-token operator level: check for one specific token, use fixed op
    ($(#[doc = $doc:literal])* $fn_name:ident, $next:ident, token: $tok:expr, op: $op:expr) => {
        $(#[doc = $doc])*
        fn $fn_name(&mut self) -> Result<ExprId, ParseError> {
            let mut left = self.$next()?;
            while self.check(&$tok) {
                self.advance();
                let right = self.$next()?;
                let span = self.arena.get_expr(left).span
                    .merge(self.arena.get_expr(right).span);
                left = self.arena.alloc_expr(Expr::new(
                    ExprKind::Binary { op: $op, left, right },
                    span,
                ));
            }
            Ok(left)
        }
    };
    // Multi-token operator level: use a matcher method that returns Option<(BinaryOp, usize)>
    // where the usize is the number of tokens to consume (1 for single-token ops, 2 for compound ops like >> or >=)
    ($(#[doc = $doc:literal])* $fn_name:ident, $next:ident, $matcher:ident) => {
        $(#[doc = $doc])*
        fn $fn_name(&mut self) -> Result<ExprId, ParseError> {
            let mut left = self.$next()?;
            while let Some((op, token_count)) = self.$matcher() {
                for _ in 0..token_count {
                    self.advance();
                }
                let right = self.$next()?;
                let span = self.arena.get_expr(left).span
                    .merge(self.arena.get_expr(right).span);
                left = self.arena.alloc_expr(Expr::new(
                    ExprKind::Binary { op, left, right },
                    span,
                ));
            }
            Ok(left)
        }
    };
}

impl Parser<'_> {
    /// Parse an expression with progress tracking.
    ///
    /// Returns `Progress::None` if no tokens were consumed (not a valid expression start).
    /// Returns `Progress::Made` if tokens were consumed (success or error after consuming).
    #[allow(dead_code)] // Available for expression-level error recovery
    pub(crate) fn parse_expr_with_progress(&mut self) -> ParseResult<ExprId> {
        self.with_progress(|p| p.parse_expr())
    }

    /// Parse an expression.
    /// Handles assignment at the top level: `identifier = expression`
    ///
    /// Uses `ensure_sufficient_stack` to prevent stack overflow
    /// on deeply nested expressions.
    pub(crate) fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
        ensure_sufficient_stack(|| self.parse_expr_inner())
    }

    /// Inner expression parsing logic (wrapped by `parse_expr` for stack safety).
    fn parse_expr_inner(&mut self) -> Result<ExprId, ParseError> {
        let left = self.parse_binary_or()?;

        // Check for assignment (= but not == or =>)
        if self.check(&TokenKind::Eq) {
            let left_span = self.arena.get_expr(left).span;
            self.advance();
            let right = self.parse_expr()?;
            let right_span = self.arena.get_expr(right).span;
            let span = left_span.merge(right_span);
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Assign {
                    target: left,
                    value: right,
                },
                span,
            )));
        }

        Ok(left)
    }

    parse_binary_level! {
        /// Parse || (lowest precedence binary).
        parse_binary_or, parse_binary_and,
        token: TokenKind::PipePipe, op: BinaryOp::Or
    }

    parse_binary_level! {
        /// Parse && (logical and).
        parse_binary_and, parse_bitwise_or,
        token: TokenKind::AmpAmp, op: BinaryOp::And
    }

    parse_binary_level! {
        /// Parse | (bitwise or).
        parse_bitwise_or, parse_bitwise_xor,
        token: TokenKind::Pipe, op: BinaryOp::BitOr
    }

    parse_binary_level! {
        /// Parse ^ (bitwise xor).
        parse_bitwise_xor, parse_bitwise_and,
        token: TokenKind::Caret, op: BinaryOp::BitXor
    }

    parse_binary_level! {
        /// Parse & (bitwise and).
        parse_bitwise_and, parse_equality,
        token: TokenKind::Amp, op: BinaryOp::BitAnd
    }

    parse_binary_level! {
        /// Parse == and != (equality).
        parse_equality, parse_comparison, match_equality_op
    }

    parse_binary_level! {
        /// Parse comparison operators (<, >, <=, >=).
        parse_comparison, parse_range, match_comparison_op
    }

    // parse_range stays hand-written (unique range logic)

    /// Parse range operators (.. and ..=).
    fn parse_range(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_shift()?;

        if self.check(&TokenKind::DotDot) || self.check(&TokenKind::DotDotEq) {
            let inclusive = self.check(&TokenKind::DotDotEq);
            self.advance();

            let end = if self.check(&TokenKind::Comma)
                || self.check(&TokenKind::RParen)
                || self.check(&TokenKind::RBracket)
                || self.is_at_end()
            {
                None
            } else {
                Some(self.parse_shift()?)
            };

            let span = if let Some(end_expr) = end {
                self.arena
                    .get_expr(left)
                    .span
                    .merge(self.arena.get_expr(end_expr).span)
            } else {
                self.arena.get_expr(left).span.merge(self.previous_span())
            };

            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Range {
                    start: Some(left),
                    end,
                    inclusive,
                },
                span,
            ));
        }

        Ok(left)
    }

    parse_binary_level! {
        /// Parse << and >> (shift operators).
        parse_shift, parse_additive, match_shift_op
    }

    parse_binary_level! {
        /// Parse + and -.
        parse_additive, parse_multiplicative, match_additive_op
    }

    parse_binary_level! {
        /// Parse *, /, %.
        parse_multiplicative, parse_unary, match_multiplicative_op
    }

    /// Parse unary operators.
    ///
    /// When the operator is `-` and the next token is an integer literal,
    /// folds them into a single `ExprKind::Int` node. This allows
    /// `-9223372036854775808` (`i64::MIN`) to be represented directly.
    fn parse_unary(&mut self) -> Result<ExprId, ParseError> {
        /// Absolute value of `i64::MIN` as `u64` (for negation folding).
        const I64_MIN_ABS: u64 = 9_223_372_036_854_775_808;

        if let Some(op) = self.match_unary_op() {
            let start = self.current_span();

            // Fold negation with integer literals: `-42` → `ExprKind::Int(-42)`
            if op == UnaryOp::Neg {
                if let TokenKind::Int(n) = self.peek_next_kind() {
                    self.advance(); // consume `-`
                    let lit_span = self.current_span();
                    self.advance(); // consume integer literal
                    let span = start.merge(lit_span);

                    return if let Ok(signed) = i64::try_from(n) {
                        Ok(self
                            .arena
                            .alloc_expr(Expr::new(ExprKind::Int(-signed), span)))
                    } else if n == I64_MIN_ABS {
                        Ok(self
                            .arena
                            .alloc_expr(Expr::new(ExprKind::Int(i64::MIN), span)))
                    } else {
                        Err(ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large".to_string(),
                            span,
                        ))
                    };
                }
            }

            self.advance();
            let operand = self.parse_unary()?;

            let span = start.merge(self.arena.get_expr(operand).span);
            return Ok(self
                .arena
                .alloc_expr(Expr::new(ExprKind::Unary { op, operand }, span)));
        }

        self.parse_call()
    }
}
