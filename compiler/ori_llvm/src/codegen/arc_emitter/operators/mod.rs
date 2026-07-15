//! Operator emission for [`ArcIrEmitter`].
//!
//! Emits LLVM IR for binary and unary operations. Primitive/builtin types
//! use [`OpStrategy`] dispatch from the registry; non-primitive types dispatch
//! to operator trait methods (e.g., `+` → `Add.add()`, `==` → `Eq.eq()`).
//!
//! Strategy dispatch helpers live in [`strategy`].

mod strategy;

use ori_ir::{BinaryOp, UnaryOp};
use ori_registry::{OpStrategy, RuntimeOperator};
use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a binary operation.
    ///
    /// For non-primitive types, dispatches to trait methods. For primitive/builtin
    /// types, uses [`OpStrategy`] from the registry to select the correct LLVM
    /// instruction family — eliminating ad-hoc type guards.
    pub(super) fn emit_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
        strategy: OpStrategy,
        arc_func: &ori_arc::ir::ArcFunction,
    ) -> ValueId {
        match strategy {
            OpStrategy::SignedInteger => self.emit_int_binary_op(op, lhs, rhs),
            OpStrategy::FloatingPoint => self.emit_float_binary_op(op, lhs, rhs),
            OpStrategy::UnsignedComparison => self.emit_unsigned_binary_op(op, lhs, rhs),
            OpStrategy::BooleanLogic => self.emit_bool_binary_op(op, lhs, rhs),
            OpStrategy::StructuralEquality => {
                let equals = self
                    .emit_element_equals(lhs, rhs, lhs_ty)
                    .expect("validated structural equality has an LLVM projection");
                if op == BinaryOp::NotEq {
                    self.builder.not(equals, "neq")
                } else {
                    equals
                }
            }
            OpStrategy::StructuralOrdering => self
                .emit_ordering_comparison(op, lhs, rhs, lhs_ty)
                .expect("validated structural ordering has an LLVM projection"),
            OpStrategy::RuntimeCall(RuntimeOperator::ListConcat) => {
                let TypeInfo::List { element } = self.type_info.get(lhs_ty) else {
                    unreachable!("typed ListConcat strategy requires a List receiver")
                };
                let cm = self.cow_mode_const(arc_func);
                self.emit_list_concat_cow(lhs, rhs, element, cm, lhs_ty)
                    .expect("list concat runtime returns one list value")
            }
            OpStrategy::RuntimeCall(runtime) => self.emit_runtime_binary_op(runtime, op, lhs, rhs),
            OpStrategy::Unsupported => {
                unreachable!("validated primitive has Unsupported strategy")
            }
        }
    }

    /// Emit a unary operation.
    ///
    /// For non-primitive types, dispatches to the corresponding operator trait
    /// method (e.g., `-` → `Negate.negate()`). For primitive/builtin types,
    /// uses [`OpStrategy`] from the registry to select the correct LLVM
    /// instruction family.
    pub(super) fn emit_unary_op(
        &mut self,
        op: UnaryOp,
        operand: ValueId,
        _operand_ty: Idx,
        strategy: OpStrategy,
    ) -> ValueId {
        match strategy {
            OpStrategy::SignedInteger
            | OpStrategy::BooleanLogic
            | OpStrategy::UnsignedComparison => match op {
                UnaryOp::Neg => self.builder.checked_neg(operand, "neg"),
                UnaryOp::Not => self.builder.not(operand, "not"),
                UnaryOp::BitNot => {
                    let all_ones = self.builder.const_i64(-1);
                    self.builder.xor(operand, all_ones, "bitnot")
                }
                UnaryOp::Try => unreachable!("try desugared before ARC IR"),
            },
            OpStrategy::FloatingPoint => match op {
                UnaryOp::Neg => self.builder.fneg(operand, "neg"),
                // Registry assigns FloatingPoint only to Neg on float; Try is
                // desugared before ARC IR.
                UnaryOp::Not | UnaryOp::BitNot | UnaryOp::Try => {
                    unreachable!("unsupported float unary op {op:?}")
                }
            },
            OpStrategy::StructuralEquality | OpStrategy::StructuralOrdering => {
                unreachable!("unary op {op:?} has a structural binary strategy")
            }
            OpStrategy::Unsupported => {
                // Try is desugared before reaching ARC IR. If it slips
                // through, warn and return a zero constant.
                unreachable!("validated primitive has Unsupported strategy")
            }
            OpStrategy::RuntimeCall(_) => {
                unreachable!("unary op {op:?} has RuntimeCall strategy")
            }
        }
    }

    /// Project structural ordering to a boolean comparison.
    fn emit_ordering_comparison(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        let ordering = self.emit_element_compare(lhs, rhs, lhs_ty)?;

        // Ordering is i8: 0=Less, 1=Equal, 2=Greater
        // Map comparison operators to equality/inequality checks on the ordering value.
        let less = self.builder.const_i8(0);
        let greater = self.builder.const_i8(2);
        let result = match op {
            BinaryOp::Lt => self.builder.icmp_eq(ordering, less, "lt"),
            BinaryOp::Gt => self.builder.icmp_eq(ordering, greater, "gt"),
            BinaryOp::LtEq => self.builder.icmp_ne(ordering, greater, "le"),
            BinaryOp::GtEq => self.builder.icmp_ne(ordering, less, "ge"),
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
            | BinaryOp::Coalesce => unreachable!("only Lt/Gt/LtEq/GtEq reach here"),
        };
        Some(result)
    }
}
