//! Opaque unverified and verified bytecode artifacts.

mod ids;
mod op;

use ori_ir::Name;
use ori_repr::executable::FunctionId;

pub(crate) use ids::{MoveListId, OperandListId, Pc, Register, StringId, SwitchTableId};
pub(crate) use op::{Constant, Continuation, IntBinaryOp, Op, RegisterClass, StringBinaryOp};

/// A bytecode side-table category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableKind {
    /// Call and constructor operand lists.
    Operands,
    /// Parallel block-parameter moves.
    Moves,
    /// Switch case tables.
    Switches,
    /// Interned bytecode string constants.
    Strings,
}

#[derive(Debug)]
pub(crate) struct BytecodeFunction {
    pub(crate) name: Name,
    pub(crate) params: Box<[Register]>,
    pub(crate) ops: Box<[Op]>,
    pub(crate) entry: Pc,
    pub(crate) register_count: usize,
    pub(crate) register_classes: Box<[RegisterClass]>,
}

/// Size metrics for a compiled bytecode program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BytecodeMetrics {
    /// Number of realized bytecode functions.
    pub function_count: usize,
    /// Total number of bytecode instructions.
    pub instruction_count: usize,
}

/// Compiled bytecode that cannot execute until structural verification succeeds.
#[derive(Debug)]
pub struct BytecodeProgram {
    pub(crate) functions: Box<[BytecodeFunction]>,
    pub(crate) operands: Vec<Box<[Register]>>,
    pub(crate) moves: Vec<Box<[(Register, Register)]>>,
    pub(crate) switches: Vec<Box<[(u64, Pc)]>>,
    pub(crate) strings: Vec<String>,
    pub(crate) main: FunctionId,
    metrics: BytecodeMetrics,
}

impl BytecodeProgram {
    /// Return stable bytecode size metrics.
    #[must_use]
    pub const fn metrics(&self) -> BytecodeMetrics {
        self.metrics
    }

    pub(crate) fn new(
        functions: Vec<BytecodeFunction>,
        operands: Vec<Box<[Register]>>,
        moves: Vec<Box<[(Register, Register)]>>,
        switches: Vec<Box<[(u64, Pc)]>>,
        strings: Vec<String>,
        main: FunctionId,
    ) -> Self {
        let metrics = BytecodeMetrics {
            function_count: functions.len(),
            instruction_count: functions.iter().map(|function| function.ops.len()).sum(),
        };
        Self {
            functions: functions.into_boxed_slice(),
            operands,
            moves,
            switches,
            strings,
            main,
            metrics,
        }
    }
}

/// Structurally verified bytecode accepted by the interpreter.
#[derive(Debug)]
pub struct VerifiedProgram {
    pub(crate) program: BytecodeProgram,
}

impl VerifiedProgram {
    /// Return stable bytecode size metrics.
    #[must_use]
    pub const fn metrics(&self) -> BytecodeMetrics {
        self.program.metrics()
    }
}
