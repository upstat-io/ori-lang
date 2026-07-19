//! Backend-neutral builtin primitive-operation classification.

use ori_ir::{BinaryOp, UnaryOp};
use ori_registry::{OpStrategy, TypeTag};
use ori_types::{primitive_binary_strategy, primitive_unary_strategy};

pub use ori_registry::{OpStrategy as PrimitiveStrategy, TypeTag as BuiltinType};

/// Select the canonical lowering strategy for a builtin binary operation.
///
/// Executable backends translate this semantic classification into their own
/// instruction formats. They must not independently re-derive it.
#[must_use]
pub fn binary_primitive_strategy(type_tag: TypeTag, operation: BinaryOp) -> OpStrategy {
    primitive_binary_strategy(type_tag, operation)
}

/// Select the canonical lowering strategy for a builtin unary operation.
///
/// Executable backends translate this semantic classification into their own
/// instruction formats. They must not independently re-derive it.
#[must_use]
pub fn unary_primitive_strategy(type_tag: TypeTag, operation: UnaryOp) -> OpStrategy {
    primitive_unary_strategy(type_tag, operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_integer_float_bool_and_runtime_operations() {
        assert_eq!(
            binary_primitive_strategy(TypeTag::Int, BinaryOp::Add),
            OpStrategy::SignedInteger
        );
        assert_eq!(
            binary_primitive_strategy(TypeTag::Float, BinaryOp::Lt),
            OpStrategy::FloatingPoint
        );
        assert_eq!(
            unary_primitive_strategy(TypeTag::Bool, UnaryOp::Not),
            OpStrategy::BooleanLogic
        );
        assert!(matches!(
            binary_primitive_strategy(TypeTag::Str, BinaryOp::Add),
            OpStrategy::RuntimeCall(ori_registry::RuntimeOperator::StringConcat)
        ));
    }

    #[test]
    fn classifies_lowered_or_invalid_operations_as_unsupported() {
        assert_eq!(
            binary_primitive_strategy(TypeTag::Int, BinaryOp::Range),
            OpStrategy::Unsupported
        );
        assert_eq!(
            unary_primitive_strategy(TypeTag::Int, UnaryOp::Try),
            OpStrategy::Unsupported
        );
    }

    #[test]
    fn eager_logical_operations_preserve_existing_integer_strategy() {
        for operation in [BinaryOp::And, BinaryOp::Or] {
            assert_eq!(
                binary_primitive_strategy(TypeTag::Bool, operation),
                OpStrategy::SignedInteger
            );
        }
    }
}
