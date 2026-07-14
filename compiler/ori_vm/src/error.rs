//! Typed failures at VM phase boundaries.

use ori_arc::CtorKind;
use ori_ir::Name;
use ori_repr::executable::{FunctionId, RuntimeCall};

use crate::bytecode::TableKind;

/// Post-AIMS instruction categories not yet implemented by bytecode lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcInstructionKind {
    /// Indirect function application.
    ApplyIndirect,
    /// Partial application or closure creation.
    PartialApply,
    /// Reference-count decrement of a partially initialized value.
    RcDecPartial,
    /// Reference-count decrement of one field.
    RcDecField,
    /// Reference-count decrement selected by an enum variant.
    RcDecVariant,
    /// Burden-count increment.
    BurdenInc,
    /// Burden-count decrement.
    BurdenDec,
    /// Burden decrement of a partially initialized value.
    BurdenDecPartial,
    /// Burden decrement of one field.
    BurdenDecField,
    /// Burden decrement selected by an enum variant.
    BurdenDecVariant,
    /// Reuse-token reset.
    Reset,
    /// Allocation reuse.
    Reuse,
    /// Collection-buffer reuse.
    CollectionReuse,
}

/// Bytecode compilation failure over an otherwise validated executable program.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CompileError {
    /// A bytecode function exceeds the PC representation.
    #[error("function {function:?} has {count} bytecode positions, exceeding the VM limit")]
    FunctionTooLarge { function: Name, count: usize },
    /// A side table exceeds its stable ID representation.
    #[error("bytecode {table:?} table has {count} entries, exceeding the VM limit")]
    TableOverflow { table: TableKind, count: usize },
    /// An ARC block refers to a block absent from its owning function.
    #[error("function {function:?} refers to missing block {block}")]
    InvalidBlock { function: Name, block: usize },
    /// A jump's arguments do not match the destination block parameters.
    #[error("function {function:?} jumps with {actual} arguments to {expected} parameters")]
    JumpArity {
        function: Name,
        expected: usize,
        actual: usize,
    },
    /// A primitive operation has the wrong number of operands.
    #[error("function {function:?} has a primitive operation with {actual} operands; expected {expected}")]
    PrimitiveArity {
        function: Name,
        expected: usize,
        actual: usize,
    },
    /// A constructor exceeds the baseline aggregate representation.
    #[error("function {function:?} constructs {fields} fields; the VM baseline supports at most {limit}")]
    ConstructorTooWide {
        function: Name,
        fields: usize,
        limit: usize,
    },
    /// A constructor kind has no baseline bytecode runtime representation.
    #[error("function {function:?} uses unsupported constructor {constructor:?}")]
    UnsupportedConstructor {
        function: Name,
        constructor: CtorKind,
    },
    /// A post-AIMS instruction has no bytecode lowering yet.
    #[error("function {function:?} contains unsupported post-AIMS instruction {instruction:?}")]
    UnsupportedInstruction {
        function: Name,
        instruction: ArcInstructionKind,
    },
    /// Indirect unwind-aware invocation has no bytecode lowering yet.
    #[error("function {function:?} contains an unsupported indirect invoke")]
    UnsupportedIndirectInvoke { function: Name },
    /// A realized call-site table entry is unexpectedly absent.
    #[error("function {function:?} has no resolved target for block {block}, position {position}")]
    MissingCallTarget {
        function: Name,
        block: usize,
        position: usize,
    },
    /// A duration or size literal cannot fit the VM integer representation.
    #[error("function {function:?} contains a duration or size literal outside the VM i64 range")]
    LiteralOutOfRange { function: Name },
}

/// The kind of bytecode reference rejected by verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexKind {
    /// Function index.
    Function,
    /// Program counter.
    ProgramCounter,
    /// Register index.
    Register,
    /// Operand-list index.
    Operands,
    /// Move-list index.
    Moves,
    /// Switch-table index.
    Switch,
    /// String-constant index.
    String,
    /// Heap allocation index.
    Heap,
}

/// Structural bytecode verification failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VerifyError {
    /// A bytecode reference falls outside its owning table.
    #[error(
        "function {function:?} at {pc:?} has invalid {kind:?} index {index}; bound is {bound}"
    )]
    InvalidIndex {
        function: Name,
        pc: Option<usize>,
        kind: IndexKind,
        index: usize,
        bound: usize,
    },
    /// Register type metadata does not cover the complete register file.
    #[error("function {function:?} declares {registers} registers but has {classes} register type entries")]
    RegisterMetadata {
        function: Name,
        registers: usize,
        classes: usize,
    },
    /// A typed bytecode operation disagrees with its declared register type.
    #[error("function {function:?} at bytecode position {pc} uses register {register} as {expected}, but it is declared {found}")]
    TypedRegister {
        function: Name,
        pc: usize,
        register: usize,
        expected: &'static str,
        found: &'static str,
    },
    /// An operation that advances implicitly occupies the final position.
    #[error("function {function:?} ends with a fallthrough operation at bytecode position {pc}")]
    InvalidFallthrough { function: Name, pc: usize },
    /// Entry-point main unexpectedly accepts arguments.
    #[error("VM entry function {function:?} requires {parameters} argument(s)")]
    MainHasParameters { function: Name, parameters: usize },
    /// A direct call has the wrong arity for its resolved target.
    #[error("function {function:?} at bytecode position {pc} calls {target:?} with {actual} arguments; expected {expected}")]
    CallArity {
        function: Name,
        pc: usize,
        target: ori_repr::executable::CallableTarget,
        expected: usize,
        actual: usize,
    },
}

