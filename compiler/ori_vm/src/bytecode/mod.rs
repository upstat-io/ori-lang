//! Opaque unverified and verified bytecode artifacts.

mod ids;
mod op;
mod operands;

use ori_ir::Name;
use ori_repr::executable::FunctionId;

pub(crate) use ids::{
    CallArgumentListId, MoveListId, OperandListId, Pc, Register, StringId, SwitchTableId,
};
pub use op::OpcodeKind;
pub(crate) use op::{
    CallArgument, Constant, Continuation, IntBinaryOp, Op, RcSemantics, RegisterClass,
    StringBinaryOp,
};
pub(crate) use operands::{walk_register_operands, RegisterOperand};

/// Stable semantic type identity projected without retaining the type pool.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct VmTypeId(u32);

impl VmTypeId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// Stable VM-local projection of a shared retain-plan identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct VmRetainPlanId(u32);

impl VmRetainPlanId {
    pub(crate) fn from_shared(id: ori_arc::RetainPlanId) -> Self {
        Self(id.raw())
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VmRetainEdge {
    pub(crate) field: u32,
    pub(crate) child: VmRetainPlanId,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct VmRetainPlan {
    pub(crate) ty: VmTypeId,
    pub(crate) kind: VmRetainPlanKind,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum VmRetainPlanKind {
    SelfOwnedIdentity,
    OwnedFields(Box<[VmRetainEdge]>),
    OwnedVariants(Box<[Box<[VmRetainEdge]>]>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VmCalleeOwnerDemand {
    Borrow,
    WholeValue,
    ProjectedField(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VmClosureValueSignature {
    pub(crate) ty: VmTypeId,
    pub(crate) parameters: Box<[VmTypeId]>,
    pub(crate) result: VmTypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VmClosureAdapterSource {
    EnvironmentCapture,
    BorrowedCallArgument,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VmClosureAdapterAction {
    Borrow,
    Copy,
    Retain(VmRetainPlanId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VmClosureAdapterSlot {
    pub(crate) source: VmClosureAdapterSource,
    pub(crate) ty: VmTypeId,
    pub(crate) demand: VmCalleeOwnerDemand,
    pub(crate) action: VmClosureAdapterAction,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct VmClosureAdapterPlan {
    pub(crate) capture_count: usize,
    pub(crate) slots: Box<[VmClosureAdapterSlot]>,
}

/// A bytecode side-table category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableKind {
    /// Constructor operands and closure-capture lists.
    Operands,
    /// Call arguments paired with post-AIMS ownership.
    CallArguments,
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
    pub(crate) return_type: VmTypeId,
    /// Shared borrow-inference verdict for every parameter.
    pub(crate) param_ownership: Box<[ori_arc::Ownership]>,
    /// Leading parameters populated from a closure environment.
    pub(crate) capture_count: usize,
    /// VM projection of the shared logical adapter facts when this function is
    /// used as a closure target.
    pub(crate) closure_adapter: Option<VmClosureAdapterPlan>,
    pub(crate) ops: Box<[Op]>,
    pub(crate) entry: Pc,
    pub(crate) register_count: usize,
    pub(crate) register_types: Box<[VmTypeId]>,
    pub(crate) register_classes: Box<[RegisterClass]>,
    pub(crate) register_rc_strategies: Box<[Option<ori_arc::RcStrategy>]>,
    /// Residual arity for every function-typed register; `None` for all other
    /// registers. Verified before execution so indirect arity cannot depend on
    /// runtime provenance.
    pub(crate) register_closure_signatures: Box<[Option<VmClosureValueSignature>]>,
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
    pub(crate) call_arguments: Vec<Box<[CallArgument]>>,
    pub(crate) operands: Vec<Box<[Register]>>,
    pub(crate) moves: Vec<Box<[(Register, Register)]>>,
    pub(crate) switches: Vec<Box<[(u64, Pc)]>>,
    pub(crate) strings: Vec<String>,
    pub(crate) retain_plans: Box<[VmRetainPlan]>,
    pub(crate) main: FunctionId,
    metrics: BytecodeMetrics,
}

pub(crate) struct BytecodeProgramParts {
    pub(crate) functions: Vec<BytecodeFunction>,
    pub(crate) call_arguments: Vec<Box<[CallArgument]>>,
    pub(crate) operands: Vec<Box<[Register]>>,
    pub(crate) moves: Vec<Box<[(Register, Register)]>>,
    pub(crate) switches: Vec<Box<[(u64, Pc)]>>,
    pub(crate) strings: Vec<String>,
    pub(crate) retain_plans: Vec<VmRetainPlan>,
    pub(crate) main: FunctionId,
}

impl BytecodeProgram {
    /// Return stable bytecode size metrics.
    #[must_use]
    pub const fn metrics(&self) -> BytecodeMetrics {
        self.metrics
    }

    #[cfg(test)]
    pub(crate) fn new(
        functions: Vec<BytecodeFunction>,
        call_arguments: Vec<Box<[CallArgument]>>,
        operands: Vec<Box<[Register]>>,
        moves: Vec<Box<[(Register, Register)]>>,
        switches: Vec<Box<[(u64, Pc)]>>,
        strings: Vec<String>,
        main: FunctionId,
    ) -> Self {
        Self::from_parts(BytecodeProgramParts {
            functions,
            call_arguments,
            operands,
            moves,
            switches,
            strings,
            retain_plans: Vec::new(),
            main,
        })
    }

    pub(crate) fn from_parts(parts: BytecodeProgramParts) -> Self {
        let BytecodeProgramParts {
            functions,
            call_arguments,
            operands,
            moves,
            switches,
            strings,
            retain_plans,
            main,
        } = parts;
        let metrics = BytecodeMetrics {
            function_count: functions.len(),
            instruction_count: functions.iter().map(|function| function.ops.len()).sum(),
        };
        Self {
            functions: functions.into_boxed_slice(),
            call_arguments,
            operands,
            moves,
            switches,
            strings,
            retain_plans: retain_plans.into_boxed_slice(),
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
