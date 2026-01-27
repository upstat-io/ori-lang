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
mod primary;
mod postfix;
mod patterns;

use ori_ir::{BinaryOp, Expr, ExprId, ExprKind, TokenKind, UnaryOp};
use crate::{ParseError, Parser};
use crate::stack::ensure_sufficient_stack;

impl Parser<'_> {
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
                ExprKind::Assign { target: left, value: right },
                span,
            )));
        }

        Ok(left)
    }

    /// Parse || (lowest precedence binary).
    fn parse_binary_or(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_binary_and()?;

        while self.check(&TokenKind::PipePipe) {
            self.advance();
            let right = self.parse_binary_and()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::Or, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse && (logical and)
    fn parse_binary_and(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_or()?;

        while self.check(&TokenKind::AmpAmp) {
            self.advance();
            let right = self.parse_bitwise_or()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::And, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse | (bitwise or)
    fn parse_bitwise_or(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_xor()?;

        while self.check(&TokenKind::Pipe) {
            self.advance();
            let right = self.parse_bitwise_xor()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitOr, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse ^ (bitwise xor)
    fn parse_bitwise_xor(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_bitwise_and()?;

        while self.check(&TokenKind::Caret) {
            self.advance();
            let right = self.parse_bitwise_and()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitXor, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse & (bitwise and)
    fn parse_bitwise_and(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_equality()?;

        while self.check(&TokenKind::Amp) {
            self.advance();
            let right = self.parse_equality()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op: BinaryOp::BitAnd, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse == and != (equality)
    fn parse_equality(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_comparison()?;

        while let Some(op) = self.match_equality_op() {
            self.advance();
            let right = self.parse_comparison()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse comparison operators (<, >, <=, >=).
    fn parse_comparison(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_range()?;

        while let Some(op) = self.match_comparison_op() {
            self.advance();
            let right = self.parse_range()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse range operators (.. and ..=).
    fn parse_range(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_shift()?;

        if self.check(&TokenKind::DotDot) || self.check(&TokenKind::DotDotEq) {
            let inclusive = self.check(&TokenKind::DotDotEq);
            self.advance();

            let end = if self.check(&TokenKind::Comma) || self.check(&TokenKind::RParen) ||
                        self.check(&TokenKind::RBracket) || self.is_at_end() {
                None
            } else {
                Some(self.parse_shift()?)
            };

            let span = if let Some(end_expr) = end {
                self.arena.get_expr(left).span.merge(self.arena.get_expr(end_expr).span)
            } else {
                self.arena.get_expr(left).span.merge(self.previous_span())
            };

            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Range { start: Some(left), end, inclusive },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse << and >> (shift operators).
    fn parse_shift(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_additive()?;

        while let Some(op) = self.match_shift_op() {
            self.advance();
            let right = self.parse_additive()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse + and -.
    fn parse_additive(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_multiplicative()?;

        while let Some(op) = self.match_additive_op() {
            self.advance();
            let right = self.parse_multiplicative()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    /// Parse *, /, %.
    fn parse_multiplicative(&mut self) -> Result<ExprId, ParseError> {
        let mut left = self.parse_unary()?;

        while let Some(op) = self.match_multiplicative_op() {
            self.advance();
            let right = self.parse_unary()?;

            let span = self.arena.get_expr(left).span.merge(self.arena.get_expr(right).span);
            left = self.arena.alloc_expr(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
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
                        Ok(self.arena.alloc_expr(Expr::new(
                            ExprKind::Int(-signed),
                            span,
                        )))
                    } else if n == I64_MIN_ABS {
                        Ok(self.arena.alloc_expr(Expr::new(
                            ExprKind::Int(i64::MIN),
                            span,
                        )))
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
            return Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Unary { op, operand },
                span,
            )));
        }

        self.parse_call()
    }
}
