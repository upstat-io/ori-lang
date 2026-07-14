//! Compact bytecode instruction definitions.

use ori_arc::CtorKind;
use ori_ir::{BinaryOp, UnaryOp};
use ori_repr::executable::CallableTarget;

use super::{MoveListId, OperandListId, Pc, Register, StringId, SwitchTableId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterClass {
    Int,
    Bool,
    String,
    Other,
}

impl RegisterClass {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Bool => "bool",
            Self::String => "str",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum IntBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    FloorDiv,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl IntBinaryOp {
    pub(crate) const fn from_binary(operation: BinaryOp) -> Option<Self> {
        match operation {
            BinaryOp::Add => Some(Self::Add),
            BinaryOp::Sub => Some(Self::Sub),
            BinaryOp::Mul => Some(Self::Mul),
            BinaryOp::Div => Some(Self::Div),
            BinaryOp::Mod => Some(Self::Mod),
            BinaryOp::FloorDiv => Some(Self::FloorDiv),
            BinaryOp::Eq => Some(Self::Eq),
            BinaryOp::NotEq => Some(Self::NotEq),
            BinaryOp::Lt => Some(Self::Lt),
            BinaryOp::LtEq => Some(Self::LtEq),
            BinaryOp::Gt => Some(Self::Gt),
            BinaryOp::GtEq => Some(Self::GtEq),
            BinaryOp::BitAnd => Some(Self::BitAnd),
            BinaryOp::BitOr => Some(Self::BitOr),
            BinaryOp::BitXor => Some(Self::BitXor),
            BinaryOp::Shl => Some(Self::Shl),
            BinaryOp::Shr => Some(Self::Shr),
            BinaryOp::MatMul
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::Range
            | BinaryOp::RangeInclusive
            | BinaryOp::Coalesce => None,
        }
    }

    pub(crate) const fn returns_bool(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::NotEq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StringBinaryOp {
    Concat,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

impl StringBinaryOp {
    pub(crate) const fn from_binary(operation: BinaryOp) -> Option<Self> {
        match operation {
            BinaryOp::Add => Some(Self::Concat),
            BinaryOp::Eq => Some(Self::Eq),
            BinaryOp::NotEq => Some(Self::NotEq),
            BinaryOp::Lt => Some(Self::Lt),
            BinaryOp::LtEq => Some(Self::LtEq),
            BinaryOp::Gt => Some(Self::Gt),
            BinaryOp::GtEq => Some(Self::GtEq),
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
            | BinaryOp::Coalesce => None,
        }
    }

    pub(crate) const fn returns_bool(self) -> bool {
        !matches!(self, Self::Concat)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Constant {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(StringId),
    Char(char),
    Unit,
    Null,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Continuation {
    Next,
    At(Pc),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Op {
    Const {
        dst: Register,
        value: Constant,
    },
    Copy {
        dst: Register,
        src: Register,
    },
    Binary {
        dst: Register,
        op: BinaryOp,
        lhs: Register,
        rhs: Register,
    },
    IntBinary {
        dst: Register,
        op: IntBinaryOp,
        lhs: Register,
        rhs: Register,
    },
    StringBinary {
        dst: Register,
        op: StringBinaryOp,
        lhs: Register,
        rhs: Register,
    },
    Unary {
        dst: Register,
        op: UnaryOp,
        arg: Register,
    },
    BoolNot {
        dst: Register,
        arg: Register,
    },
    Call {
        dst: Register,
        callee: CallableTarget,
        args: OperandListId,
        normal: Continuation,
        unwind: Option<Pc>,
    },
    Construct {
        dst: Register,
        ctor: CtorKind,
        args: OperandListId,
    },
    Project {
        dst: Register,
        value: Register,
        field: u32,
    },
    RcInc {
        var: Register,
        count: u32,
    },
    RcDec {
        var: Register,
    },
    IsShared {
        dst: Register,
        var: Register,
    },
    Set {
        base: Register,
        field: u32,
        value: Register,
    },
    SetTag {
        base: Register,
        tag: u64,
    },
    Select {
        dst: Register,
        cond: Register,
        true_value: Register,
        false_value: Register,
    },
    Jump {
        target: Pc,
        moves: MoveListId,
    },
    Branch {
        cond: Register,
        then_pc: Pc,
        else_pc: Pc,
    },
    Switch {
        scrutinee: Register,
        table: SwitchTableId,
        default_pc: Pc,
    },
    Return {
        value: Register,
    },
    Resume,
    Unreachable,
}

impl Op {
    pub(crate) const fn needs_next_pc(self) -> bool {
        match self {
            Self::Const { .. }
            | Self::Copy { .. }
            | Self::Binary { .. }
            | Self::IntBinary { .. }
            | Self::StringBinary { .. }
            | Self::Unary { .. }
            | Self::BoolNot { .. }
            | Self::Construct { .. }
            | Self::Project { .. }
            | Self::RcInc { .. }
            | Self::RcDec { .. }
            | Self::IsShared { .. }
            | Self::Set { .. }
            | Self::SetTag { .. }
            | Self::Select { .. } => true,
            Self::Call { normal, .. } => matches!(normal, Continuation::Next),
            Self::Jump { .. }
            | Self::Branch { .. }
            | Self::Switch { .. }
            | Self::Return { .. }
            | Self::Resume
            | Self::Unreachable => false,
        }
    }
}