/// Runtime value categories used in typed execution diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    /// Unit.
    Unit,
    /// Integer.
    Int,
    /// Boolean.
    Bool,
    /// Float.
    Float,
    /// Character.
    Char,
    /// Null.
    Null,
    /// Constant string.
    ConstantString,
    /// Heap allocation.
    Heap,
    /// Frame-local aggregate.
    Aggregate,
    /// Frame-local iterator.
    Iterator,
}

/// Interpreted execution failure.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    /// The configured deterministic instruction budget was exhausted.
    #[error("VM step limit {limit} exceeded")]
    StepLimit { limit: u64 },
    /// The executed-step metric overflowed.
    #[error("VM step counter overflow")]
    StepCounterOverflow,
    /// A function's program counter overflowed.
    #[error("program counter overflow in function {function:?}")]
    ProgramCounterOverflow { function: FunctionId },
    /// Verified bytecode unexpectedly referenced invalid state.
    #[error("verified bytecode invariant failed for {kind:?} index {index}; bound is {bound}")]
    InvalidVerifiedIndex {
        kind: IndexKind,
        index: usize,
        bound: usize,
    },
    /// An operation received a runtime value of the wrong category.
    #[error("expected {expected:?} value, found {found:?}")]
    TypeMismatch {
        expected: ValueKind,
        found: ValueKind,
    },
    /// Integer arithmetic overflowed or used an invalid divisor/shift.
    #[error("integer operation {operation} failed")]
    IntegerOperation { operation: &'static str },
    /// A heap reference count overflowed.
    #[error("heap reference count overflow")]
    ReferenceCountOverflow,
    /// A heap reference count underflowed.
    #[error("heap reference count underflow")]
    ReferenceCountUnderflow,
    /// A heap handle is outside the allocation table.
    #[error("heap handle {index} is outside allocation table of length {bound}")]
    HeapOutOfBounds { index: usize, bound: usize },
    /// A heap handle refers to a released allocation.
    #[error("heap handle {index} refers to a released allocation")]
    ReleasedHeap { index: usize },
    /// A frame-local handle refers to a frame or slot that does not exist.
    #[error("frame-local {kind:?} handle ({frame}, {slot}) is out of bounds")]
    LocalHandleOutOfBounds {
        kind: ValueKind,
        frame: usize,
        slot: usize,
    },
    /// A local aggregate field does not exist.
    #[error("aggregate field {field} is outside aggregate length {length}")]
    AggregateFieldOutOfBounds { field: usize, length: usize },
    /// A runtime operation received the wrong number of arguments.
    #[error("runtime call {call:?} received {actual} arguments; expected {expected}")]
    RuntimeArity {
        call: RuntimeCall,
        expected: usize,
        actual: usize,
    },
    /// A range iterator uses a zero step.
    #[error("range step cannot be zero")]
    ZeroRangeStep,
    /// A collection capacity or index was negative.
    #[error("{purpose} cannot be negative: {value}")]
    NegativeInteger { purpose: &'static str, value: i64 },
    /// A collection index is outside its current length.
    #[error("collection index {index} is outside length {length}")]
    CollectionIndexOutOfBounds { index: usize, length: usize },
    /// A runtime operation received the wrong heap object kind.
    #[error("runtime call {call:?} does not support this heap object")]
    InvalidHeapObject { call: RuntimeCall },
    /// An Ori panic escaped the interpreted program.
    #[error("Ori panic: {message}")]
    Panic { message: String },
    /// Execution reached ARC resume without modeled panic state.
    #[error("resumed without a pending VM panic")]
    ResumeWithoutPanic,
    /// Execution reached an unreachable instruction.
    #[error("executed unreachable bytecode")]
    ReachedUnreachable,
    /// A baseline constructor has no interpreter implementation yet.
    #[error("constructor {constructor} is not supported by the VM runtime")]
    UnsupportedConstructor { constructor: &'static str },
    /// A primitive operation has no interpreter implementation yet.
    #[error("primitive operation {operation} is not supported for these values")]
    UnsupportedPrimitive { operation: &'static str },
    /// A configured resource limit was reached before an allocation or write.
    #[error("VM {resource} limit {limit} exceeded")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    /// A frame-local handle cannot represent the current stack depth.
    #[error("VM stack depth {depth} exceeds the frame-handle representation")]
    FrameHandleOverflow { depth: usize },
    /// A heap handle cannot represent another allocation slot.
    #[error("VM heap length {length} exceeds the heap-handle representation")]
    HeapHandleOverflow { length: usize },
    /// An iterator attempted to escape the frame that owns its mutable state.
    #[error("an iterator escaped its owning VM frame")]
    EscapingIterator,
}

pub(crate) fn invalid_verified_index(
    kind: IndexKind,
    index: usize,
    bound: usize,
) -> ExecutionError {
    ExecutionError::InvalidVerifiedIndex { kind, index, bound }
}
