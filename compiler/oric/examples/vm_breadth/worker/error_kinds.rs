//! Exhaustive typed mappings from compiler errors to protocol identities.

#[path = "error_kinds/realization.rs"]
mod realization;

use std::fmt::{Debug, Display};

use ori_repr::executable::RealizationError;
use oric::test_support::ExecutableCompileError;

use crate::contract::{Phase, VmWorkerDisposition};
use crate::errors::{
    CompileErrorKind, ErrorKind, ErrorRecord, ExecutionErrorKind, FrontendErrorKind,
    VerifyErrorKind,
};
use realization::program_realization_error_kind;

pub(super) fn executable_error_kind(
    error: &ExecutableCompileError,
) -> (VmWorkerDisposition, Phase, ErrorKind) {
    match error {
        ExecutableCompileError::Parse { .. } => (
            VmWorkerDisposition::FrontendRejected,
            Phase::Parse,
            ErrorKind::Frontend {
                kind: FrontendErrorKind::Parse,
            },
        ),
        ExecutableCompileError::Type { .. } => (
            VmWorkerDisposition::FrontendRejected,
            Phase::Typecheck,
            ErrorKind::Frontend {
                kind: FrontendErrorKind::Typecheck,
            },
        ),
        ExecutableCompileError::MissingTypePool { .. } => (
            VmWorkerDisposition::RealizationError,
            Phase::Realization,
            ErrorKind::Frontend {
                kind: FrontendErrorKind::MissingTypePool,
            },
        ),
        ExecutableCompileError::Realization(error) => {
            let kind = program_realization_error_kind(error);
            let unsupported = matches!(
                error,
                oric::realization::ProgramRealizationError::Executable(
                    RealizationError::MissingCallable { .. }
                        | RealizationError::InvalidClosureTarget { .. }
                )
            );
            (
                if unsupported {
                    VmWorkerDisposition::Unsupported
                } else {
                    VmWorkerDisposition::RealizationError
                },
                Phase::Realization,
                ErrorKind::Realization { kind },
            )
        }
    }
}

pub(super) fn compile_error_kind(error: &ori_vm::CompileError) -> CompileErrorKind {
    match error {
        ori_vm::CompileError::MissingCliEntry => CompileErrorKind::MissingCliEntry,
        ori_vm::CompileError::MissingFunctionIdentity { .. } => {
            CompileErrorKind::MissingFunctionIdentity
        }
        ori_vm::CompileError::FunctionTooLarge { .. } => CompileErrorKind::FunctionTooLarge,
        ori_vm::CompileError::TableOverflow { .. } => CompileErrorKind::TableOverflow,
        ori_vm::CompileError::InvalidBlock { .. } => CompileErrorKind::InvalidBlock,
        ori_vm::CompileError::JumpArity { .. } => CompileErrorKind::JumpArity,
        ori_vm::CompileError::PrimitiveArity { .. } => CompileErrorKind::PrimitiveArity,
        ori_vm::CompileError::MissingPrimitiveFact { .. } => CompileErrorKind::MissingPrimitiveFact,
        ori_vm::CompileError::InvalidPrimitiveFact { .. } => CompileErrorKind::InvalidPrimitiveFact,
        ori_vm::CompileError::UnsupportedPrimitiveProjection { .. } => {
            CompileErrorKind::UnsupportedPrimitiveProjection
        }
        ori_vm::CompileError::CallOwnershipArity { .. } => CompileErrorKind::CallOwnershipArity,
        ori_vm::CompileError::UnsupportedIteratorSource { .. } => {
            CompileErrorKind::UnsupportedIteratorSource
        }
        ori_vm::CompileError::UnsupportedRuntimeCall { .. } => {
            CompileErrorKind::UnsupportedRuntimeCall
        }
        ori_vm::CompileError::UnsupportedExternalCall { .. } => {
            CompileErrorKind::UnsupportedExternalCall
        }
        ori_vm::CompileError::ConstructorTooWide { .. } => CompileErrorKind::ConstructorTooWide,
        ori_vm::CompileError::UnsupportedConstructor { .. } => {
            CompileErrorKind::UnsupportedConstructor
        }
        ori_vm::CompileError::UnsupportedInstruction { .. } => {
            CompileErrorKind::UnsupportedInstruction
        }
        ori_vm::CompileError::UnsupportedRcAtomicity { .. } => {
            CompileErrorKind::UnsupportedRcAtomicity
        }
        ori_vm::CompileError::UnsupportedRcStrategy { .. } => {
            CompileErrorKind::UnsupportedRcStrategy
        }
        ori_vm::CompileError::RcStrategyMismatch { .. } => CompileErrorKind::RcStrategyMismatch,
        ori_vm::CompileError::InvalidClosureTarget { .. } => CompileErrorKind::InvalidClosureTarget,
        ori_vm::CompileError::MissingCallTarget { .. } => CompileErrorKind::MissingCallTarget,
        ori_vm::CompileError::LiteralOutOfRange { .. } => CompileErrorKind::LiteralOutOfRange,
    }
}

