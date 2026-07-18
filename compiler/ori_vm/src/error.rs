//! Typed failures at VM phase boundaries.

use ori_arc::{CtorKind, RcAtomicity, RcStrategy};
use ori_ir::Name;
use ori_repr::executable::{ExternalFunctionId, FunctionId, IteratorSource, RuntimeCall};

use crate::bytecode::TableKind;

/// Post-AIMS instruction categories not yet implemented by bytecode lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcInstructionKind {
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
    /// Bytecode execution currently starts at the distinguished CLI entry.
    #[error("VM bytecode compilation requires a distinguished command-line entry")]
    MissingCliEntry,
    /// A realized function disappeared from the executable identity table.
    #[error("function {function:?} has no executable function identity")]
    MissingFunctionIdentity { function: Name },
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
    /// A realized primitive site lacks its frozen AIMS descriptor.
    #[error("function {function:?} has no frozen primitive fact for register {destination}")]
    MissingPrimitiveFact { function: Name, destination: usize },
    /// A frozen primitive descriptor is malformed for the instruction arity.
    #[error(
        "function {function:?} has an invalid frozen primitive fact for register {destination}"
    )]
    InvalidPrimitiveFact { function: Name, destination: usize },
    /// A valid shared primitive strategy has no VM projection in this tier.
    #[error("function {function:?} primitive register {destination} uses unsupported VM strategy {strategy:?}")]
    UnsupportedPrimitiveProjection {
        function: Name,
        destination: usize,
        strategy: ori_registry::OpStrategy,
    },
    /// Post-AIMS call ownership and source arguments have different lengths.
    #[error("function {function:?} has {arguments} call arguments but {ownership_entries} ownership entries")]
    CallOwnershipArity {
        function: Name,
        arguments: usize,
        ownership_entries: usize,
    },
    /// Iterator source identity is preserved but not implemented by this VM tier.
    #[error(
        "VM cannot lower the {iterator_source:?} iterator source in function {function:?} yet"
    )]
    UnsupportedIteratorSource {
        function: Name,
        iterator_source: IteratorSource,
    },
    /// A closed runtime identity has no implementation in this VM tier.
    #[error("VM cannot lower runtime call {call:?} in function {function:?} yet")]
    UnsupportedRuntimeCall { function: Name, call: RuntimeCall },
    /// This VM tier does not link external compiled-unit callables.
    #[error("VM cannot lower external callable {external:?} in function {function:?}")]
    UnsupportedExternalCall {
        function: Name,
        external: ExternalFunctionId,
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
    /// Artifact closure promised a function body but lowering found a runtime target.
    #[error("function {function:?} has non-function closure target {target:?}")]
    InvalidClosureTarget {
        function: Name,
        target: ori_repr::executable::CallableTarget,
    },
    /// A post-AIMS instruction has no bytecode lowering yet.
    #[error("VM cannot execute {operation} in function '{function_symbol}' yet; call a named function directly or select the evaluator/LLVM backend for this program")]
    UnsupportedInstruction {
        function: Name,
        function_symbol: Box<str>,
        instruction: ArcInstructionKind,
        operation: &'static str,
    },
    /// An RC instruction requests counter arithmetic the VM cannot execute.
    #[error("function {function:?} uses unsupported VM reference-count atomicity {atomicity:?}")]
    UnsupportedRcAtomicity {
        function: Name,
        atomicity: RcAtomicity,
    },
    /// An RC instruction requests traversal the VM cannot execute.
    #[error("function {function:?} uses unsupported VM reference-count strategy {strategy:?}")]
    UnsupportedRcStrategy {
        function: Name,
        strategy: RcStrategy,
    },
    /// An RC instruction's strategy disagrees with its realized value representation.
    #[error("function {function:?} uses reference-count strategy {found:?} for register {register}, but its realized representation requires {expected:?}")]
    RcStrategyMismatch {
        function: Name,
        register: usize,
        expected: Option<RcStrategy>,
        found: RcStrategy,
    },
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
    /// Call-argument-list index.
    CallArguments,
    /// Move-list index.
    Moves,
    /// Switch-table index.
    Switch,
    /// String-constant index.
    String,
    /// Logical retain-plan index.
    RetainPlan,
    /// Heap allocation index.
    Heap,
}

