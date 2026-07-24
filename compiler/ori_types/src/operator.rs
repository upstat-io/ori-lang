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

/// Select the canonical executable strategy for a builtin binary primitive.
/// Lowered control-flow operators remain unsupported; eager logical operators
/// use the existing integer instruction family.
#[must_use]
pub fn primitive_binary_strategy(type_tag: TypeTag, operation: BinaryOp) -> OpStrategy {
    let structural = match operation {
        BinaryOp::Eq | BinaryOp::NotEq
            if matches!(
                type_tag,
                TypeTag::List
                    | TypeTag::Map
                    | TypeTag::Set
                    | TypeTag::Tuple
                    | TypeTag::Option
                    | TypeTag::Result
            ) =>
        {
            Some(OpStrategy::StructuralEquality)
        }
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq
            if matches!(
                type_tag,
                TypeTag::List | TypeTag::Tuple | TypeTag::Option | TypeTag::Result
            ) =>
        {
            Some(OpStrategy::StructuralOrdering)
        }
        _ => None,
    };
    if let Some(strategy) = structural {
        return strategy;
    }
    match operation {
        BinaryOp::And | BinaryOp::Or => return OpStrategy::SignedInteger,
        BinaryOp::MatMul | BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Coalesce => {
            return OpStrategy::Unsupported;
        }
        _ => {}
    }
    registry_binary_strategy(type_tag, operation).unwrap_or(OpStrategy::Unsupported)
}

/// Select the canonical structural strategy for a non-builtin value.
///
/// Type checking has already proved the corresponding `Eq` or `Comparable`
/// capability. Arithmetic and other overloadable operators are rewritten to
/// exact callable bodies before this seam and therefore have no structural
/// fallback here.
#[must_use]
pub const fn user_structural_binary_strategy(operation: BinaryOp) -> Option<OpStrategy> {
    match operation {
        BinaryOp::Eq | BinaryOp::NotEq => Some(OpStrategy::StructuralEquality),
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq => {
            Some(OpStrategy::StructuralOrdering)
        }
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::FloorDiv
        | BinaryOp::MatMul
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::Shl
        | BinaryOp::Shr
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::Coalesce => None,
    }
}

/// Select the canonical executable strategy for a builtin unary primitive.
#[must_use]
pub fn primitive_unary_strategy(type_tag: TypeTag, operation: UnaryOp) -> OpStrategy {
    if matches!(operation, UnaryOp::Try) {
        return OpStrategy::Unsupported;
    }
    registry_unary_strategy(type_tag, operation).unwrap_or(OpStrategy::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_binary_registry_fields() {
        assert_eq!(
            registry_binary_strategy(TypeTag::Int, BinaryOp::Add),
            Some(OpStrategy::SignedInteger)
        );
        assert!(matches!(
            registry_binary_strategy(TypeTag::Str, BinaryOp::Add),
            Some(OpStrategy::RuntimeCall(
                ori_registry::RuntimeOperator::StringConcat
            ))
        ));
        assert_eq!(registry_binary_strategy(TypeTag::Bool, BinaryOp::And), None);
    }

    #[test]
    fn reads_unary_registry_fields() {
        assert_eq!(
            registry_unary_strategy(TypeTag::Bool, UnaryOp::Not),
            Some(OpStrategy::BooleanLogic)
        );
        assert_eq!(registry_unary_strategy(TypeTag::Int, UnaryOp::Try), None);
    }

    #[test]
    fn compound_comparisons_have_shared_structural_identities() {
        assert_eq!(
            primitive_binary_strategy(TypeTag::Unit, BinaryOp::Eq),
            OpStrategy::StructuralEquality
        );
        assert_eq!(
            primitive_binary_strategy(TypeTag::List, BinaryOp::Eq),
            OpStrategy::StructuralEquality
        );
        assert_eq!(
            primitive_binary_strategy(TypeTag::Set, BinaryOp::Eq),
            OpStrategy::StructuralEquality
        );
        assert_eq!(
            primitive_binary_strategy(TypeTag::Unit, BinaryOp::Lt),
            OpStrategy::StructuralOrdering
        );
        assert_eq!(
            primitive_binary_strategy(TypeTag::Option, BinaryOp::Lt),
            OpStrategy::StructuralOrdering
        );
        assert_eq!(
            primitive_binary_strategy(TypeTag::Map, BinaryOp::Lt),
            OpStrategy::Unsupported
        );
        assert_eq!(
            user_structural_binary_strategy(BinaryOp::Eq),
            Some(OpStrategy::StructuralEquality)
        );
        assert_eq!(
            user_structural_binary_strategy(BinaryOp::GtEq),
            Some(OpStrategy::StructuralOrdering)
        );
        assert_eq!(user_structural_binary_strategy(BinaryOp::Add), None);
    }
}
