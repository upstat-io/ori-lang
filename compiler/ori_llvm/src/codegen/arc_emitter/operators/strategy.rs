//! `OpStrategy` dispatch helpers for binary and unary operator emission.
//!
//! Maps `(TypeTag, BinaryOp/UnaryOp)` to LLVM instruction families via the
//! registry's [`OpStrategy`]. Each strategy variant delegates to a focused
//! helper that contains the `match op` for that instruction family.

use ori_ir::{BinaryOp, UnaryOp};
use ori_registry::{find_type, OpStrategy, TypeTag};
use ori_types::Idx;

use super::super::builtins;
use super::super::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Look up the [`OpStrategy`] for a binary operation on a builtin type.
    ///
    /// Maps `(TypeTag, BinaryOp)` to the corresponding strategy field in the
    /// registry's [`OpDefs`](ori_registry::OpDefs). Structural operations
    /// (`And`/`Or`/`Coalesce`) bypass the registry lookup entirely.
    pub(in crate::codegen::arc_emitter) fn op_strategy_for_binary(
        type_tag: TypeTag,
        op: BinaryOp,
    ) -> OpStrategy {
        // Structural ops: not type-dependent, bypass registry lookup.
        match op {
            BinaryOp::And | BinaryOp::Or | BinaryOp::Coalesce => return OpStrategy::IntInstr,
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
                return OpStrategy::Unsupported;
            }
            _ => {}
        }

        let Some(type_def) = find_type(type_tag) else {
            return OpStrategy::Unsupported;
        };
        match op {
            BinaryOp::Add => type_def.operators.add,
            BinaryOp::Sub => type_def.operators.sub,
            BinaryOp::Mul => type_def.operators.mul,
            BinaryOp::Div => type_def.operators.div,
            BinaryOp::Mod => type_def.operators.rem,
            BinaryOp::FloorDiv => type_def.operators.floor_div,
            BinaryOp::Eq => type_def.operators.eq,
            BinaryOp::NotEq => type_def.operators.neq,
            BinaryOp::Lt => type_def.operators.lt,
            BinaryOp::Gt => type_def.operators.gt,
            BinaryOp::LtEq => type_def.operators.lt_eq,
            BinaryOp::GtEq => type_def.operators.gt_eq,
            BinaryOp::BitAnd => type_def.operators.bit_and,
            BinaryOp::BitOr => type_def.operators.bit_or,
            BinaryOp::BitXor => type_def.operators.bit_xor,
            BinaryOp::Shl => type_def.operators.shl,
            BinaryOp::Shr => type_def.operators.shr,
            // Already handled above
            BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::Coalesce
            | BinaryOp::Range
            | BinaryOp::RangeInclusive
            | BinaryOp::MatMul => unreachable!(),
        }
    }

    /// Look up the [`OpStrategy`] for a unary operation on a builtin type.
    ///
    /// Maps `(TypeTag, UnaryOp)` to the corresponding strategy field in the
    /// registry's [`OpDefs`](ori_registry::OpDefs). `Try` is always
    /// `Unsupported` because it is desugared before reaching ARC IR.
    pub(in crate::codegen::arc_emitter) fn op_strategy_for_unary(
        type_tag: TypeTag,
        op: UnaryOp,
    ) -> OpStrategy {
        // Try is desugared before reaching ARC IR.
        if matches!(op, UnaryOp::Try) {
            return OpStrategy::Unsupported;
        }

        let Some(type_def) = find_type(type_tag) else {
            return OpStrategy::Unsupported;
        };
        match op {
            UnaryOp::Neg => type_def.operators.neg,
            UnaryOp::Not => type_def.operators.not,
            UnaryOp::BitNot => type_def.operators.bit_not,
            // Already handled above
            UnaryOp::Try => unreachable!(),
        }
    }

    /// Emit a binary op using signed integer LLVM instructions.
    ///
    /// Handles arithmetic (`checked_add`, `checked_sub`, etc.), signed comparison
    /// (`icmp slt`), bitwise ops, and structural ops (`And`/`Or`/`Coalesce`).
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
            BinaryOp::Div => self.builder.sdiv(lhs, rhs, "div"),
            BinaryOp::Mod => self.builder.srem(lhs, rhs, "rem"),
            BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
            BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
            BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
            BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
            BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
            BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
            BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
            BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
            BinaryOp::Coalesce => self.emit_coalesce(lhs, rhs),
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
                unreachable!("desugared op {op:?} should not reach emit_int_binary_op")
            }
        }
    }

    /// Emit the coalesce operation (`??`).
    ///
    /// `opt ?? default` → extract tag, if Some return payload else default.
    /// Same pattern for `Result`: Ok → payload, else default.
    /// (`OPTION_TAG_SOME` == `RESULT_TAG_OK` == 0)
    fn emit_coalesce(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let tag = self
            .builder
            .extract_value(lhs, 0, "coal.tag")
            .unwrap_or(lhs);
        let payload = self
            .builder
            .extract_value(lhs, 1, "coal.val")
            .unwrap_or(lhs);
        let some_tag = self
            .builder
            .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
        let is_some = self.builder.icmp_eq(tag, some_tag, "is_some");
        self.builder.select(is_some, payload, rhs, "coal")
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
            BinaryOp::NotEq => self.builder.fcmp_one(lhs, rhs, "ne"),
            BinaryOp::Lt => self.builder.fcmp_olt(lhs, rhs, "lt"),
            BinaryOp::Gt => self.builder.fcmp_ogt(lhs, rhs, "gt"),
            BinaryOp::LtEq => self.builder.fcmp_ole(lhs, rhs, "le"),
            BinaryOp::GtEq => self.builder.fcmp_oge(lhs, rhs, "ge"),
            _ => unreachable!("unsupported float binary op {op:?}"),
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
            _ => unreachable!("unsupported unsigned binary op {op:?}"),
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
            _ => unreachable!("unsupported bool binary op {op:?}"),
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
            _ => unreachable!("unsupported runtime binary op {op:?}"),
        }
    }

    // Registry bridge

    /// Map a type pool [`Idx`] to a registry [`TypeTag`] for `OpStrategy` lookup.
    ///
    /// This is the bridge between the type checker's pool-based type system
    /// and the registry's static type tag system. For primitive types (Idx 0-11),
    /// the mapping is a direct match on the well-known index constants.
    /// For dynamic types, we consult the [`TypeInfo`] store.
    ///
    /// Returns `None` for user-defined structs/enums — these are handled by
    /// trait dispatch before reaching `OpStrategy`, so `None` here indicates a
    /// compiler bug if the caller expected a builtin type.
    pub(in crate::codegen::arc_emitter) fn idx_to_type_tag(&self, idx: Idx) -> Option<TypeTag> {
        // Fast path: well-known primitive indices (0-11).
        // Idx::ERROR (index 8) intentionally excluded — error types should
        // never reach codegen; if they do, returning None triggers an ICE
        // at the call site.
        let tag = match idx {
            Idx::INT => TypeTag::Int,
            Idx::FLOAT => TypeTag::Float,
            Idx::BOOL => TypeTag::Bool,
            Idx::STR => TypeTag::Str,
            Idx::CHAR => TypeTag::Char,
            Idx::BYTE => TypeTag::Byte,
            Idx::UNIT => TypeTag::Unit,
            Idx::NEVER => TypeTag::Never,
            Idx::DURATION => TypeTag::Duration,
            Idx::SIZE => TypeTag::Size,
            Idx::ORDERING => TypeTag::Ordering,
            _ => {
                // Dynamic types: consult TypeInfoStore.
                return match self.type_info.get(idx) {
                    TypeInfo::Int => Some(TypeTag::Int),
                    TypeInfo::Float => Some(TypeTag::Float),
                    TypeInfo::Bool => Some(TypeTag::Bool),
                    TypeInfo::Char => Some(TypeTag::Char),
                    TypeInfo::Byte => Some(TypeTag::Byte),
                    TypeInfo::Str => Some(TypeTag::Str),
                    TypeInfo::Unit => Some(TypeTag::Unit),
                    TypeInfo::Never => Some(TypeTag::Never),
                    TypeInfo::Duration => Some(TypeTag::Duration),
                    TypeInfo::Size => Some(TypeTag::Size),
                    TypeInfo::Ordering => Some(TypeTag::Ordering),
                    TypeInfo::Error => Some(TypeTag::Error),
                    TypeInfo::List { .. } => Some(TypeTag::List),
                    TypeInfo::Map { .. } => Some(TypeTag::Map),
                    TypeInfo::Set { .. } => Some(TypeTag::Set),
                    TypeInfo::Tuple { .. } => Some(TypeTag::Tuple),
                    TypeInfo::Option { .. } => Some(TypeTag::Option),
                    TypeInfo::Result { .. } => Some(TypeTag::Result),
                    TypeInfo::Range => Some(TypeTag::Range),
                    TypeInfo::Iterator { .. } => Some(TypeTag::Iterator),
                    TypeInfo::Channel { .. } => Some(TypeTag::Channel),
                    TypeInfo::Function { .. } => Some(TypeTag::Function),
                    // Struct/Enum are handled by trait dispatch (non-primitives).
                    // Returning None signals the caller to ICE.
                    TypeInfo::Struct { .. } | TypeInfo::Enum { .. } => None,
                };
            }
        };
        Some(tag)
    }
}
