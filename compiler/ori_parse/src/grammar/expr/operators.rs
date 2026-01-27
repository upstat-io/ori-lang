//! Operator Matching Helpers
//!
//! Helper methods for matching binary and unary operators during parsing.

use ori_ir::{BinaryOp, FunctionExpKind, TokenKind, UnaryOp};
use crate::Parser;

impl Parser<'_> {
    pub(crate) fn match_equality_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::EqEq => Some(BinaryOp::Eq),
            TokenKind::NotEq => Some(BinaryOp::NotEq),
            _ => None,
        }
    }

    pub(crate) fn match_comparison_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Lt => Some(BinaryOp::Lt),
            TokenKind::LtEq => Some(BinaryOp::LtEq),
            TokenKind::Gt => Some(BinaryOp::Gt),
            TokenKind::GtEq => Some(BinaryOp::GtEq),
            _ => None,
        }
    }

    pub(crate) fn match_shift_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Shl => Some(BinaryOp::Shl),
            TokenKind::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    pub(crate) fn match_additive_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        }
    }

    pub(crate) fn match_multiplicative_op(&self) -> Option<BinaryOp> {
        match self.current_kind() {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Mod),
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

    /// Match `function_seq` keywords. Returns Some(true) for try, Some(false) for run.
    pub(crate) fn match_function_seq_kind(&self) -> Option<bool> {
        match self.current_kind() {
            TokenKind::Run => Some(false),
            TokenKind::Try => Some(true),
            _ => None,
        }
    }

    /// Match `function_exp` keywords.
    pub(crate) fn match_function_exp_kind(&self) -> Option<FunctionExpKind> {
        // Compiler pattern keywords (require special syntax or static analysis)
        match self.current_kind() {
            TokenKind::Recurse => return Some(FunctionExpKind::Recurse),
            TokenKind::Parallel => return Some(FunctionExpKind::Parallel),
            TokenKind::Spawn => return Some(FunctionExpKind::Spawn),
            TokenKind::Timeout => return Some(FunctionExpKind::Timeout),
            TokenKind::Cache => return Some(FunctionExpKind::Cache),
            TokenKind::With => {
                // Check if this is capability provision syntax: with Ident =
                // If so, don't treat it as a function_exp - it's a special expression
                if self.is_with_capability_syntax() {
                    return None;
                }
                return Some(FunctionExpKind::With);
            }
            _ => {}
        }

        // Fundamental built-in functions are context-sensitive:
        // only keywords when followed by `(`
        if !self.next_is_lparen() {
            return None;
        }

        match self.current_kind() {
            TokenKind::Print => Some(FunctionExpKind::Print),
            TokenKind::Panic => Some(FunctionExpKind::Panic),
            _ => None,
        }
    }
}