/// Structural bytecode verification failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VerifyError {
    /// External compiled-unit calls are excluded from verified VM bytecode.
    #[error("function {function:?} at bytecode position {pc} references unsupported external callable {external:?}")]
    ExternalCallTarget {
        function: Name,
        pc: usize,
        external: ExternalFunctionId,
    },
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
    /// Semantic type identities do not cover the complete register file.
    #[error("function {function:?} declares {registers} registers but has {types} semantic type entries")]
    RegisterTypeMetadata {
        function: Name,
        registers: usize,
        types: usize,
    },
    /// Callable signatures do not cover the complete register file.
    #[error("function {function:?} declares {registers} registers but has {signatures} callable-signature entries")]
    CallableRegisterMetadata {
        function: Name,
        registers: usize,
        signatures: usize,
    },
    /// RC strategy metadata does not cover the complete register file.
    #[error("function {function:?} declares {registers} registers but has {strategies} register RC-strategy entries")]
    RcRegisterMetadata {
        function: Name,
        registers: usize,
        strategies: usize,
    },
    /// Parameter ownership metadata is not parallel to the parameter table.
    #[error("function {function:?} declares {parameters} parameters but has {ownership_entries} ownership entries")]
    ParameterOwnershipMetadata {
        function: Name,
        parameters: usize,
        ownership_entries: usize,
    },
    /// A function claims more closure captures than it has parameters.
    #[error("function {function:?} declares {captures} closure captures but has only {parameters} parameters")]
    CaptureMetadata {
        function: Name,
        captures: usize,
        parameters: usize,
    },
    /// Frozen callable metadata disagrees with a bytecode relationship.
    #[error("function {function:?} at {pc:?} has invalid callable metadata: {details}")]
    InvalidCallableMetadata {
        function: Name,
        pc: Option<usize>,
        details: &'static str,
    },
    /// A function's closure adapter does not match its frozen signature.
    #[error("function {function:?} has invalid closure adapter metadata: {details}")]
    InvalidClosureAdapterMetadata {
        function: Name,
        details: &'static str,
    },
    /// A projected logical retain plan is not closed and deterministic.
    #[error("retain plan {plan} is invalid: {details}")]
    InvalidRetainPlanMetadata { plan: usize, details: &'static str },
    /// A residual indirect call violates the all-borrowed convention.
    #[error("function {function:?} at bytecode position {pc} passes closure argument {argument} as non-borrowed")]
    ClosureArgumentOwnership {
        function: Name,
        pc: usize,
        argument: usize,
    },
    /// Closure construction does not match the target function's capture prefix.
    #[error("function {function:?} at bytecode position {pc} captures {actual} values for {target:?}; expected {expected}")]
    ClosureCaptureArity {
        function: Name,
        pc: usize,
        target: FunctionId,
        expected: usize,
        actual: usize,
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
    /// A typed runtime identity has no verified bytecode execution path.
    #[error("function {function:?} at bytecode position {pc} uses unsupported runtime primitive {operator:?}")]
    UnsupportedRuntimePrimitive {
        function: Name,
        pc: usize,
        operator: ori_registry::RuntimeOperator,
    },
    /// A closed builtin-method identity has no verified VM projection yet.
    #[error(
        "function {function:?} at bytecode position {pc} uses unsupported runtime call {call:?}"
    )]
    UnsupportedRuntimeCall {
        function: Name,
        pc: usize,
        call: RuntimeCall,
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
    /// An RC opcode requests counter arithmetic outside the verified VM subset.
    #[error("function {function:?} at bytecode position {pc} uses unsupported reference-count atomicity {atomicity:?}")]
    UnsupportedRcAtomicity {
        function: Name,
        pc: usize,
        atomicity: RcAtomicity,
    },
    /// An RC opcode requests traversal outside the verified VM subset.
    #[error("function {function:?} at bytecode position {pc} uses unsupported reference-count strategy {strategy:?}")]
    UnsupportedRcStrategy {
        function: Name,
        pc: usize,
        strategy: RcStrategy,
    },
    /// An RC opcode's strategy disagrees with its register representation.
    #[error("function {function:?} at bytecode position {pc} uses reference-count strategy {found:?} for register {register}, but the register requires {expected:?}")]
    RcStrategyMismatch {
        function: Name,
        pc: usize,
        register: usize,
        expected: Option<RcStrategy>,
        found: RcStrategy,
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
    /// Session-arena aggregate.
    Aggregate,
    /// Session-arena iterator.
    Iterator,
    /// Allocation-free optional value produced by advancing an iterator.
    IteratorStep,
}

/// Interpreted execution failure.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    /// A verified program unexpectedly retained an external call target.
    #[error("verified VM bytecode reached external callable {external:?}")]
    ExternalCallTarget { external: ExternalFunctionId },
    /// The configured deterministic instruction budget was exhausted.
    #[error("VM step limit {limit} exceeded")]
    StepLimit { limit: u64 },
    /// The executed-step metric overflowed.
    #[error("VM step counter overflow")]
    StepCounterOverflow,
    /// A session-local resource metric exceeded its counter representation.
    #[error("VM resource metric '{metric}' overflowed")]
    MetricOverflow { metric: &'static str },
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
    /// Realized ARC metadata disagrees with the runtime value representation.
    #[error("RC strategy {strategy:?} does not support runtime value {found:?}")]
    RcStrategyMismatch {
        strategy: ori_arc::RcStrategy,
        found: ValueKind,
    },
    /// Verified bytecode reached an RC strategy excluded from this VM tier.
    #[error("RC strategy {strategy:?} is not supported by this VM tier")]
    UnsupportedRcStrategy { strategy: ori_arc::RcStrategy },
    /// Verified input reached an iterator source excluded by the VM compiler.
    #[error("VM iterator source {iterator_source:?} is not implemented by this execution tier")]
    UnsupportedIteratorSource { iterator_source: IteratorSource },
    /// Verified execution reached a registered method excluded by this VM tier.
    #[error("VM runtime call {call:?} is not implemented by this execution tier")]
    UnsupportedRuntimeCall { call: RuntimeCall },
    /// A heap handle is outside the allocation table.
    #[error("heap handle {index} is outside allocation table of length {bound}")]
    HeapOutOfBounds { index: usize, bound: usize },
    /// A heap handle refers to a released allocation.
    #[error("heap handle {index} refers to a released allocation")]
    ReleasedHeap { index: usize },
    /// A value-arena handle refers to an entry that does not exist.
    #[error("value-arena handle {index} is outside allocation table of length {bound}")]
    ValueArenaHandleOutOfBounds { index: usize, bound: usize },
    /// A value-arena handle refers to a reclaimed or reused entry.
    #[error(
        "value-arena handle {index} generation {handle_generation} does not name a live entry at slot generation {slot_generation}"
    )]
    StaleValueArenaHandle {
        index: usize,
        handle_generation: u32,
        slot_generation: u32,
    },
    /// Arena collection found a live iterator without a compiler-owned drop edge.
    #[error("value-arena collection would reclaim live iterator at slot {index}; the ARC control-flow path must run ori_iter_drop before the iterator becomes unreachable")]
    LiveIteratorReclamation { index: usize },
    /// A value-arena handle refers to an entry of the wrong category.
    #[error("value-arena handle {index} refers to {found:?}, not {expected:?}")]
    ValueArenaKindMismatch {
        index: usize,
        expected: ValueKind,
        found: ValueKind,
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
    /// A typed primitive received a heap object outside its semantic domain.
    #[error("runtime primitive {operator:?} does not support this heap object")]
    InvalidPrimitiveObject {
        operator: ori_registry::RuntimeOperator,
    },
    /// An Ori panic escaped the interpreted program.
    #[error("Ori panic: {message}")]
    Panic { message: String },
    /// Execution reached ARC resume without modeled panic state.
    #[error("resumed without a pending VM panic")]
    ResumeWithoutPanic,
    /// Catch recovery executed without a pending panic to consume.
    #[error("catch recovery executed without a pending VM panic")]
    CatchRecoverWithoutPanic,
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
    /// The activation-generation counter cannot represent another frame.
    #[error("VM frame generation counter overflow")]
    FrameGenerationOverflow,
    /// A value-arena handle cannot represent another entry.
    #[error("VM value arena length {length} exceeds the handle representation")]
    ValueArenaHandleOverflow { length: usize },
    /// A value-arena slot cannot represent another reuse generation.
    #[error("VM value arena slot {index} exhausted its reuse generations")]
    ValueArenaGenerationOverflow { index: usize },
    /// A heap handle cannot represent another allocation slot.
    #[error("VM heap length {length} exceeds the heap-handle representation")]
    HeapHandleOverflow { length: usize },
    /// An iterator escaped or outlived the activation that owns its mutable state.
    #[error("an iterator escaped or outlived its owning VM frame activation")]
    EscapingIterator,
    /// A closure cannot be represented as a process exit value.
    #[error("a closure escaped as the VM result; call the closure before returning it")]
    EscapingClosure,
    /// A closure opcode received a heap value with another runtime representation.
    #[error("closure execution expected an RC-counted closure environment")]
    InvalidClosureObject,
    /// A closure call's explicit arguments do not match its frozen target.
    #[error(
        "closure call supplied {actual} explicit argument(s), but its frozen target requires {expected}"
    )]
    ClosureCallArity { expected: usize, actual: usize },
    /// Runtime values disagreed with a structurally verified closure adapter.
    #[error(
        "verified closure adapter expected {expected}, found {found:?}; this indicates invalid compiler output"
    )]
    ClosureAdapterValueShape {
        expected: &'static str,
        found: ValueKind,
    },
    /// An active variant is absent from its frozen retain topology.
    #[error(
        "verified closure adapter selected variant {variant}, but its retain topology has {variants} variant(s); this indicates invalid compiler output"
    )]
    ClosureAdapterVariantOutOfBounds { variant: usize, variants: usize },
    /// Structurally verified closure-adapter facts failed during execution.
    #[error(
        "verified closure adapter invariant failed: {details}; this indicates invalid compiler output"
    )]
    InvalidClosureAdapterExecution { details: &'static str },
}

pub(crate) fn invalid_verified_index(
    kind: IndexKind,
    index: usize,
    bound: usize,
) -> ExecutionError {
    ExecutionError::InvalidVerifiedIndex { kind, index, bound }
}
