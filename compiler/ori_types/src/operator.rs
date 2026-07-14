//! Shared bridge from language operators to registry strategy fields.

use ori_ir::{BinaryOp, UnaryOp};
use ori_registry::{find_type, OpStrategy, TypeTag};

/// Read the registry strategy field corresponding to a binary operator.
///
/// `None` means either the type has no builtin registry entry or the operator
/// is lowered by a dedicated path rather than an [`ori_registry::OpDefs`]
/// field.
#[must_use]
pub fn registry_binary_strategy(type_tag: TypeTag, operation: BinaryOp) -> Option<OpStrategy> {
    let operators = &find_type(type_tag)?.operators;
    Some(match operation {
        BinaryOp::Add => operators.add,
        BinaryOp::Sub => operators.sub,
        BinaryOp::Mul => operators.mul,
        BinaryOp::Div => operators.div,
        BinaryOp::Mod => operators.rem,
        BinaryOp::FloorDiv => operators.floor_div,
        BinaryOp::Eq => operators.eq,
        BinaryOp::NotEq => operators.neq,
        BinaryOp::Lt => operators.lt,
        BinaryOp::LtEq => operators.lt_eq,
        BinaryOp::Gt => operators.gt,
        BinaryOp::GtEq => operators.gt_eq,
        BinaryOp::BitAnd => operators.bit_and,
        BinaryOp::BitOr => operators.bit_or,
        BinaryOp::BitXor => operators.bit_xor,
        BinaryOp::Shl => operators.shl,
        BinaryOp::Shr => operators.shr,
        BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::MatMul
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::Coalesce => return None,
    })
}

/// Read the registry strategy field corresponding to a unary operator.
///
/// `None` means either the type has no builtin registry entry or the operator
/// is lowered by a dedicated path rather than an [`ori_registry::OpDefs`]
/// field.
#[must_use]
pub fn registry_unary_strategy(type_tag: TypeTag, operation: UnaryOp) -> Option<OpStrategy> {
    let operators = &find_type(type_tag)?.operators;
    Some(match operation {
        UnaryOp::Neg => operators.neg,
        UnaryOp::Not => operators.not,
        UnaryOp::BitNot => operators.bit_not,
        UnaryOp::Try => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_binary_registry_fields() {
        assert_eq!(
            registry_binary_strategy(TypeTag::Int, BinaryOp::Add),
            Some(OpStrategy::IntInstr)
        );
        assert!(matches!(
            registry_binary_strategy(TypeTag::Str, BinaryOp::Add),
            Some(OpStrategy::RuntimeCall {
                fn_name: "ori_str_concat",
                returns_bool: false
            })
        ));
        assert_eq!(registry_binary_strategy(TypeTag::Bool, BinaryOp::And), None);
    }

    #[test]
    fn reads_unary_registry_fields() {
        assert_eq!(
            registry_unary_strategy(TypeTag::Bool, UnaryOp::Not),
            Some(OpStrategy::BoolLogic)
        );
        assert_eq!(registry_unary_strategy(TypeTag::Int, UnaryOp::Try), None);
    }
}
