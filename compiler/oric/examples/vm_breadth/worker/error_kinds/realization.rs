//! Exhaustive realization-error mappings grouped by producer-owned invariant.

use ori_repr::executable::RealizationError;
use oric::realization::ProgramRealizationError;

use crate::errors::RealizationErrorKind;

pub(super) fn program_realization_error_kind(
    error: &ProgramRealizationError,
) -> RealizationErrorKind {
    match error {
        ProgramRealizationError::ArcLowering { .. } => RealizationErrorKind::ArcLowering,
        ProgramRealizationError::LambdaSpecialization { .. } => {
            RealizationErrorKind::LambdaSpecialization
        }
        ProgramRealizationError::OperatorCallResolution { .. } => {
            RealizationErrorKind::OperatorCallResolution
        }
        ProgramRealizationError::DuplicateArcParent { .. } => {
            RealizationErrorKind::DuplicateArcParent
        }
        ProgramRealizationError::DuplicateArcBody { .. } => RealizationErrorKind::DuplicateArcBody,
        ProgramRealizationError::AmbiguousUserDropTarget { .. } => {
            RealizationErrorKind::AmbiguousUserDropTarget
        }
        ProgramRealizationError::MissingUserDropTarget { .. } => {
            RealizationErrorKind::ProgramMissingUserDropTarget
        }
        ProgramRealizationError::UnexpectedUserDropRole { .. } => {
            RealizationErrorKind::UnexpectedUserDropRole
        }
        ProgramRealizationError::UserDropLogicalIdentityMismatch { .. } => {
            RealizationErrorKind::ProgramUserDropLogicalIdentityMismatch
        }
        ProgramRealizationError::ArcVerification { .. } => RealizationErrorKind::ArcVerification,
        ProgramRealizationError::Aims { .. } => RealizationErrorKind::Aims,
        ProgramRealizationError::Intern(_) => RealizationErrorKind::Intern,
        ProgramRealizationError::Executable(error) => realization_error_kind(error),
    }
}

