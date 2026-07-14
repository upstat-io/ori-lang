//! LLVM instruction-family emitters for shared primitive strategies.

use ori_ir::BinaryOp;
use ori_registry::TypeTag;
use ori_types::Idx;

use super::super::builtins;
use super::super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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
    ) -> ValueId {
        match op {
            BinaryOp::Add => self.builder.checked_add(lhs, rhs, "add"),
            BinaryOp::Sub => self.builder.checked_sub(lhs, rhs, "sub"),
            BinaryOp::Mul => self.builder.checked_mul(lhs, rhs, "mul"),
            BinaryOp::Div => self.builder.checked_div(lhs, rhs, "div"),
            BinaryOp::Mod => self.builder.checked_rem(lhs, rhs, "rem"),
            BinaryOp::FloorDiv => self.builder.checked_div(lhs, rhs, "floordiv"),
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
            // Registry assigns FloatInstr only to the arms above for float;
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
            // UnsignedCmp covers byte/char/bool comparison plus And/Or only;
            // arithmetic/bitwise on those types uses IntInstr or Unsupported.
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
    /// uses [`OpStrategy::UnsignedCmp`] instead.
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
            // BoolLogic covers Eq/NotEq/And/Or only; bool ordering routes
            // through UnsignedCmp and everything else is Unsupported.
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
    /// Currently handles string operations only (the sole [`OpStrategy::RuntimeCall`]
    /// type in the registry). Comparison ops use `ori_str_compare` which returns
    /// `Ordering` (i8) and is post-processed into a bool predicate.
    pub(in crate::codegen::arc_emitter) fn emit_runtime_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        match op {
            BinaryOp::Add => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
            BinaryOp::Eq => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
            BinaryOp::NotEq => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
            BinaryOp::Lt => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
                .expect("str Lt comparison should always succeed"),
            BinaryOp::Gt => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Greater)
                .expect("str Gt comparison should always succeed"),
            BinaryOp::LtEq => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::LessOrEqual)
                .expect("str LtEq comparison should always succeed"),
            BinaryOp::GtEq => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::GreaterOrEqual)
                .expect("str GtEq comparison should always succeed"),
            // RuntimeCall exists only for str concat/comparison in the
            // registry; no other op carries a RuntimeCall strategy.
            BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::FloorDiv
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
            | BinaryOp::Coalesce => unreachable!("unsupported runtime binary op {op:?}"),
        }
    }

    // Registry bridge

    /// Map a pool type to its shared builtin registry identity.
    pub(in crate::codegen::arc_emitter) fn idx_to_type_tag(&self, idx: Idx) -> Option<TypeTag> {
        self.pool.builtin_type_tag(idx)
    }
}
