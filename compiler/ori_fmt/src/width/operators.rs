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
mod tests {
    use super::*;

    #[test]
    fn test_binary_op_width_arithmetic() {
        assert_eq!(binary_op_width(BinaryOp::Add), 3); // " + "
        assert_eq!(binary_op_width(BinaryOp::Sub), 3); // " - "
        assert_eq!(binary_op_width(BinaryOp::Mul), 3); // " * "
        assert_eq!(binary_op_width(BinaryOp::Div), 3); // " / "
        assert_eq!(binary_op_width(BinaryOp::Mod), 3); // " % "
        assert_eq!(binary_op_width(BinaryOp::FloorDiv), 5); // " div "
    }

    #[test]
    fn test_binary_op_width_comparison() {
        assert_eq!(binary_op_width(BinaryOp::Eq), 4); // " == "
        assert_eq!(binary_op_width(BinaryOp::NotEq), 4); // " != "
        assert_eq!(binary_op_width(BinaryOp::Lt), 3); // " < "
        assert_eq!(binary_op_width(BinaryOp::Gt), 3); // " > "
        assert_eq!(binary_op_width(BinaryOp::LtEq), 4); // " <= "
        assert_eq!(binary_op_width(BinaryOp::GtEq), 4); // " >= "
    }

    #[test]
    fn test_binary_op_width_logical() {
        assert_eq!(binary_op_width(BinaryOp::And), 4); // " && "
        assert_eq!(binary_op_width(BinaryOp::Or), 4); // " || "
    }

    #[test]
    fn test_binary_op_width_bitwise() {
        assert_eq!(binary_op_width(BinaryOp::BitAnd), 3); // " & "
        assert_eq!(binary_op_width(BinaryOp::BitOr), 3); // " | "
        assert_eq!(binary_op_width(BinaryOp::BitXor), 3); // " ^ "
        assert_eq!(binary_op_width(BinaryOp::Shl), 4); // " << "
        assert_eq!(binary_op_width(BinaryOp::Shr), 4); // " >> "
    }

    #[test]
    fn test_binary_op_width_range() {
        assert_eq!(binary_op_width(BinaryOp::Range), 4); // " .. "
        assert_eq!(binary_op_width(BinaryOp::RangeInclusive), 5); // " ..= "
    }

    #[test]
    fn test_binary_op_width_coalesce() {
        assert_eq!(binary_op_width(BinaryOp::Coalesce), 4); // " ?? "
    }

    #[test]
    fn test_unary_op_width() {
        assert_eq!(unary_op_width(UnaryOp::Neg), 1); // "-"
        assert_eq!(unary_op_width(UnaryOp::Not), 1); // "!"
        assert_eq!(unary_op_width(UnaryOp::BitNot), 1); // "~"
        assert_eq!(unary_op_width(UnaryOp::Try), 1); // "?"
    }
}
