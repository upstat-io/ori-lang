//! Operator Matching Helpers
//!
//! Helper methods for matching binary and unary operators during parsing.
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
//!
//! The match functions return `(BinaryOp, usize)` where usize is the number
//! of tokens to consume (1 for single-token ops, 2 for compound ops).

use crate::Parser;
use ori_ir::{BinaryOp, FunctionExpKind, TokenKind, UnaryOp};

impl Parser<'_> {
    /// Match equality operators: `==`, `!=`
    /// Returns `(op, token_count)` where `token_count` is always 1.
    pub(crate) fn match_equality_op(&self) -> Option<(BinaryOp, usize)> {
        match self.current_kind() {
            TokenKind::EqEq => Some((BinaryOp::Eq, 1)),
            TokenKind::NotEq => Some((BinaryOp::NotEq, 1)),
            _ => None,
        }
    }

    /// Match comparison operators: `<`, `<=`, `>`, `>=`
    ///
    /// Note: `>=` is detected as adjacent `>` and `=` tokens (2 tokens).
    /// Returns `(op, token_count)`.
    pub(crate) fn match_comparison_op(&self) -> Option<(BinaryOp, usize)> {
        match self.current_kind() {
            TokenKind::Lt => Some((BinaryOp::Lt, 1)),
            TokenKind::LtEq => Some((BinaryOp::LtEq, 1)),
            TokenKind::Gt => {
                // Check for compound >= (adjacent > and =)
                if self.is_greater_equal() {
                    Some((BinaryOp::GtEq, 2))
                } else {
                    Some((BinaryOp::Gt, 1))
                }
            }
            _ => None,
        }
    }

    /// Match shift operators: `<<`, `>>`
    ///
    /// Note: `>>` is detected as adjacent `>` and `>` tokens (2 tokens).
    /// Returns `(op, token_count)`.
    pub(crate) fn match_shift_op(&self) -> Option<(BinaryOp, usize)> {
        match self.current_kind() {
            TokenKind::Shl => Some((BinaryOp::Shl, 1)),
            TokenKind::Gt => {
                // Check for compound >> (adjacent > and >)
                if self.is_shift_right() {
                    Some((BinaryOp::Shr, 2))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Match additive operators: `+`, `-`
    /// Returns `(op, token_count)` where `token_count` is always 1.
    pub(crate) fn match_additive_op(&self) -> Option<(BinaryOp, usize)> {
        match self.current_kind() {
            TokenKind::Plus => Some((BinaryOp::Add, 1)),
            TokenKind::Minus => Some((BinaryOp::Sub, 1)),
            _ => None,
        }
    }

    /// Match multiplicative operators: `*`, `/`, `%`, `div`
    /// Returns `(op, token_count)` where `token_count` is always 1.
    pub(crate) fn match_multiplicative_op(&self) -> Option<(BinaryOp, usize)> {
        match self.current_kind() {
            TokenKind::Star => Some((BinaryOp::Mul, 1)),
            TokenKind::Slash => Some((BinaryOp::Div, 1)),
            TokenKind::Percent => Some((BinaryOp::Mod, 1)),
            TokenKind::Div => Some((BinaryOp::FloorDiv, 1)),
            _ => None,
        }
    }

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
