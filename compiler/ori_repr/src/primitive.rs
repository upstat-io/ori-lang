//! Backend-neutral builtin primitive-operation classification.

use ori_ir::{BinaryOp, UnaryOp};
use ori_registry::{OpStrategy, TypeTag};
use ori_types::{registry_binary_strategy, registry_unary_strategy};

pub use ori_registry::{OpStrategy as PrimitiveStrategy, TypeTag as BuiltinType};

/// Select the canonical lowering strategy for a builtin binary operation.
///
/// Executable backends translate this semantic classification into their own
/// instruction formats. They must not independently re-derive it.
#[must_use]
pub fn binary_primitive_strategy(type_tag: TypeTag, operation: BinaryOp) -> OpStrategy {
    match operation {
        BinaryOp::And | BinaryOp::Or => return OpStrategy::IntInstr,
        BinaryOp::MatMul | BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Coalesce => {
            return OpStrategy::Unsupported
        }
        _ => {}
    }

    registry_binary_strategy(type_tag, operation).unwrap_or(OpStrategy::Unsupported)
}

/// Select the canonical lowering strategy for a builtin unary operation.
///
/// Executable backends translate this semantic classification into their own
/// instruction formats. They must not independently re-derive it.
#[must_use]
pub fn unary_primitive_strategy(type_tag: TypeTag, operation: UnaryOp) -> OpStrategy {
    if matches!(operation, UnaryOp::Try) {
        return OpStrategy::Unsupported;
    }

    registry_unary_strategy(type_tag, operation).unwrap_or(OpStrategy::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_integer_float_bool_and_runtime_operations() {
        assert_eq!(
            binary_primitive_strategy(TypeTag::Int, BinaryOp::Add),
            OpStrategy::IntInstr
        );
        assert_eq!(
            binary_primitive_strategy(TypeTag::Float, BinaryOp::Lt),
            OpStrategy::FloatInstr
        );
        assert_eq!(
            unary_primitive_strategy(TypeTag::Bool, UnaryOp::Not),
            OpStrategy::BoolLogic
        );
        assert!(matches!(
            binary_primitive_strategy(TypeTag::Str, BinaryOp::Add),
            OpStrategy::RuntimeCall {
                fn_name: "ori_str_concat",
                returns_bool: false
            }
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
                OpStrategy::IntInstr
            );
        }
    }
}
