//! Width calculation for operators.
//!
//! Provides width calculations for:
//! - Binary operators (including surrounding spaces)
//! - Unary operators (prefix only)

use ori_ir::{BinaryOp, UnaryOp};

/// Calculate width of a binary operator (including surrounding spaces).
///
/// Returns the total width including the mandatory spaces around the operator.
/// For example, `+` returns 3 for " + ".
#[expect(
    clippy::match_same_arms,
    reason = "Each arm explicitly documents the operator width for maintainability"
)]
pub(super) fn binary_op_width(op: BinaryOp) -> usize {
    let op_w = match op {
        // Single-character arithmetic: + - * / %
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 1,
        // Floor division: div
        BinaryOp::FloorDiv => 3,
        // Two-character comparison: == !=
        BinaryOp::Eq | BinaryOp::NotEq => 2,
        // Single-character comparison: < >
        BinaryOp::Lt | BinaryOp::Gt => 1,
        // Two-character comparison: <= >=
        BinaryOp::LtEq | BinaryOp::GtEq => 2,
        // Logical: && ||
        BinaryOp::And | BinaryOp::Or => 2,
        // Single-character bitwise: & | ^
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => 1,
        // Shifts: << >>
        BinaryOp::Shl | BinaryOp::Shr => 2,
        // Range: ..
        BinaryOp::Range => 2,
        // Inclusive range: ..=
        BinaryOp::RangeInclusive => 3,
        // Coalesce: ??
        BinaryOp::Coalesce => 2,
        // MatMul: @
        BinaryOp::MatMul => 1,
    };
    // " op " - space on each side
    1 + op_w + 1
}

/// Calculate width of a unary operator.
///
/// Unary operators have no surrounding spaces - they attach directly to their operand.
#[expect(
    clippy::match_same_arms,
    reason = "Each arm explicitly documents the operator for maintainability"
)]
pub(super) fn unary_op_width(op: UnaryOp) -> usize {
    match op {
        UnaryOp::Neg => 1,    // "-"
        UnaryOp::Not => 1,    // "!"
        UnaryOp::BitNot => 1, // "~"
        UnaryOp::Try => 1,    // "?"
    }
}

#[cfg(test)]
mod tests;