fn realization_error_kind(error: &RealizationError) -> RealizationErrorKind {
    use RealizationError as Error;
    use RealizationErrorKind as Kind;

    match error {
        Error::UnsupportedVersion { .. } => Kind::UnsupportedVersion,
        Error::TooManyFunctions { .. } => Kind::TooManyFunctions,
        Error::TooManyExternalFunctions { .. } => Kind::TooManyExternalFunctions,
        Error::UnknownFunctionName { .. } => Kind::UnknownFunctionName,
        Error::DuplicateFunction { .. } => Kind::DuplicateFunction,
        Error::MissingFunctionIdentity { .. } => Kind::MissingFunctionIdentity,
        Error::UnknownFunctionFamilyParent { .. } => family_kind(FamilyKind::UnknownParent),
        Error::UnknownFunctionFamilyLambda { .. } => family_kind(FamilyKind::UnknownLambda),
        Error::DuplicateFunctionFamilyMember { .. } => family_kind(FamilyKind::DuplicateMember),
        Error::MissingFunctionFamily { .. } => family_kind(FamilyKind::Missing),
        Error::MissingFunctionContract { .. } => fact_kind(FactKind::MissingContract),
        Error::UnexpectedFunctionContract { .. } => fact_kind(FactKind::UnexpectedContract),
        Error::FunctionContractArity { .. } => fact_kind(FactKind::ContractArity),
        Error::MissingFunctionEffectFacts { .. } => fact_kind(FactKind::MissingEffects),
        Error::UnexpectedFunctionEffectFacts { .. } => fact_kind(FactKind::UnexpectedEffects),
        Error::FunctionEffectFactsMismatch { .. } => fact_kind(FactKind::EffectsMismatch),
        Error::InvalidFunctionEffectFacts { .. } => fact_kind(FactKind::InvalidEffects),
        Error::MissingFreshReturnFacts { .. } => fact_kind(FactKind::MissingFreshReturn),
        Error::UnexpectedFreshReturnFacts { .. } => fact_kind(FactKind::UnexpectedFreshReturn),
        Error::FreshReturnFactsMismatch { .. } => fact_kind(FactKind::FreshReturnMismatch),
        Error::MissingParamDisjointnessFacts { .. } => {
            fact_kind(FactKind::MissingParamDisjointness)
        }
        Error::UnexpectedParamDisjointnessFacts { .. } => {
            fact_kind(FactKind::UnexpectedParamDisjointness)
        }
        Error::ParamDisjointnessArity { .. } => fact_kind(FactKind::ParamDisjointnessArity),
        Error::MissingCallableFacts { .. } => fact_kind(FactKind::MissingCallable),
        Error::UnexpectedCallableFacts { .. } => fact_kind(FactKind::UnexpectedCallable),
        Error::InvalidCallableFacts { .. } => fact_kind(FactKind::InvalidCallable),
        Error::MissingClosureAdapterFacts { .. } => fact_kind(FactKind::MissingClosureAdapter),
        Error::ClosureAdapterFactsMismatch { .. } => fact_kind(FactKind::ClosureAdapterMismatch),
        Error::UnexpectedClosureAdapterFacts { .. } => {
            fact_kind(FactKind::UnexpectedClosureAdapter)
        }
        Error::InvalidClosureAdapterFacts { .. } => fact_kind(FactKind::InvalidClosureAdapter),
        Error::InvalidRetainPlanFacts { .. } => fact_kind(FactKind::InvalidRetainPlan),
        Error::MissingEntryPoint { .. } => root_kind(RootKind::MissingEntryPoint),
        Error::MissingProgramRoots => root_kind(RootKind::MissingProgramRoots),
        Error::MissingProgramRoot { .. } => root_kind(RootKind::MissingProgramRoot),
        Error::ProgramRootIsLambda { .. } => root_kind(RootKind::IsLambda),
        Error::DuplicateProgramRoot { .. } => root_kind(RootKind::Duplicate),
        Error::CliEntryNotRoot { .. } => root_kind(RootKind::CliEntryNotRoot),
        Error::InvalidUserDropType { .. } => user_drop_kind(UserDropKind::InvalidType),
        Error::DuplicateUserDropBinding { .. } => user_drop_kind(UserDropKind::DuplicateBinding),
        Error::UnexpectedUserDropBinding { .. } => user_drop_kind(UserDropKind::UnexpectedBinding),
        Error::MissingUserDropBinding { .. } => user_drop_kind(UserDropKind::MissingBinding),
        Error::UserDropTypeIdentityCollision { .. } => {
            user_drop_kind(UserDropKind::TypeIdentityCollision)
        }
        Error::UserDropLogicalIdentityMismatch { .. } => {
            user_drop_kind(UserDropKind::LogicalIdentityMismatch)
        }
        Error::MissingUserDropTarget { .. } => user_drop_kind(UserDropKind::MissingTarget),
        Error::InvalidUserDropSignature { .. } => user_drop_kind(UserDropKind::InvalidSignature),
        Error::UnknownExternalName { .. } => external_kind(ExternalKind::UnknownName),
        Error::ExternalFunctionBodyCollision { .. } => external_kind(ExternalKind::BodyCollision),
        Error::DuplicateExternalFunction { .. } => external_kind(ExternalKind::DuplicateFunction),
        Error::EmptyExternalLinkSymbol { .. } => external_kind(ExternalKind::EmptyLinkSymbol),
        Error::ExternalContractArity { .. } => external_kind(ExternalKind::ContractArity),
        Error::ExternalUnwindMismatch { .. } => external_kind(ExternalKind::UnwindMismatch),
        Error::InvalidExternalSignature { .. } => external_kind(ExternalKind::InvalidSignature),
        Error::StaleExternalFacts { .. } => external_kind(ExternalKind::StaleFacts),
        Error::ExternalAliasFactsMismatch { .. } => external_kind(ExternalKind::AliasFactsMismatch),
        Error::ExternalCallSignatureMismatch { .. } => {
            external_kind(ExternalKind::CallSignatureMismatch)
        }
        Error::ExternalCallOwnershipMismatch { .. } => {
            external_kind(ExternalKind::CallOwnershipMismatch)
        }
        Error::VariableMetadataUnrealized { .. } => Kind::VariableMetadataUnrealized,
        Error::RepresentationMetadata { .. } => Kind::RepresentationMetadata,
        Error::RcStrategyMetadata { .. } => Kind::RcStrategyMetadata,
        Error::RcStrategyShape { .. } => Kind::RcStrategyShape,
        Error::RcStrategyCoherence { .. } => Kind::RcStrategyCoherence,
        Error::CallOwnershipMetadata { .. } => call_kind(CallKind::OwnershipMetadata),
        Error::IndirectCallOwnership { .. } => call_kind(CallKind::IndirectOwnership),
        Error::DuplicateMethodCallFact { .. } => call_kind(CallKind::DuplicateMethodFact),
        Error::OrphanMethodCallFact { .. } => call_kind(CallKind::OrphanMethodFact),
        Error::MissingMethodReceiver { .. } => call_kind(CallKind::MissingMethodReceiver),
        Error::MethodReceiverMismatch { .. } => call_kind(CallKind::MethodReceiverMismatch),
        Error::InvalidPrimitiveFacts { .. } => call_kind(CallKind::InvalidPrimitiveFacts),
        Error::UnresolvedBoundVar { .. } => call_kind(CallKind::UnresolvedBoundVar),
        Error::MissingCallable { .. } => call_kind(CallKind::MissingCallable),
        Error::InvalidClosureTarget { .. } => call_kind(CallKind::InvalidClosureTarget),
        Error::DuplicateDirectCallResult { .. } => call_kind(CallKind::DuplicateDirectCallResult),
        Error::TooManyBlocks { .. } => Kind::TooManyBlocks,
        Error::TooManyInstructions { .. } => Kind::TooManyInstructions,
    }
}

