//! LLVM instruction-family emitters for shared primitive strategies.

use ori_ir::BinaryOp;
use ori_registry::RuntimeOperator;
use ori_types::Idx;

use super::super::builtins;
use super::super::{ArcIrEmitter, StringRuntimeReturnAbi};
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

#[derive(Clone, Copy)]
enum CheckedIntSemantics {
    Int,
    Duration,
    Size,
}

impl CheckedIntSemantics {
    const fn overflow_message(self, operation: BinaryOp) -> &'static str {
        match (self, operation) {
            (Self::Int, BinaryOp::Add) => "integer overflow in addition",
            (Self::Int, BinaryOp::Sub) => "integer overflow in subtraction",
            (Self::Int, BinaryOp::Mul) => "integer overflow in multiplication",
            (Self::Int, BinaryOp::Div) => "integer overflow in division",
            (Self::Int, BinaryOp::FloorDiv) => "integer overflow in floor division",
            (Self::Duration, BinaryOp::Add) => "integer overflow in duration addition",
            (Self::Duration, BinaryOp::Sub) => "integer overflow in duration subtraction",
            (Self::Duration, BinaryOp::Mul) => "integer overflow in duration multiplication",
            (Self::Duration, BinaryOp::Div | BinaryOp::FloorDiv) => {
                "integer overflow in duration division"
            }
            (Self::Size, BinaryOp::Add) => "integer overflow in size addition",
            (Self::Size, BinaryOp::Sub) => "integer overflow in size subtraction",
            (Self::Size, BinaryOp::Mul) => "integer overflow in size multiplication",
            (Self::Size, BinaryOp::Div | BinaryOp::FloorDiv) => "integer overflow in size division",
            _ => "integer overflow",
        }
    }
}

