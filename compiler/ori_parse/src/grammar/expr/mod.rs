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
//! - `blocks.rs`: Shared block-statement parsing (used by block and try)
//! - `operators.rs`: Operator matching helpers and binding power table
//! - `primary.rs`: Literals, identifiers, variant constructors
//! - `postfix.rs`: Call, method call, field, index
//! - `patterns.rs`: try, match, for, `function_exp`

mod blocks;
mod operators;
mod patterns;
mod postfix;
mod primary;

use crate::{chain, committed, require, ParseError, ParseOutcome, Parser};
use ori_ir::{BinaryOp, Expr, ExprId, ExprKind, TokenKind, UnaryOp};
use ori_stack::ensure_sufficient_stack;
use tracing::trace;

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
    /// Parse an expression.
    /// Handles assignment at the top level: `identifier = expression`
    ///
    /// Uses `ensure_sufficient_stack` to prevent stack overflow
    /// on deeply nested expressions.
    pub(crate) fn parse_expr(&mut self) -> ParseOutcome<ExprId> {
        trace!(
            pos = self.cursor.position(),
            kind = self.cursor.current_kind().display_name(),
            "parse_expr"
        );
        ensure_sufficient_stack(|| self.parse_expr_inner())
    }

    /// Parse an expression without allowing top-level assignment.
    ///
    /// Use this for contexts where `=` is a delimiter rather than an operator,
    /// such as guard clauses (`if condition = body`).
    pub(crate) fn parse_non_assign_expr(&mut self) -> ParseOutcome<ExprId> {
        ensure_sufficient_stack(|| self.parse_binary_pratt(0))
    }

    /// Parse an expression without comparison operators (`<`, `>`, `<=`, `>=`).
    ///
    /// Use this in contexts where `<` and `>` are delimiters, not operators,
    /// such as const generic default values: `<$N: int = 10>`.
    pub(crate) fn parse_non_comparison_expr(&mut self) -> ParseOutcome<ExprId> {
        ensure_sufficient_stack(|| self.parse_binary_pratt(bp::ABOVE_COMPARISON))
    }

    /// Inner expression parsing logic (wrapped by `parse_expr` for stack safety).
    fn parse_expr_inner(&mut self) -> ParseOutcome<ExprId> {
        let left = chain!(self, self.parse_binary_pratt(0));

        // Check for assignment (= but not == or =>)
        if self.cursor.check(&TokenKind::Eq) {
            let left_span = self.arena.get_expr(left).span;
            self.cursor.advance();
            let right = require!(self, self.parse_expr(), "expression after `=`");
            let right_span = self.arena.get_expr(right).span;
            let span = left_span.merge(right_span);
            return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Assign {
                    target: left,
                    value: right,
                },
                span,
            )));
        }

        // Check for compound assignment (+=, -=, *=, /=, %=, @=, &=, |=, ^=, <<=, &&=, ||=)
        // Desugars: `x op= y` → `x = x op y`
        if let Some(op) = self.compound_assign_op() {
            return self.desugar_compound_assign(left, op, 1);
        }

        // Check for >>= (synthesized from three adjacent > > = tokens)
        if self.cursor.is_shift_right_assign() {
            return self.desugar_compound_assign(left, BinaryOp::Shr, 3);
        }

        ParseOutcome::consumed_ok(left)
    }

    /// Desugar a compound assignment expression.
    ///
    /// `x op= y` → `Assign { target: x, value: Binary { op, left: x_copy, right: y } }`
    ///
    /// `token_count` is how many tokens to consume for the operator:
    /// - 1 for single-token operators (`+=`, `-=`, etc.)
    /// - 3 for `>>=` (three adjacent `>` `>` `=` tokens)
    fn desugar_compound_assign(
        &mut self,
        target: ExprId,
        op: BinaryOp,
        token_count: u8,
    ) -> ParseOutcome<ExprId> {
        let target_expr = self.arena.get_expr(target);
        let left_span = target_expr.span;

        // Consume the compound assignment operator token(s)
        match token_count {
            1 => {
                self.cursor.advance();
            }
            3 => {
                self.cursor.consume_triple();
            }
            _ => unreachable!(),
        }

        // Parse the right-hand side
        let rhs = require!(
            self,
            self.parse_expr(),
            "expression after compound assignment"
        );
        let rhs_span = self.arena.get_expr(rhs).span;

        // Duplicate the target expression as the left operand of the binary op.
        // ExprKind is Copy, so this is a cheap re-allocation in the arena.
        let left_copy = self
            .arena
            .alloc_expr(Expr::new(target_expr.kind, left_span));

        // Build the binary expression: target op rhs
        let binary_span = left_span.merge(rhs_span);
        let binary = self.arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op,
                left: left_copy,
                right: rhs,
            },
            binary_span,
        ));

        // Build the assignment: target = binary
        let assign_span = left_span.merge(rhs_span);
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Assign {
                target,
                value: binary,
            },
            assign_span,
        )))
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
    fn parse_binary_pratt(&mut self, min_bp: u8) -> ParseOutcome<ExprId> {
        let mut left = chain!(self, self.parse_unary());

        // Track whether we've already parsed a range in this call.
        // Range operators don't chain: `1..10..20` is invalid.
        let mut parsed_range = false;

        loop {
            self.cursor.skip_newlines();

            // Range operators (non-standard binary with optional end/step).
            // Checked before standard operators because `..`/`..=` are not in
            // the infix binding power table (they need special parsing).
            if !parsed_range
                && min_bp <= bp::RANGE
                && matches!(
                    self.cursor.current_kind(),
                    TokenKind::DotDot | TokenKind::DotDotEq
                )
            {
                left = committed!(self.parse_range_continuation(left));
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
                    self.cursor.advance();
                }
                let right = require!(self, self.parse_binary_pratt(r_bp), "right operand");
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

        ParseOutcome::consumed_ok(left)
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
        let inclusive = matches!(self.cursor.current_kind(), TokenKind::DotDotEq);
        self.cursor.advance();

        // Parse end expression (optional for open-ended ranges like 0..)
        // The token after `..` determines whether this is an open-ended range:
        // delimiters (`,`, `)`, `]`, `}`), the `by` step keyword, or keywords
        // that follow ranges in control flow contexts (`do`, `yield`, `then`).
        let end = if matches!(
            self.cursor.current_kind(),
            TokenKind::Comma
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::By
                | TokenKind::Do
                | TokenKind::Yield
                | TokenKind::Then
        ) || self.cursor.is_at_end()
        {
            ExprId::INVALID
        } else {
            self.parse_binary_pratt(bp::SHIFT.0).into_result()?
        };

        // Parse optional step: `by <expr>`
        let step = if matches!(self.cursor.current_kind(), TokenKind::By) {
            self.cursor.advance();
            self.parse_binary_pratt(bp::SHIFT.0).into_result()?
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
            self.arena
                .get_expr(left)
                .span
                .merge(self.cursor.previous_span())
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
    fn parse_unary(&mut self) -> ParseOutcome<ExprId> {
        /// Absolute value of `i64::MIN` as `u64` (for negation folding).
        const I64_MIN_ABS: u64 = 9_223_372_036_854_775_808;

        if let Some(op) = self.match_unary_op() {
            let start = self.cursor.current_span();

            // Fold negation with integer literals: `-42` → `ExprKind::Int(-42)`
            // After folding, still apply postfix operators for cases like `-100 as float`.
            if op == UnaryOp::Neg {
                if let TokenKind::Int(n) = *self.cursor.peek_next_kind() {
                    self.cursor.advance(); // consume `-`
                    let lit_span = self.cursor.current_span();
                    self.cursor.advance(); // consume integer literal
                    let span = start.merge(lit_span);

                    let expr = if let Ok(signed) = i64::try_from(n) {
                        self.arena
                            .alloc_expr(Expr::new(ExprKind::Int(-signed), span))
                    } else if n == I64_MIN_ABS {
                        self.arena
                            .alloc_expr(Expr::new(ExprKind::Int(i64::MIN), span))
                    } else {
                        return ParseOutcome::consumed_err(
                            ParseError::new(
                                ori_diagnostic::ErrorCode::E1002,
                                "integer literal too large".to_string(),
                                span,
                            ),
                            span,
                        );
                    };
                    // Apply postfix operators (e.g., `as type`, `?`, method calls)
                    let result = committed!(self.apply_postfix_ops(expr));
                    return ParseOutcome::consumed_ok(result);
                }
            }

            self.cursor.advance();
            let operand = require!(self, self.parse_unary(), "operand after unary operator");

            let span = start.merge(self.arena.get_expr(operand).span);
            return ParseOutcome::consumed_ok(
                self.arena
                    .alloc_expr(Expr::new(ExprKind::Unary { op, operand }, span)),
            );
        }

        self.parse_call()
    }
}
