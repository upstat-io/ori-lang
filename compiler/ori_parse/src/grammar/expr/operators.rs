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

/// Packed operator info for the static lookup table.
///
/// Stores left/right binding power, `BinaryOp` variant (as u8), and token count
/// in 4 bytes. A `left_bp` of 0 means "not an operator".
#[derive(Clone, Copy)]
struct OperInfo {
    left_bp: u8,
    right_bp: u8,
    op: u8,
    token_count: u8,
}

impl OperInfo {
    const NONE: Self = OperInfo {
        left_bp: 0,
        right_bp: 0,
        op: 0,
        token_count: 0,
    };

    const fn new(left_bp: u8, right_bp: u8, op: u8, token_count: u8) -> Self {
        OperInfo {
            left_bp,
            right_bp,
            op,
            token_count,
        }
    }
}

/// Convert a `u8` op index back to `BinaryOp`.
///
/// The indices must match what's stored in `OPER_TABLE`.
#[inline]
fn op_from_u8(op: u8) -> BinaryOp {
    match op {
        0 => BinaryOp::Coalesce,
        1 => BinaryOp::Or,
        2 => BinaryOp::And,
        3 => BinaryOp::BitOr,
        4 => BinaryOp::BitXor,
        5 => BinaryOp::BitAnd,
        6 => BinaryOp::Eq,
        7 => BinaryOp::NotEq,
        8 => BinaryOp::Lt,
        9 => BinaryOp::LtEq,
        10 => BinaryOp::Gt,
        11 => BinaryOp::Shl,
        12 => BinaryOp::Add,
        13 => BinaryOp::Sub,
        14 => BinaryOp::Mul,
        15 => BinaryOp::Div,
        16 => BinaryOp::Mod,
        17 => BinaryOp::FloorDiv,
        _ => unreachable!(),
    }
}

/// Static lookup table indexed by token discriminant tag.
///
/// For each tag that represents a binary operator, stores the binding powers,
/// op variant, and token count. Non-operator tags have `left_bp == 0`.
///
/// The `Gt` token (tag 96) is special: it maps to `BinaryOp::Gt` here,
/// but compound `>=` and `>>` are handled separately at the call site.
static OPER_TABLE: [OperInfo; 128] = {
    let mut table = [OperInfo::NONE; 128];

    table[TokenKind::TAG_DOUBLE_QUESTION as usize] =
        OperInfo::new(bp::COALESCE.0, bp::COALESCE.1, 0, 1);
    table[TokenKind::TAG_PIPEPIPE as usize] = OperInfo::new(bp::OR.0, bp::OR.1, 1, 1);
    table[TokenKind::TAG_AMPAMP as usize] = OperInfo::new(bp::AND.0, bp::AND.1, 2, 1);
    table[TokenKind::TAG_PIPE as usize] = OperInfo::new(bp::BIT_OR.0, bp::BIT_OR.1, 3, 1);
    table[TokenKind::TAG_CARET as usize] = OperInfo::new(bp::BIT_XOR.0, bp::BIT_XOR.1, 4, 1);
    table[TokenKind::TAG_AMP as usize] = OperInfo::new(bp::BIT_AND.0, bp::BIT_AND.1, 5, 1);
    table[TokenKind::TAG_EQEQ as usize] = OperInfo::new(bp::EQUALITY.0, bp::EQUALITY.1, 6, 1);
    table[TokenKind::TAG_NOTEQ as usize] = OperInfo::new(bp::EQUALITY.0, bp::EQUALITY.1, 7, 1);
    table[TokenKind::TAG_LT as usize] = OperInfo::new(bp::COMPARISON.0, bp::COMPARISON.1, 8, 1);
    table[TokenKind::TAG_LTEQ as usize] = OperInfo::new(bp::COMPARISON.0, bp::COMPARISON.1, 9, 1);
    table[TokenKind::TAG_GT as usize] = OperInfo::new(bp::COMPARISON.0, bp::COMPARISON.1, 10, 1);
    table[TokenKind::TAG_SHL as usize] = OperInfo::new(bp::SHIFT.0, bp::SHIFT.1, 11, 1);
    table[TokenKind::TAG_PLUS as usize] = OperInfo::new(bp::ADDITIVE.0, bp::ADDITIVE.1, 12, 1);
    table[TokenKind::TAG_MINUS as usize] = OperInfo::new(bp::ADDITIVE.0, bp::ADDITIVE.1, 13, 1);
    table[TokenKind::TAG_STAR as usize] =
        OperInfo::new(bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, 14, 1);
    table[TokenKind::TAG_SLASH as usize] =
        OperInfo::new(bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, 15, 1);
    table[TokenKind::TAG_PERCENT as usize] =
        OperInfo::new(bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, 16, 1);
    table[TokenKind::TAG_DIV as usize] =
        OperInfo::new(bp::MULTIPLICATIVE.0, bp::MULTIPLICATIVE.1, 17, 1);

    table
};

