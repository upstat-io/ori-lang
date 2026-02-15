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