#[derive(Clone, Copy)]
enum FamilyKind {
    UnknownParent,
    UnknownLambda,
    DuplicateMember,
    Missing,
}

fn family_kind(kind: FamilyKind) -> RealizationErrorKind {
    match kind {
        FamilyKind::UnknownParent => RealizationErrorKind::UnknownFunctionFamilyParent,
        FamilyKind::UnknownLambda => RealizationErrorKind::UnknownFunctionFamilyLambda,
        FamilyKind::DuplicateMember => RealizationErrorKind::DuplicateFunctionFamilyMember,
        FamilyKind::Missing => RealizationErrorKind::MissingFunctionFamily,
    }
}

#[derive(Clone, Copy)]
enum FactKind {
    MissingContract,
    UnexpectedContract,
    ContractArity,
    MissingEffects,
    UnexpectedEffects,
    EffectsMismatch,
    InvalidEffects,
    MissingFreshReturn,
    UnexpectedFreshReturn,
    FreshReturnMismatch,
    MissingParamDisjointness,
    UnexpectedParamDisjointness,
    ParamDisjointnessArity,
    MissingCallable,
    UnexpectedCallable,
    InvalidCallable,
    MissingClosureAdapter,
    ClosureAdapterMismatch,
    UnexpectedClosureAdapter,
    InvalidClosureAdapter,
    InvalidRetainPlan,
}

fn fact_kind(kind: FactKind) -> RealizationErrorKind {
    match kind {
        FactKind::MissingContract => RealizationErrorKind::MissingFunctionContract,
        FactKind::UnexpectedContract => RealizationErrorKind::UnexpectedFunctionContract,
        FactKind::ContractArity => RealizationErrorKind::FunctionContractArity,
        FactKind::MissingEffects => RealizationErrorKind::MissingFunctionEffectFacts,
        FactKind::UnexpectedEffects => RealizationErrorKind::UnexpectedFunctionEffectFacts,
        FactKind::EffectsMismatch => RealizationErrorKind::FunctionEffectFactsMismatch,
        FactKind::InvalidEffects => RealizationErrorKind::InvalidFunctionEffectFacts,
        FactKind::MissingFreshReturn => RealizationErrorKind::MissingFreshReturnFacts,
        FactKind::UnexpectedFreshReturn => RealizationErrorKind::UnexpectedFreshReturnFacts,
        FactKind::FreshReturnMismatch => RealizationErrorKind::FreshReturnFactsMismatch,
        FactKind::MissingParamDisjointness => RealizationErrorKind::MissingParamDisjointnessFacts,
        FactKind::UnexpectedParamDisjointness => {
            RealizationErrorKind::UnexpectedParamDisjointnessFacts
        }
        FactKind::ParamDisjointnessArity => RealizationErrorKind::ParamDisjointnessArity,
        FactKind::MissingCallable => RealizationErrorKind::MissingCallableFacts,
        FactKind::UnexpectedCallable => RealizationErrorKind::UnexpectedCallableFacts,
        FactKind::InvalidCallable => RealizationErrorKind::InvalidCallableFacts,
        FactKind::MissingClosureAdapter => RealizationErrorKind::MissingClosureAdapterFacts,
        FactKind::ClosureAdapterMismatch => RealizationErrorKind::ClosureAdapterFactsMismatch,
        FactKind::UnexpectedClosureAdapter => RealizationErrorKind::UnexpectedClosureAdapterFacts,
        FactKind::InvalidClosureAdapter => RealizationErrorKind::InvalidClosureAdapterFacts,
        FactKind::InvalidRetainPlan => RealizationErrorKind::InvalidRetainPlanFacts,
    }
}

#[derive(Clone, Copy)]
enum RootKind {
    MissingEntryPoint,
    MissingProgramRoots,
    MissingProgramRoot,
    IsLambda,
    Duplicate,
    CliEntryNotRoot,
}

fn root_kind(kind: RootKind) -> RealizationErrorKind {
    match kind {
        RootKind::MissingEntryPoint => RealizationErrorKind::MissingEntryPoint,
        RootKind::MissingProgramRoots => RealizationErrorKind::MissingProgramRoots,
        RootKind::MissingProgramRoot => RealizationErrorKind::MissingProgramRoot,
        RootKind::IsLambda => RealizationErrorKind::ProgramRootIsLambda,
        RootKind::Duplicate => RealizationErrorKind::DuplicateProgramRoot,
        RootKind::CliEntryNotRoot => RealizationErrorKind::CliEntryNotRoot,
    }
}