pub(super) fn compile_error_is_unsupported(error: &ori_vm::CompileError) -> bool {
    matches!(
        error,
        ori_vm::CompileError::ConstructorTooWide { .. }
            | ori_vm::CompileError::UnsupportedIteratorSource { .. }
            | ori_vm::CompileError::UnsupportedRuntimeCall { .. }
            | ori_vm::CompileError::UnsupportedExternalCall { .. }
            | ori_vm::CompileError::UnsupportedConstructor { .. }
            | ori_vm::CompileError::UnsupportedInstruction { .. }
            | ori_vm::CompileError::UnsupportedRcAtomicity { .. }
            | ori_vm::CompileError::UnsupportedRcStrategy { .. }
            | ori_vm::CompileError::UnsupportedPrimitiveProjection { .. }
    )
}

pub(super) fn verify_error_kind(error: &ori_vm::VerifyError) -> VerifyErrorKind {
    match error {
        ori_vm::VerifyError::ExternalCallTarget { .. } => VerifyErrorKind::ExternalCallTarget,
        ori_vm::VerifyError::InvalidIndex { .. } => VerifyErrorKind::InvalidIndex,
        ori_vm::VerifyError::RegisterMetadata { .. } => VerifyErrorKind::RegisterMetadata,
        ori_vm::VerifyError::RegisterTypeMetadata { .. } => VerifyErrorKind::RegisterTypeMetadata,
        ori_vm::VerifyError::CallableRegisterMetadata { .. } => {
            VerifyErrorKind::CallableRegisterMetadata
        }
        ori_vm::VerifyError::RcRegisterMetadata { .. } => VerifyErrorKind::RcRegisterMetadata,
        ori_vm::VerifyError::ParameterOwnershipMetadata { .. } => {
            VerifyErrorKind::ParameterOwnershipMetadata
        }
        ori_vm::VerifyError::CaptureMetadata { .. } => VerifyErrorKind::CaptureMetadata,
        ori_vm::VerifyError::InvalidCallableMetadata { .. } => {
            VerifyErrorKind::InvalidCallableMetadata
        }
        ori_vm::VerifyError::InvalidClosureAdapterMetadata { .. } => {
            VerifyErrorKind::InvalidClosureAdapterMetadata
        }
        ori_vm::VerifyError::InvalidRetainPlanMetadata { .. } => {
            VerifyErrorKind::InvalidRetainPlanMetadata
        }
        ori_vm::VerifyError::ClosureArgumentOwnership { .. } => {
            VerifyErrorKind::ClosureArgumentOwnership
        }
        ori_vm::VerifyError::ClosureCaptureArity { .. } => VerifyErrorKind::ClosureCaptureArity,
        ori_vm::VerifyError::TypedRegister { .. } => VerifyErrorKind::TypedRegister,
        ori_vm::VerifyError::UnsupportedRuntimePrimitive { .. } => {
            VerifyErrorKind::UnsupportedRuntimePrimitive
        }
        ori_vm::VerifyError::UnsupportedRuntimeCall { .. } => {
            VerifyErrorKind::UnsupportedRuntimeCall
        }
        ori_vm::VerifyError::InvalidFallthrough { .. } => VerifyErrorKind::InvalidFallthrough,
        ori_vm::VerifyError::MainHasParameters { .. } => VerifyErrorKind::MainHasParameters,
        ori_vm::VerifyError::CallArity { .. } => VerifyErrorKind::CallArity,
        ori_vm::VerifyError::UnsupportedRcAtomicity { .. } => {
            VerifyErrorKind::UnsupportedRcAtomicity
        }
        ori_vm::VerifyError::UnsupportedRcStrategy { .. } => VerifyErrorKind::UnsupportedRcStrategy,
        ori_vm::VerifyError::RcStrategyMismatch { .. } => VerifyErrorKind::RcStrategyMismatch,
    }
}

