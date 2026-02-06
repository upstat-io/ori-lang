//! Expression parsing.
//!
//! This module extends Parser with methods for parsing expressions,
//! including binary operators, unary operators, function calls,
//! lambda expressions, and primary expressions.
//!
//! # Specification
//!
//! - Syntax: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § EXPRESSIONS
//! - Semantics: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/0.1-alpha/spec/09-expressions.md`
//!
//! # Module Structure
//!
//! - `mod.rs`: Entry point (`parse_expr`) and Pratt parser for binary operators
//! - `operators.rs`: Operator matching helpers and binding power table
//! - `primary.rs`: Literals, identifiers, variant constructors
//! - `postfix.rs`: Call, method call, field, index
//! - `patterns.rs`: run, try, match, for, `function_exp`

mod operators;
mod patterns;
mod postfix;
mod primary;

use crate::{ParseError, ParseOutcome, Parser};
use ori_ir::{Expr, ExprId, ExprKind, TokenKind, UnaryOp};
use ori_stack::ensure_sufficient_stack;

/// Binding power constants for the Pratt parser.
///
/// Left-associative operators use (even, odd) pairs: `(N, N+1)`.
/// Right-associative operators use (odd, even) pairs: `(N+1, N)`.
/// Higher values bind tighter.
pub(super) mod bp {
    /// Coalesce `??` (right-associative)
    pub const COALESCE: (u8, u8) = (2, 1);
    /// Logical or `||`
    pub const OR: (u8, u8) = (3, 4);
    /// Logical and `&&`
    pub const AND: (u8, u8) = (5, 6);
    /// Bitwise or `|`
    pub const BIT_OR: (u8, u8) = (7, 8);
    /// Bitwise xor `^`
    pub const BIT_XOR: (u8, u8) = (9, 10);
    /// Bitwise and `&`
    pub const BIT_AND: (u8, u8) = (11, 12);
    /// Equality `==` `!=`
    pub const EQUALITY: (u8, u8) = (13, 14);
    /// Comparison `<` `>` `<=` `>=`
    pub const COMPARISON: (u8, u8) = (15, 16);
    /// Range `..` `..=` (non-associative, special handling)
    pub const RANGE: u8 = 17;
    /// Shift `<<` `>>`
    pub const SHIFT: (u8, u8) = (19, 20);
    /// Additive `+` `-`
    pub const ADDITIVE: (u8, u8) = (21, 22);
    /// Multiplicative `*` `/` `%` `div`
    pub const MULTIPLICATIVE: (u8, u8) = (23, 24);

    /// Minimum binding power for parsing without comparison operators.
    /// Used by `parse_non_comparison_expr` for contexts where `<`/`>`
    /// are delimiters (e.g., const generic defaults `<$N: int = 10>`).
    pub const ABOVE_COMPARISON: u8 = RANGE;
}