#[derive(Clone, Copy)]
enum UserDropKind {
    InvalidType,
    DuplicateBinding,
    UnexpectedBinding,
    MissingBinding,
    TypeIdentityCollision,
    LogicalIdentityMismatch,
    MissingTarget,
    InvalidSignature,
}

fn user_drop_kind(kind: UserDropKind) -> RealizationErrorKind {
    match kind {
        UserDropKind::InvalidType => RealizationErrorKind::InvalidUserDropType,
        UserDropKind::DuplicateBinding => RealizationErrorKind::DuplicateUserDropBinding,
        UserDropKind::UnexpectedBinding => RealizationErrorKind::UnexpectedUserDropBinding,
        UserDropKind::MissingBinding => RealizationErrorKind::MissingUserDropBinding,
        UserDropKind::TypeIdentityCollision => RealizationErrorKind::UserDropTypeIdentityCollision,
        UserDropKind::LogicalIdentityMismatch => {
            RealizationErrorKind::ExecutableUserDropLogicalIdentityMismatch
        }
        UserDropKind::MissingTarget => RealizationErrorKind::ExecutableMissingUserDropTarget,
        UserDropKind::InvalidSignature => RealizationErrorKind::InvalidUserDropSignature,
    }
}

#[derive(Clone, Copy)]
enum ExternalKind {
    UnknownName,
    BodyCollision,
    DuplicateFunction,
    EmptyLinkSymbol,
    ContractArity,
    UnwindMismatch,
    InvalidSignature,
    StaleFacts,
    AliasFactsMismatch,
    CallSignatureMismatch,
    CallOwnershipMismatch,
}

fn external_kind(kind: ExternalKind) -> RealizationErrorKind {
    match kind {
        ExternalKind::UnknownName => RealizationErrorKind::UnknownExternalName,
        ExternalKind::BodyCollision => RealizationErrorKind::ExternalFunctionBodyCollision,
        ExternalKind::DuplicateFunction => RealizationErrorKind::DuplicateExternalFunction,
        ExternalKind::EmptyLinkSymbol => RealizationErrorKind::EmptyExternalLinkSymbol,
        ExternalKind::ContractArity => RealizationErrorKind::ExternalContractArity,
        ExternalKind::UnwindMismatch => RealizationErrorKind::ExternalUnwindMismatch,
        ExternalKind::InvalidSignature => RealizationErrorKind::InvalidExternalSignature,
        ExternalKind::StaleFacts => RealizationErrorKind::StaleExternalFacts,
        ExternalKind::AliasFactsMismatch => RealizationErrorKind::ExternalAliasFactsMismatch,
        ExternalKind::CallSignatureMismatch => RealizationErrorKind::ExternalCallSignatureMismatch,
        ExternalKind::CallOwnershipMismatch => RealizationErrorKind::ExternalCallOwnershipMismatch,
    }
}

#[derive(Clone, Copy)]
enum CallKind {
    OwnershipMetadata,
    IndirectOwnership,
    DuplicateMethodFact,
    OrphanMethodFact,
    MissingMethodReceiver,
    MethodReceiverMismatch,
    InvalidPrimitiveFacts,
    UnresolvedBoundVar,
    MissingCallable,
    InvalidClosureTarget,
    DuplicateDirectCallResult,
}

fn call_kind(kind: CallKind) -> RealizationErrorKind {
    match kind {
        CallKind::OwnershipMetadata => RealizationErrorKind::CallOwnershipMetadata,
        CallKind::IndirectOwnership => RealizationErrorKind::IndirectCallOwnership,
        CallKind::DuplicateMethodFact => RealizationErrorKind::DuplicateMethodCallFact,
        CallKind::OrphanMethodFact => RealizationErrorKind::OrphanMethodCallFact,
        CallKind::MissingMethodReceiver => RealizationErrorKind::MissingMethodReceiver,
        CallKind::MethodReceiverMismatch => RealizationErrorKind::MethodReceiverMismatch,
        CallKind::InvalidPrimitiveFacts => RealizationErrorKind::InvalidPrimitiveFacts,
        CallKind::UnresolvedBoundVar => RealizationErrorKind::UnresolvedBoundVar,
        CallKind::MissingCallable => RealizationErrorKind::MissingCallable,
        CallKind::InvalidClosureTarget => RealizationErrorKind::InvalidClosureTarget,
        CallKind::DuplicateDirectCallResult => RealizationErrorKind::DuplicateDirectCallResult,
    }
}