const fn native_runtime_symbol(runtime: RuntimeOperator) -> Option<&'static str> {
    match runtime {
        RuntimeOperator::StringConcat => Some("ori_str_concat"),
        RuntimeOperator::StringEqual => Some("ori_str_eq"),
        RuntimeOperator::StringNotEqual => Some("ori_str_ne"),
        RuntimeOperator::StringCompare => Some("ori_str_compare"),
        RuntimeOperator::ListConcat => None,
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    fn emit_checked_int_sub(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        semantics: CheckedIntSemantics,
        op: BinaryOp,
    ) -> ValueId {
        if matches!(semantics, CheckedIntSemantics::Size) {
            self.builder.panic_if_signed_less_than(
                lhs,
                rhs,
                "size_sub.negative",
                "size subtraction would result in negative value",
            );
        }
        self.builder
            .checked_sub_msg(lhs, rhs, "sub", semantics.overflow_message(op))
    }

    fn emit_checked_int_mul(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        lhs_is_size: bool,
        rhs_is_size: bool,
        semantics: CheckedIntSemantics,
        op: BinaryOp,
    ) -> ValueId {
        if matches!(semantics, CheckedIntSemantics::Size) {
            let scalar = if lhs_is_size && !rhs_is_size {
                Some(rhs)
            } else if rhs_is_size && !lhs_is_size {
                Some(lhs)
            } else {
                None
            };
            if let Some(scalar) = scalar {
                self.builder.panic_if_negative(
                    scalar,
                    "size_mul.negative",
                    "cannot multiply Size by negative integer",
                );
            }
        }
        self.builder
            .checked_mul_msg(lhs, rhs, "mul", semantics.overflow_message(op))
    }

    fn emit_checked_int_div(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        lhs_is_size: bool,
        rhs_is_size: bool,
        semantics: CheckedIntSemantics,
        op: BinaryOp,
    ) -> ValueId {
        if lhs_is_size && !rhs_is_size {
            self.builder.panic_if_negative(
                rhs,
                "size_div.negative",
                "cannot divide Size by negative integer",
            );
        }
        self.builder.checked_div_msg(
            lhs,
            rhs,
            "div",
            "division by zero",
            semantics.overflow_message(op),
        )
    }

    /// Emit a binary op using signed integer LLVM instructions.
    ///
    /// Handles arithmetic (`checked_add`, `checked_sub`, etc.), signed comparison
    /// (`icmp slt`), bitwise ops, and logical `And`/`Or` (from compiler-generated
    /// `PrimOps` like range step conditions). `Coalesce` is lowered to control flow
    /// by `ori_arc` and never reaches this function.
    pub(in crate::codegen::arc_emitter) fn emit_int_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
        rhs_ty: Idx,
        result_ty: Idx,
    ) -> ValueId {
        let lhs_is_size = matches!(self.type_info.get(lhs_ty), TypeInfo::Size);
        let rhs_is_size = matches!(self.type_info.get(rhs_ty), TypeInfo::Size);
        let result_is_size = matches!(self.type_info.get(result_ty), TypeInfo::Size);
        let has_duration = matches!(self.type_info.get(lhs_ty), TypeInfo::Duration)
            || matches!(self.type_info.get(rhs_ty), TypeInfo::Duration)
            || matches!(self.type_info.get(result_ty), TypeInfo::Duration);
        let semantics = if lhs_is_size || rhs_is_size || result_is_size {
            CheckedIntSemantics::Size
        } else if has_duration {
            CheckedIntSemantics::Duration
        } else {
            CheckedIntSemantics::Int
        };

        match op {
            BinaryOp::Add => self.builder.checked_add_msg(
                lhs,
                rhs,
                "add",
                semantics.overflow_message(op),
            ),
            BinaryOp::Sub => self.emit_checked_int_sub(lhs, rhs, semantics, op),
            BinaryOp::Mul => {
                self.emit_checked_int_mul(lhs, rhs, lhs_is_size, rhs_is_size, semantics, op)
            }
            BinaryOp::Div => {
                self.emit_checked_int_div(lhs, rhs, lhs_is_size, rhs_is_size, semantics, op)
            }
            BinaryOp::Mod => self.builder.checked_rem_msg(lhs, rhs, "rem", "modulo by zero"),
            BinaryOp::FloorDiv => self.builder.checked_div_msg(
                lhs,
                rhs,
                "floordiv",
                "division by zero",
                semantics.overflow_message(op),
            ),
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
            BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
            BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
            BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            // Coalesce is always lowered to control flow by ori_arc.
            BinaryOp::Coalesce => unreachable!(
                "Coalesce is lowered to control flow by ori_arc and should never reach emit_int_binary_op"
            ),
            BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
            BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
            BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
            BinaryOp::Shl => self.builder.checked_shl(lhs, rhs, "shl"),
            BinaryOp::Shr => self.builder.checked_shr(lhs, rhs, "shr"),
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
                unreachable!("desugared op {op:?} should not reach emit_int_binary_op")
            }
        }
    }

    /// Emit a binary op using floating-point LLVM instructions.
    pub(in crate::codegen::arc_emitter) fn emit_float_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        match op {
            BinaryOp::Add => self.builder.fadd(lhs, rhs, "add"),
            BinaryOp::Sub => self.builder.fsub(lhs, rhs, "sub"),
            BinaryOp::Mul => self.builder.fmul(lhs, rhs, "mul"),
            BinaryOp::Div => self.builder.fdiv(lhs, rhs, "div"),
            BinaryOp::Mod => self.builder.frem(lhs, rhs, "rem"),
            BinaryOp::Eq => self.builder.fcmp_oeq(lhs, rhs, "eq"),
            BinaryOp::NotEq => self.builder.fcmp_une(lhs, rhs, "ne"),
            BinaryOp::Lt => self.builder.fcmp_olt(lhs, rhs, "lt"),
            BinaryOp::Gt => self.builder.fcmp_ogt(lhs, rhs, "gt"),
            BinaryOp::LtEq => self.builder.fcmp_ole(lhs, rhs, "le"),
            BinaryOp::GtEq => self.builder.fcmp_oge(lhs, rhs, "ge"),
            // Registry assigns FloatingPoint only to the arms above for float;
            // every other op is Unsupported or never reaches strategy lookup.
            BinaryOp::FloorDiv
            | BinaryOp::MatMul
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
            | BinaryOp::Range
            | BinaryOp::RangeInclusive
            | BinaryOp::Coalesce => unreachable!("unsupported float binary op {op:?}"),
        }
    }

    /// Emit a binary op using unsigned integer comparison instructions.
    ///
    /// Used for `byte`/`char` where comparison semantics are unsigned, and
    /// for `bool` ordering (`false < true` — unsigned since false=0, true=1).
    pub(in crate::codegen::arc_emitter) fn emit_unsigned_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        match op {
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::Lt => self.builder.icmp_ult(lhs, rhs, "lt"),
            BinaryOp::Gt => self.builder.icmp_ugt(lhs, rhs, "gt"),
            BinaryOp::LtEq => self.builder.icmp_ule(lhs, rhs, "le"),
            BinaryOp::GtEq => self.builder.icmp_uge(lhs, rhs, "ge"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            // UnsignedComparison covers byte/char/bool comparison plus And/Or only;
            // arithmetic/bitwise on those types uses SignedInteger or Unsupported.
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
            | BinaryOp::Range
            | BinaryOp::RangeInclusive
            | BinaryOp::Coalesce => unreachable!("unsupported unsigned binary op {op:?}"),
        }
    }

    /// Emit a binary op using boolean logic instructions.
    ///
    /// Handles `bool` equality and logical operators. Ordering on `bool`
    /// uses [`OpStrategy::UnsignedComparison`] instead.
    pub(in crate::codegen::arc_emitter) fn emit_bool_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        match op {
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            // BooleanLogic covers Eq/NotEq/And/Or only; bool ordering routes
            // through UnsignedComparison and everything else is Unsupported.
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::FloorDiv
            | BinaryOp::MatMul
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
            | BinaryOp::Range
            | BinaryOp::RangeInclusive
            | BinaryOp::Coalesce => unreachable!("unsupported bool binary op {op:?}"),
        }
    }

    /// Emit a binary op via runtime function call.
    ///
    /// Handles string operations (the sole [`OpStrategy::RuntimeCall`]
    /// type in the registry). Comparison ops use `ori_str_compare` which returns
    /// `Ordering` (i8) and is post-processed into a bool predicate.
    pub(in crate::codegen::arc_emitter) fn emit_runtime_binary_op(
        &mut self,
        runtime: RuntimeOperator,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        match (runtime, op) {
            (RuntimeOperator::StringConcat, BinaryOp::Add) => self.emit_str_runtime_call(
                native_runtime_symbol(runtime)
                    .expect("string runtime identity has a native ABI symbol"),
                lhs,
                rhs,
                StringRuntimeReturnAbi::StringSret,
            ),
            (RuntimeOperator::StringEqual, BinaryOp::Eq)
            | (RuntimeOperator::StringNotEqual, BinaryOp::NotEq) => self.emit_str_runtime_call(
                native_runtime_symbol(runtime)
                    .expect("string runtime identity has a native ABI symbol"),
                lhs,
                rhs,
                StringRuntimeReturnAbi::BoolDirect,
            ),
            (RuntimeOperator::StringCompare, BinaryOp::Lt) => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
                .expect("str Lt comparison should always succeed"),
            (RuntimeOperator::StringCompare, BinaryOp::Gt) => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Greater)
                .expect("str Gt comparison should always succeed"),
            (RuntimeOperator::StringCompare, BinaryOp::LtEq) => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::LessOrEqual)
                .expect("str LtEq comparison should always succeed"),
            (RuntimeOperator::StringCompare, BinaryOp::GtEq) => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::GreaterOrEqual)
                .expect("str GtEq comparison should always succeed"),
            (RuntimeOperator::ListConcat, BinaryOp::Add) => {
                unreachable!("list concat is emitted by the ownership-aware operator projection")
            }
            _ => unreachable!("runtime operation {runtime:?} does not implement {op:?}"),
        }
    }
}
