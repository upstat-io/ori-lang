//! Typed error identities carried across worker boundaries.

use serde::{Deserialize, Serialize};

use crate::digest::ByteArtifact;

/// Compiler or harness error identity without message parsing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "domain", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ErrorKind {
    Frontend {
        kind: FrontendErrorKind,
    },
    Realization {
        kind: RealizationErrorKind,
    },
    BytecodeCompile {
        kind: CompileErrorKind,
    },
    BytecodeVerify {
        kind: VerifyErrorKind,
    },
    PhysicalPrepare,
    VmExecution {
        kind: ExecutionErrorKind,
    },
    EvaluatorRuntime {
        kind_name: String,
        error_code: String,
    },
    Harness {
        kind: HarnessErrorKind,
    },
}

/// Display text plus exact debug bytes for a typed error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ErrorRecord {
    pub(crate) kind: ErrorKind,
    pub(crate) message: String,
    pub(crate) debug: ByteArtifact,
}

impl ErrorRecord {
    pub(crate) fn new(kind: ErrorKind, message: String, debug: &str) -> Self {
        Self {
            kind,
            message,
            debug: ByteArtifact::complete(debug.as_bytes()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrontendErrorKind {
    Lex,
    Parse,
    Typecheck,
    MissingTypePool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RealizationErrorKind {
    CallableCensus,
    ArcLowering,
    LambdaSpecialization,
    OperatorCallResolution,
    DuplicateArcParent,
    DuplicateArcBody,
    AmbiguousUserDropTarget,
    ProgramMissingUserDropTarget,
    UnexpectedUserDropRole,
    ProgramUserDropLogicalIdentityMismatch,
    ArcVerification,
    Aims,
    Intern,
    UnsupportedVersion,
    TooManyFunctions,
    TooManyExternalFunctions,
    UnknownFunctionName,
    DuplicateFunction,
    MissingFunctionIdentity,
    UnknownFunctionFamilyParent,
    UnknownFunctionFamilyLambda,
    DuplicateFunctionFamilyMember,
    MissingFunctionFamily,
    MissingFunctionContract,
    UnexpectedFunctionContract,
    FunctionContractArity,
    MissingFunctionEffectFacts,
    UnexpectedFunctionEffectFacts,
    FunctionEffectFactsMismatch,
    InvalidFunctionEffectFacts,
    MissingFreshReturnFacts,
    UnexpectedFreshReturnFacts,
    FreshReturnFactsMismatch,
    MissingParamDisjointnessFacts,
    UnexpectedParamDisjointnessFacts,
    ParamDisjointnessArity,
    MissingCallableFacts,
    UnexpectedCallableFacts,
    InvalidCallableFacts,
    MissingClosureAdapterFacts,
    ClosureAdapterFactsMismatch,
    UnexpectedClosureAdapterFacts,
    InvalidClosureAdapterFacts,
    InvalidRetainPlanFacts,
    MissingEntryPoint,
    MissingProgramRoots,
    MissingProgramRoot,
    ProgramRootIsLambda,
    DuplicateProgramRoot,
    CliEntryNotRoot,
    InvalidUserDropType,
    DuplicateUserDropBinding,
    UnexpectedUserDropBinding,
    MissingUserDropBinding,
    UserDropTypeIdentityCollision,
    ExecutableUserDropLogicalIdentityMismatch,
    ExecutableMissingUserDropTarget,
    InvalidUserDropSignature,
    UnknownExternalName,
    ExternalFunctionBodyCollision,
    DuplicateExternalFunction,
    EmptyExternalLinkSymbol,
    ExternalContractArity,
    ExternalUnwindMismatch,
    InvalidExternalSignature,
    StaleExternalFacts,
    ExternalAliasFactsMismatch,
    ExternalCallSignatureMismatch,
    ExternalCallOwnershipMismatch,
    VariableMetadataUnrealized,
    RepresentationMetadata,
    RcStrategyMetadata,
    RcStrategyShape,
    RcStrategyCoherence,
    CallOwnershipMetadata,
    IndirectCallOwnership,
    DuplicateMethodCallFact,
    OrphanMethodCallFact,
    MissingMethodReceiver,
    MethodReceiverMismatch,
    InvalidPrimitiveFacts,
    UnresolvedBoundVar,
    MissingCallable,
    InvalidClosureTarget,
    DuplicateDirectCallResult,
    InvalidGeneratedCallProvenance,
    TooManyBlocks,
    TooManyInstructions,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CompileErrorKind {
    MissingCliEntry,
    MissingFunctionIdentity,
    FunctionTooLarge,
    TableOverflow,
    InvalidBlock,
    JumpArity,
    PrimitiveArity,
    MissingPrimitiveFact,
    InvalidPrimitiveFact,
    UnsupportedPrimitiveProjection,
    CallOwnershipArity,
    UnsupportedIteratorSource,
    UnsupportedRuntimeCall,
    UnsupportedExternalCall,
    ConstructorTooWide,
    UnsupportedConstructor,
    UnsupportedInstruction,
    UnsupportedRcAtomicity,
    UnsupportedRcStrategy,
    RcStrategyMismatch,
    InvalidClosureTarget,
    MissingCallTarget,
    LiteralOutOfRange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VerifyErrorKind {
    ExternalCallTarget,
    InvalidIndex,
    RegisterMetadata,
    RegisterTypeMetadata,
    CallableRegisterMetadata,
    RcRegisterMetadata,
    ParameterOwnershipMetadata,
    CaptureMetadata,
    InvalidCallableMetadata,
    InvalidClosureAdapterMetadata,
    InvalidRetainPlanMetadata,
    ClosureArgumentOwnership,
    ClosureCaptureArity,
    TypedRegister,
    UnsupportedRuntimePrimitive,
    UnsupportedRuntimeCall,
    InvalidFallthrough,
    MainHasParameters,
    CallArity,
    UnsupportedRcAtomicity,
    UnsupportedRcStrategy,
    RcStrategyMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionErrorKind {
    ExternalCallTarget,
    StepLimit,
    StepCounterOverflow,
    MetricOverflow,
    ProgramCounterOverflow,
    InvalidVerifiedIndex,
    TypeMismatch,
    IntegerOperation,
    ReferenceCountOverflow,
    ReferenceCountUnderflow,
    RcStrategyMismatch,
    UnsupportedRcStrategy,
    UnsupportedIteratorSource,
    UnsupportedRuntimeCall,
    HeapOutOfBounds,
    ReleasedHeap,
    ValueArenaHandleOutOfBounds,
    StaleValueArenaHandle,
    LiveIteratorReclamation,
    ValueArenaKindMismatch,
    AggregateFieldOutOfBounds,
    RuntimeArity,
    ZeroRangeStep,
    NegativeInteger,
    CollectionIndexOutOfBounds,
    InvalidHeapObject,
    InvalidPrimitiveObject,
    Panic,
    ResumeWithoutPanic,
    ReachedUnreachable,
    UnsupportedConstructor,
    UnsupportedPrimitive,
    ResourceLimit,
    FrameGenerationOverflow,
    ValueArenaHandleOverflow,
    ValueArenaGenerationOverflow,
    HeapHandleOverflow,
    EscapingIterator,
    EscapingClosure,
    InvalidClosureObject,
    ClosureCallArity,
    ClosureAdapterValueShape,
    ClosureAdapterVariantOutOfBounds,
    InvalidClosureAdapterExecution,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HarnessErrorKind {
    SourceRead,
    WorkerRecordWrite,
    InconsistentWorkerRecord,
    WorkerRecordMissing,
    WorkerRecordInvalid,
    ProcessSpawn,
    ProcessWait,
    ProcessCrash,
    ProcessOutputTruncated,
}
