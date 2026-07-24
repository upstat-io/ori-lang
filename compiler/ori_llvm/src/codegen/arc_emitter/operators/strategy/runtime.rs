//! Runtime-backed binary operation selection.

use ori_ir::BinaryOp;
use ori_registry::RuntimeOperator;

/// Resolve the native symbol attached to a runtime operator.
pub(super) const fn native_runtime_symbol(runtime: RuntimeOperator) -> Option<&'static str> {
    match runtime {
        RuntimeOperator::StringConcat => Some("ori_str_concat"),
        RuntimeOperator::StringEqual => Some("ori_str_eq"),
        RuntimeOperator::StringNotEqual => Some("ori_str_ne"),
        RuntimeOperator::StringCompare => Some("ori_str_compare"),
        RuntimeOperator::ListConcat => None,
    }
}

/// Classifies string runtime operations by their result projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::codegen::arc_emitter) enum RuntimeBinaryOperation {
    /// Concatenate two strings.
    Concat,
    /// Test two strings for equality.
    Equal,
    /// Test two strings for inequality.
    NotEqual,
    /// Project a less-than result from string comparison.
    Less,
    /// Project a greater-than result from string comparison.
    Greater,
    /// Project a less-than-or-equal result from string comparison.
    LessOrEqual,
    /// Project a greater-than-or-equal result from string comparison.
    GreaterOrEqual,
}

impl RuntimeBinaryOperation {
    /// Validate and classify one canonical registry pair.
    pub(in crate::codegen::arc_emitter) fn from_parts(
        runtime: RuntimeOperator,
        operation: BinaryOp,
    ) -> Self {
        match runtime {
            RuntimeOperator::StringConcat => {
                require_pair(runtime, operation, BinaryOp::Add);
                Self::Concat
            }
            RuntimeOperator::StringEqual => {
                require_pair(runtime, operation, BinaryOp::Eq);
                Self::Equal
            }
            RuntimeOperator::StringNotEqual => {
                require_pair(runtime, operation, BinaryOp::NotEq);
                Self::NotEqual
            }
            RuntimeOperator::StringCompare => match operation {
                BinaryOp::Lt => Self::Less,
                BinaryOp::Gt => Self::Greater,
                BinaryOp::LtEq => Self::LessOrEqual,
                BinaryOp::GtEq => Self::GreaterOrEqual,
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Mod
                | BinaryOp::FloorDiv
                | BinaryOp::MatMul
                | BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr
                | BinaryOp::Range
                | BinaryOp::RangeInclusive
                | BinaryOp::Coalesce => invalid_pair(runtime, operation),
            },
            RuntimeOperator::ListConcat => invalid_pair(runtime, operation),
        }
    }

    /// Return the registry operator that owns this emission strategy.
    pub(super) const fn runtime(self) -> RuntimeOperator {
        match self {
            Self::Concat => RuntimeOperator::StringConcat,
            Self::Equal => RuntimeOperator::StringEqual,
            Self::NotEqual => RuntimeOperator::StringNotEqual,
            Self::Less | Self::Greater | Self::LessOrEqual | Self::GreaterOrEqual => {
                RuntimeOperator::StringCompare
            }
        }
    }
}

fn require_pair(runtime: RuntimeOperator, operation: BinaryOp, expected: BinaryOp) {
    if operation != expected {
        invalid_pair(runtime, operation);
    }
}

fn invalid_pair(runtime: RuntimeOperator, operation: BinaryOp) -> ! {
    // Why: the registry admits only the canonical pairs classified above.
    unreachable!(
        "operator registry maps {operation:?} to incompatible runtime operation {runtime:?}"
    )
}
