//! Operator Matching Helpers
//!
//! Helper methods for matching operators during parsing:
//! - `infix_binding_power`: Pratt parser binding power table for binary operators
//! - `match_unary_op`: Unary operator detection
//! - `match_function_exp_kind`: Pattern/function keyword detection
//!
//! # Specification
//!
//! - Syntax: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § EXPRESSIONS
//! - Semantics: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Precedence: `docs/ori_lang/0.1-alpha/spec/operator-rules.md` § Precedence Table
//!
//! ## Compound Operators
//!
//! The lexer produces individual `>` tokens to enable parsing nested generics
//! like `Result<Result<T, E>, E>`. In expression context, the parser combines
//! adjacent `>` tokens into compound operators:
//!
//! - `>` followed by `>` (no whitespace) → `>>` (right shift)
//! - `>` followed by `=` (no whitespace) → `>=` (greater-equal)

use super::bp;
use crate::Parser;
use ori_ir::{BinaryOp, FunctionExpKind, TokenKind, UnaryOp};

impl Parser<'_> {
    /// Get the infix binding power for the current token.
    ///
    /// Returns `(left_bp, right_bp, op, token_count)` or `None` if the
    /// current token is not a binary operator.
    ///
    /// - `left_bp`: compared against `min_bp` to decide if this operator binds here
    /// - `right_bp`: the `min_bp` for parsing the right operand
    /// - `op`: the `BinaryOp` variant
    /// - `token_count`: tokens to consume (1 for most, 2 for compound `>=`/`>>`)
    #[inline]
    pub(crate) fn infix_binding_power(&self) -> Option<(u8, u8, BinaryOp, usize)> {
        match self.current_kind() {
            // Right-associative: right_bp < left_bp
            TokenKind::DoubleQuestion => {
                Some((bp::COALESCE.0, bp::COALESCE.1, BinaryOp::Coalesce, 1))
            }
            // Left-associative (ascending precedence): right_bp = left_bp + 1
            TokenKind::PipePipe => Some((bp::OR.0, bp::OR.1, BinaryOp::Or, 1)),
            TokenKind::AmpAmp => Some((bp::AND.0, bp::AND.1, BinaryOp::And, 1)),
            TokenKind::Pipe => Some((bp::BIT_OR.0, bp::BIT_OR.1, BinaryOp::BitOr, 1)),
            TokenKind::Caret => Some((bp::BIT_XOR.0, bp::BIT_XOR.1, BinaryOp::BitXor, 1)),
            TokenKind::Amp => Some((bp::BIT_AND.0, bp::BIT_AND.1, BinaryOp::BitAnd, 1)),
            TokenKind::EqEq => Some((bp::EQUALITY.0, bp::EQUALITY.1, BinaryOp::Eq, 1)),
            TokenKind::NotEq => Some((bp::EQUALITY.0, bp::EQUALITY.1, BinaryOp::NotEq, 1)),
            TokenKind::Lt => Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::Lt, 1)),
            TokenKind::LtEq => Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::LtEq, 1)),
            TokenKind::Gt => {
                // Compound operators: adjacent `>` tokens combined in expression context
                if self.is_greater_equal() {
                    Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::GtEq, 2))
                } else if self.is_shift_right() {
                    Some((bp::SHIFT.0, bp::SHIFT.1, BinaryOp::Shr, 2))
                } else {
                    Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::Gt, 1))
                }
            }
            TokenKind::Shl => Some((bp::SHIFT.0, bp::SHIFT.1, BinaryOp::Shl, 1)),
            TokenKind::Plus => Some((bp::ADDITIVE.0, bp::ADDITIVE.1, BinaryOp::Add, 1)),
            TokenKind::Minus => Some((bp::ADDITIVE.0, bp::ADDITIVE.1, BinaryOp::Sub, 1)),
            TokenKind::Star => Some((bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, BinaryOp::Mul, 1)),
            TokenKind::Slash => {
                Some((bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, BinaryOp::Div, 1))
            }
            TokenKind::Percent => {
                Some((bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, BinaryOp::Mod, 1))
            }
            TokenKind::Div => Some((
                bp::MULTIPLICATIVE.0,
                bp::MULTIPLICATIVE.1,
                BinaryOp::FloorDiv,
                1,
            )),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn match_unary_op(&self) -> Option<UnaryOp> {
        match self.current_kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        }
    }

    /// Match `function_exp` keywords.
    pub(crate) fn match_function_exp_kind(&self) -> Option<FunctionExpKind> {
        // `with` has special capability provision syntax: with Ident = ...
        // Check for that case first
        if matches!(self.current_kind(), TokenKind::With) {
            if self.is_with_capability_syntax() {
                return None;
            }
            if self.next_is_lparen() {
                return Some(FunctionExpKind::With);
            }
            return None;
        }

        // All pattern/function keywords are context-sensitive:
        // only treated as keywords when followed by `(`
        if !self.next_is_lparen() {
            return None;
        }

        match self.current_kind() {
            // Compiler pattern keywords (require special syntax or static analysis)
            TokenKind::Recurse => Some(FunctionExpKind::Recurse),
            TokenKind::Parallel => Some(FunctionExpKind::Parallel),
            TokenKind::Spawn => Some(FunctionExpKind::Spawn),
            TokenKind::Timeout => Some(FunctionExpKind::Timeout),
            TokenKind::Cache => Some(FunctionExpKind::Cache),
            TokenKind::With => Some(FunctionExpKind::With),

            // Fundamental built-in functions
            TokenKind::Print => Some(FunctionExpKind::Print),
            TokenKind::Panic => Some(FunctionExpKind::Panic),
            TokenKind::Catch => Some(FunctionExpKind::Catch),
            TokenKind::Todo => Some(FunctionExpKind::Todo),
            TokenKind::Unreachable => Some(FunctionExpKind::Unreachable),
            _ => None,
        }
    }
}