impl Parser<'_> {
    /// Get the infix binding power for the current token.
    ///
    /// Uses a static lookup table indexed by the token's discriminant tag
    /// for O(1) dispatch — a single memory read instead of a 20-arm match.
    ///
    /// Returns `(left_bp, right_bp, op, token_count)` or `None` if the
    /// current token is not a binary operator.
    #[inline]
    pub(crate) fn infix_binding_power(&self) -> Option<(u8, u8, BinaryOp, usize)> {
        let tag = self.current_tag();

        // Fast path: tags >= 128 are never operators (we only have 116 token kinds)
        if tag >= 128 {
            return None;
        }

        let info = OPER_TABLE[tag as usize];
        if info.left_bp == 0 {
            return None;
        }

        // Special case: Gt may be compound >= or >>
        if tag == TokenKind::TAG_GT {
            if self.is_greater_equal() {
                return Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::GtEq, 2));
            }
            if self.is_shift_right() {
                return Some((bp::SHIFT.0, bp::SHIFT.1, BinaryOp::Shr, 2));
            }
        }

        Some((
            info.left_bp,
            info.right_bp,
            op_from_u8(info.op),
            info.token_count as usize,
        ))
    }

    #[inline]
    pub(crate) fn match_unary_op(&self) -> Option<UnaryOp> {
        match self.current_tag() {
            TokenKind::TAG_MINUS => Some(UnaryOp::Neg),
            TokenKind::TAG_BANG => Some(UnaryOp::Not),
            TokenKind::TAG_TILDE => Some(UnaryOp::BitNot),
            _ => None,
        }
    }

    /// Match `function_exp` keywords.
    pub(crate) fn match_function_exp_kind(&self) -> Option<FunctionExpKind> {
        let tag = self.current_tag();

        // `with` has special capability provision syntax: with Ident = ...
        if tag == TokenKind::TAG_WITH {
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

        match tag {
            TokenKind::TAG_RECURSE => Some(FunctionExpKind::Recurse),
            TokenKind::TAG_PARALLEL => Some(FunctionExpKind::Parallel),
            TokenKind::TAG_SPAWN => Some(FunctionExpKind::Spawn),
            TokenKind::TAG_TIMEOUT => Some(FunctionExpKind::Timeout),
            TokenKind::TAG_CACHE => Some(FunctionExpKind::Cache),
            TokenKind::TAG_WITH => Some(FunctionExpKind::With),
            TokenKind::TAG_PRINT => Some(FunctionExpKind::Print),
            TokenKind::TAG_PANIC => Some(FunctionExpKind::Panic),
            TokenKind::TAG_CATCH => Some(FunctionExpKind::Catch),
            TokenKind::TAG_TODO => Some(FunctionExpKind::Todo),
            TokenKind::TAG_UNREACHABLE => Some(FunctionExpKind::Unreachable),
            _ => None,
        }
    }
}