impl Parser<'_> {
    /// Parse an expression with outcome tracking.
    ///
    /// Returns `EmptyOk`/`EmptyErr` if no tokens were consumed.
    /// Returns `ConsumedOk`/`ConsumedErr` if tokens were consumed.
    #[allow(dead_code)] // Available for expression-level error recovery
    pub(crate) fn parse_expr_with_outcome(&mut self) -> ParseOutcome<ExprId> {
        self.with_outcome(Self::parse_expr)
    }

    /// Parse an expression.
    /// Handles assignment at the top level: `identifier = expression`
    ///
    /// Uses `ensure_sufficient_stack` to prevent stack overflow
    /// on deeply nested expressions.
    pub(crate) fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
        ensure_sufficient_stack(|| self.parse_expr_inner())
    }

    /// Parse an expression without allowing top-level assignment.
    ///
    /// Use this for contexts where `=` is a delimiter rather than an operator,
    /// such as guard clauses (`if condition = body`).
    pub(crate) fn parse_non_assign_expr(&mut self) -> Result<ExprId, ParseError> {
        ensure_sufficient_stack(|| self.parse_binary_pratt(0))
    }

    /// Parse an expression without comparison operators (`<`, `>`, `<=`, `>=`).
    ///
    /// Use this in contexts where `<` and `>` are delimiters, not operators,
    /// such as const generic default values: `<$N: int = 10>`.
    pub(crate) fn parse_non_comparison_expr(&mut self) -> Result<ExprId, ParseError> {
        ensure_sufficient_stack(|| self.parse_binary_pratt(bp::ABOVE_COMPARISON))
    }

    /// Inner expression parsing logic (wrapped by `parse_expr` for stack safety).
    fn parse_expr_inner(&mut self) -> Result<ExprId, ParseError> {
        let left = self.parse_binary_pratt(0)?;

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

    /// Parse binary expressions using a Pratt parser.
    ///
    /// Replaces the recursive descent precedence chain (12 levels of function
    /// calls per primary expression) with a single loop that uses a binding
    /// power table. This reduces function call overhead from ~30 calls per
    /// simple expression to ~4.
    ///
    /// `min_bp` controls which operators are parsed at this level:
    /// - `0`: all binary operators (entry point for full expressions)
    /// - `bp::ABOVE_COMPARISON`: range + shift + arithmetic only
    #[inline]
    fn parse_binary_pratt(&mut self, min_bp: u8) -> Result<ExprId, ParseError> {
        let mut left = self.parse_unary()?;

        // Track whether we've already parsed a range in this call.
        // Range operators don't chain: `1..10..20` is invalid.
        let mut parsed_range = false;

        loop {
            self.skip_newlines();

            // Range operators (non-standard binary with optional end/step).
            // Checked before standard operators because `..`/`..=` are not in
            // the infix binding power table (they need special parsing).
            if !parsed_range
                && min_bp <= bp::RANGE
                && matches!(self.current_kind(), TokenKind::DotDot | TokenKind::DotDotEq)
            {
                left = self.parse_range_continuation(left)?;
                parsed_range = true;
                // Continue to allow lower-precedence operators to wrap the range
                // (e.g., `1..10 == other_range`).
                continue;
            }

            // Standard binary operators via Pratt binding power.
            if let Some((l_bp, r_bp, op, token_count)) = self.infix_binding_power() {
                if l_bp < min_bp {
                    break;
                }
                for _ in 0..token_count {
                    self.advance();
                }
                let right = self.parse_binary_pratt(r_bp)?;
                let span = self
                    .arena
                    .get_expr(left)
                    .span
                    .merge(self.arena.get_expr(right).span);
                left = self
                    .arena
                    .alloc_expr(Expr::new(ExprKind::Binary { op, left, right }, span));
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parse the continuation of a range expression (after the left operand).
    ///
    /// Grammar: `left ( ".." | "..=" ) [ end ] [ "by" step ]`
    ///
    /// End and step expressions are parsed at shift precedence level,
    /// matching the original recursive descent behavior where `parse_range`
    /// called `parse_shift` for both operands.
    #[inline]
    fn parse_range_continuation(&mut self, left: ExprId) -> Result<ExprId, ParseError> {
        let inclusive = matches!(self.current_kind(), TokenKind::DotDotEq);
        self.advance();

        // Parse end expression (optional for open-ended ranges like 0..)
        let end = if matches!(
            self.current_kind(),
            TokenKind::Comma | TokenKind::RParen | TokenKind::RBracket | TokenKind::By
        ) || self.is_at_end()
        {
            ExprId::INVALID
        } else {
            self.parse_binary_pratt(bp::SHIFT.0)?
        };

        // Parse optional step: `by <expr>`
        let step = if matches!(self.current_kind(), TokenKind::By) {
            self.advance();
            self.parse_binary_pratt(bp::SHIFT.0)?
        } else {
            ExprId::INVALID
        };

        // Compute span from start to end/step
        let span = if step.is_present() {
            self.arena
                .get_expr(left)
                .span
                .merge(self.arena.get_expr(step).span)
        } else if end.is_present() {
            self.arena
                .get_expr(left)
                .span
                .merge(self.arena.get_expr(end).span)
        } else {
            self.arena.get_expr(left).span.merge(self.previous_span())
        };

        Ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Range {
                start: left,
                end,
                step,
                inclusive,
            },
            span,
        )))
    }

    /// Parse unary operators.
    ///
    /// When the operator is `-` and the next token is an integer literal,
    /// folds them into a single `ExprKind::Int` node. This allows
    /// `-9223372036854775808` (`i64::MIN`) to be represented directly.
    #[inline]
    fn parse_unary(&mut self) -> Result<ExprId, ParseError> {
        /// Absolute value of `i64::MIN` as `u64` (for negation folding).
        const I64_MIN_ABS: u64 = 9_223_372_036_854_775_808;

        if let Some(op) = self.match_unary_op() {
            let start = self.current_span();

            // Fold negation with integer literals: `-42` → `ExprKind::Int(-42)`
            // After folding, still apply postfix operators for cases like `-100 as float`.
            if op == UnaryOp::Neg {
                if let TokenKind::Int(n) = *self.peek_next_kind() {
                    self.advance(); // consume `-`
                    let lit_span = self.current_span();
                    self.advance(); // consume integer literal
                    let span = start.merge(lit_span);

                    let expr = if let Ok(signed) = i64::try_from(n) {
                        self.arena
                            .alloc_expr(Expr::new(ExprKind::Int(-signed), span))
                    } else if n == I64_MIN_ABS {
                        self.arena
                            .alloc_expr(Expr::new(ExprKind::Int(i64::MIN), span))
                    } else {
                        return Err(ParseError::new(
                            ori_diagnostic::ErrorCode::E1002,
                            "integer literal too large".to_string(),
                            span,
                        ));
                    };
                    // Apply postfix operators (e.g., `as type`, `?`, method calls)
                    return self.apply_postfix_ops(expr);
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
