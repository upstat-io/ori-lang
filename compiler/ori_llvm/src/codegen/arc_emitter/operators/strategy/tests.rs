use ori_ir::BinaryOp;
use ori_registry::RuntimeOperator;

use super::RuntimeBinaryOperation;

#[test]
fn canonical_runtime_binary_pairs_map_to_every_emission_strategy() {
    let cases = [
        (
            RuntimeOperator::StringConcat,
            BinaryOp::Add,
            RuntimeBinaryOperation::Concat,
        ),
        (
            RuntimeOperator::StringEqual,
            BinaryOp::Eq,
            RuntimeBinaryOperation::Equal,
        ),
        (
            RuntimeOperator::StringNotEqual,
            BinaryOp::NotEq,
            RuntimeBinaryOperation::NotEqual,
        ),
        (
            RuntimeOperator::StringCompare,
            BinaryOp::Lt,
            RuntimeBinaryOperation::Less,
        ),
        (
            RuntimeOperator::StringCompare,
            BinaryOp::Gt,
            RuntimeBinaryOperation::Greater,
        ),
        (
            RuntimeOperator::StringCompare,
            BinaryOp::LtEq,
            RuntimeBinaryOperation::LessOrEqual,
        ),
        (
            RuntimeOperator::StringCompare,
            BinaryOp::GtEq,
            RuntimeBinaryOperation::GreaterOrEqual,
        ),
    ];

    for (runtime, operation, expected) in cases {
        assert_eq!(
            RuntimeBinaryOperation::from_parts(runtime, operation),
            expected
        );
    }
}