pub(super) fn execution_error_kind(error: &ori_vm::ExecutionError) -> ExecutionErrorKind {
    match error {
        ori_vm::ExecutionError::ExternalCallTarget { .. } => ExecutionErrorKind::ExternalCallTarget,
        ori_vm::ExecutionError::StepLimit { .. } => ExecutionErrorKind::StepLimit,
        ori_vm::ExecutionError::StepCounterOverflow => ExecutionErrorKind::StepCounterOverflow,
        ori_vm::ExecutionError::MetricOverflow { .. } => ExecutionErrorKind::MetricOverflow,
        ori_vm::ExecutionError::ProgramCounterOverflow { .. } => {
            ExecutionErrorKind::ProgramCounterOverflow
        }
        ori_vm::ExecutionError::InvalidVerifiedIndex { .. } => {
            ExecutionErrorKind::InvalidVerifiedIndex
        }
        ori_vm::ExecutionError::TypeMismatch { .. } => ExecutionErrorKind::TypeMismatch,
        ori_vm::ExecutionError::IntegerOperation { .. } => ExecutionErrorKind::IntegerOperation,
        ori_vm::ExecutionError::ReferenceCountOverflow => {
            ExecutionErrorKind::ReferenceCountOverflow
        }
        ori_vm::ExecutionError::ReferenceCountUnderflow => {
            ExecutionErrorKind::ReferenceCountUnderflow
        }
        ori_vm::ExecutionError::RcStrategyMismatch { .. } => ExecutionErrorKind::RcStrategyMismatch,
        ori_vm::ExecutionError::UnsupportedRcStrategy { .. } => {
            ExecutionErrorKind::UnsupportedRcStrategy
        }
        ori_vm::ExecutionError::UnsupportedIteratorSource { .. } => {
            ExecutionErrorKind::UnsupportedIteratorSource
        }
        ori_vm::ExecutionError::UnsupportedRuntimeCall { .. } => {
            ExecutionErrorKind::UnsupportedRuntimeCall
        }
        ori_vm::ExecutionError::HeapOutOfBounds { .. } => ExecutionErrorKind::HeapOutOfBounds,
        ori_vm::ExecutionError::ReleasedHeap { .. } => ExecutionErrorKind::ReleasedHeap,
        ori_vm::ExecutionError::ValueArenaHandleOutOfBounds { .. } => {
            ExecutionErrorKind::ValueArenaHandleOutOfBounds
        }
        ori_vm::ExecutionError::StaleValueArenaHandle { .. } => {
            ExecutionErrorKind::StaleValueArenaHandle
        }
        ori_vm::ExecutionError::LiveIteratorReclamation { .. } => {
            ExecutionErrorKind::LiveIteratorReclamation
        }
        ori_vm::ExecutionError::ValueArenaKindMismatch { .. } => {
            ExecutionErrorKind::ValueArenaKindMismatch
        }
        ori_vm::ExecutionError::AggregateFieldOutOfBounds { .. } => {
            ExecutionErrorKind::AggregateFieldOutOfBounds
        }
        ori_vm::ExecutionError::RuntimeArity { .. } => ExecutionErrorKind::RuntimeArity,
        ori_vm::ExecutionError::ZeroRangeStep => ExecutionErrorKind::ZeroRangeStep,
        ori_vm::ExecutionError::NegativeInteger { .. } => ExecutionErrorKind::NegativeInteger,
        ori_vm::ExecutionError::CollectionIndexOutOfBounds { .. } => {
            ExecutionErrorKind::CollectionIndexOutOfBounds
        }
        ori_vm::ExecutionError::InvalidHeapObject { .. } => ExecutionErrorKind::InvalidHeapObject,
        ori_vm::ExecutionError::InvalidPrimitiveObject { .. } => {
            ExecutionErrorKind::InvalidPrimitiveObject
        }
        ori_vm::ExecutionError::Panic { .. } => ExecutionErrorKind::Panic,
        ori_vm::ExecutionError::ResumeWithoutPanic => ExecutionErrorKind::ResumeWithoutPanic,
        ori_vm::ExecutionError::CatchRecoverWithoutPanic => {
            ExecutionErrorKind::CatchRecoverWithoutPanic
        }
        ori_vm::ExecutionError::ReachedUnreachable => ExecutionErrorKind::ReachedUnreachable,
        ori_vm::ExecutionError::UnsupportedConstructor { .. } => {
            ExecutionErrorKind::UnsupportedConstructor
        }
        ori_vm::ExecutionError::UnsupportedPrimitive { .. } => {
            ExecutionErrorKind::UnsupportedPrimitive
        }
        ori_vm::ExecutionError::ResourceLimit { .. } => ExecutionErrorKind::ResourceLimit,
        ori_vm::ExecutionError::FrameGenerationOverflow => {
            ExecutionErrorKind::FrameGenerationOverflow
        }
        ori_vm::ExecutionError::ValueArenaHandleOverflow { .. } => {
            ExecutionErrorKind::ValueArenaHandleOverflow
        }
        ori_vm::ExecutionError::ValueArenaGenerationOverflow { .. } => {
            ExecutionErrorKind::ValueArenaGenerationOverflow
        }
        ori_vm::ExecutionError::HeapHandleOverflow { .. } => ExecutionErrorKind::HeapHandleOverflow,
        ori_vm::ExecutionError::EscapingIterator => ExecutionErrorKind::EscapingIterator,
        ori_vm::ExecutionError::EscapingClosure => ExecutionErrorKind::EscapingClosure,
        ori_vm::ExecutionError::InvalidClosureObject => ExecutionErrorKind::InvalidClosureObject,
        ori_vm::ExecutionError::ClosureCallArity { .. } => ExecutionErrorKind::ClosureCallArity,
        ori_vm::ExecutionError::ClosureAdapterValueShape { .. } => {
            ExecutionErrorKind::ClosureAdapterValueShape
        }
        ori_vm::ExecutionError::ClosureAdapterVariantOutOfBounds { .. } => {
            ExecutionErrorKind::ClosureAdapterVariantOutOfBounds
        }
        ori_vm::ExecutionError::InvalidClosureAdapterExecution { .. } => {
            ExecutionErrorKind::InvalidClosureAdapterExecution
        }
    }
}

pub(super) fn record_error(error_kind: ErrorKind, error: &(impl Display + Debug)) -> ErrorRecord {
    ErrorRecord::new(error_kind, error.to_string(), &format!("{error:?}"))
}
