//! Compact bytecode instruction definitions.

use ori_arc::{ArgOwnership, CtorKind, RcAtomicity, RcStrategy};
use ori_ir::{BinaryOp, UnaryOp};
use ori_repr::executable::CallableTarget;

use super::{CallArgumentListId, MoveListId, OperandListId, Pc, Register, StringId, SwitchTableId};

macro_rules! define_opcode_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// Stable opcode identity used by execution profiles.
        ///
        /// The numeric representation is dense but is not a serialized bytecode ABI.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[non_exhaustive]
        #[repr(u8)]
        pub enum OpcodeKind {
            $($variant),+
        }

        impl OpcodeKind {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub(crate) const COUNT: usize = Self::ALL.len();

            pub(crate) const fn index(self) -> usize {
                self as usize
            }

            /// Return the stable human-readable opcode name.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }
        }
    };
}

define_opcode_kinds! {
    Const => "const",
    Copy => "copy",
    Binary => "binary",
    IntBinary => "int_binary",
    StringBinary => "string_binary",
    RuntimeBinary => "runtime_binary",
    Unary => "unary",
    BoolNot => "bool_not",
    Call => "call",
    MakeClosure => "make_closure",
    CallClosure => "call_closure",
    Construct => "construct",
    Project => "project",
    RcInc => "rc_inc",
    RcDec => "rc_dec",
    IsShared => "is_shared",
    Set => "set",
    SetTag => "set_tag",
    Select => "select",
    Jump => "jump",
    Branch => "branch",
    Switch => "switch",
    Return => "return",
    Resume => "resume",
    Unreachable => "unreachable",
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterClass {
    Int,
    Bool,
    String,
    Closure,
    Other,
}

/// Transitional compiled-shaped RC adapter retained at the bytecode boundary.
///
/// This proves current behavior only. Production VM bytecode references stable
/// logical value/drop plans and lets `VmLayoutPlan` choose physical mechanics;
/// it must not preserve `RcStrategy` or `RcAtomicity` as AIMS vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RcSemantics {
    pub(crate) strategy: RcStrategy,
    pub(crate) atomicity: RcAtomicity,
}

/// One realized call argument and its post-AIMS ownership contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CallArgument {
    register: Register,
    ownership: ArgOwnership,
}

impl CallArgument {
    pub(crate) const fn new(register: Register, ownership: ArgOwnership) -> Self {
        Self {
            register,
            ownership,
        }
    }

    pub(crate) const fn register(self) -> Register {
        self.register
    }

    pub(crate) const fn ownership(self) -> ArgOwnership {
        self.ownership
    }
}

const _: () = assert!(core::mem::size_of::<CallArgument>() == 8);

impl RcSemantics {
    pub(crate) const fn new(strategy: RcStrategy, atomicity: RcAtomicity) -> Self {
        Self {
            strategy,
            atomicity,
        }
    }

    pub(crate) const fn has_supported_atomicity(self) -> bool {
        matches!(self.atomicity, RcAtomicity::Atomic)
    }

    pub(crate) const fn has_supported_strategy(self) -> bool {
        matches!(
            self.strategy,
            RcStrategy::HeapPointer
                | RcStrategy::FatPointer
                | RcStrategy::AggregateFields
                | RcStrategy::InlineEnum
                | RcStrategy::Iterator
                | RcStrategy::Closure
        )
    }
}

impl RegisterClass {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Bool => "bool",
            Self::String => "str",
            Self::Closure => "closure",
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
    RuntimeBinary {
        dst: Register,
        operator: ori_registry::RuntimeOperator,
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
        args: CallArgumentListId,
        normal: Continuation,
        unwind: Option<Pc>,
    },
    MakeClosure {
        dst: Register,
        callee: ori_repr::executable::FunctionId,
        captures: OperandListId,
    },
    CallClosure {
        dst: Register,
        closure: Register,
        args: CallArgumentListId,
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
        semantics: RcSemantics,
    },
    RcDec {
        var: Register,
        semantics: RcSemantics,
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
    pub(crate) const fn kind(self) -> OpcodeKind {
        match self {
            Self::Const { .. } => OpcodeKind::Const,
            Self::Copy { .. } => OpcodeKind::Copy,
            Self::Binary { .. } => OpcodeKind::Binary,
            Self::IntBinary { .. } => OpcodeKind::IntBinary,
            Self::StringBinary { .. } => OpcodeKind::StringBinary,
            Self::RuntimeBinary { .. } => OpcodeKind::RuntimeBinary,
            Self::Unary { .. } => OpcodeKind::Unary,
            Self::BoolNot { .. } => OpcodeKind::BoolNot,
            Self::Call { .. } => OpcodeKind::Call,
            Self::MakeClosure { .. } => OpcodeKind::MakeClosure,
            Self::CallClosure { .. } => OpcodeKind::CallClosure,
            Self::Construct { .. } => OpcodeKind::Construct,
            Self::Project { .. } => OpcodeKind::Project,
            Self::RcInc { .. } => OpcodeKind::RcInc,
            Self::RcDec { .. } => OpcodeKind::RcDec,
            Self::IsShared { .. } => OpcodeKind::IsShared,
            Self::Set { .. } => OpcodeKind::Set,
            Self::SetTag { .. } => OpcodeKind::SetTag,
            Self::Select { .. } => OpcodeKind::Select,
            Self::Jump { .. } => OpcodeKind::Jump,
            Self::Branch { .. } => OpcodeKind::Branch,
            Self::Switch { .. } => OpcodeKind::Switch,
            Self::Return { .. } => OpcodeKind::Return,
            Self::Resume => OpcodeKind::Resume,
            Self::Unreachable => OpcodeKind::Unreachable,
        }
    }

    pub(crate) const fn is_linear_dispatch(self) -> bool {
        match self {
            Self::Const { .. }
            | Self::Copy { .. }
            | Self::Binary { .. }
            | Self::IntBinary { .. }
            | Self::StringBinary { .. }
            | Self::RuntimeBinary { .. }
            | Self::Unary { .. }
            | Self::BoolNot { .. }
            | Self::MakeClosure { .. }
            | Self::Construct { .. }
            | Self::Project { .. }
            | Self::RcInc { .. }
            | Self::RcDec { .. }
            | Self::IsShared { .. }
            | Self::Set { .. }
            | Self::SetTag { .. }
            | Self::Select { .. } => true,
            Self::Call { .. }
            | Self::CallClosure { .. }
            | Self::Jump { .. }
            | Self::Branch { .. }
            | Self::Switch { .. }
            | Self::Return { .. }
            | Self::Resume
            | Self::Unreachable => false,
        }
    }

    pub(crate) const fn needs_next_pc(self) -> bool {
        match self {
            Self::Const { .. }
            | Self::Copy { .. }
            | Self::Binary { .. }
            | Self::IntBinary { .. }
            | Self::StringBinary { .. }
            | Self::RuntimeBinary { .. }
            | Self::Unary { .. }
            | Self::BoolNot { .. }
            | Self::MakeClosure { .. }
            | Self::Construct { .. }
            | Self::Project { .. }
            | Self::RcInc { .. }
            | Self::RcDec { .. }
            | Self::IsShared { .. }
            | Self::Set { .. }
            | Self::SetTag { .. }
            | Self::Select { .. } => true,
            Self::Call { normal, .. } | Self::CallClosure { normal, .. } => {
                matches!(normal, Continuation::Next)
            }
            Self::Jump { .. }
            | Self::Branch { .. }
            | Self::Switch { .. }
            | Self::Return { .. }
            | Self::Resume
            | Self::Unreachable => false,
        }
    }
}
