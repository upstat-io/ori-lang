//! Typed failures from frontend-to-executable realization.

use ori_repr::executable::RealizationError;
use ori_types::Idx;

use super::super::CallableCensusError;

/// A typed failure in frontend-to-executable realization.
#[derive(Debug, thiserror::Error)]
pub enum ProgramRealizationError {
    /// Raw declarations could not form one semantic callable seed inventory.
    #[error(transparent)]
    CallableCensus(#[from] CallableCensusError),
    /// ARC lowering rejected one or more ordinary or monomorphized bodies.
    #[error("ARC lowering produced {count} problem(s): {problems:?}")]
    ArcLowering {
        /// Number of lowering problems.
        count: usize,
        /// Structured lowering problems.
        problems: Vec<ori_arc::ArcProblem>,
    },
    /// Shared pre-AIMS lambda specialization could not make every body concrete.
    #[error("lambda specialization produced {count} error(s): {errors:?}")]
    LambdaSpecialization {
        /// Number of parent/lambda batches that could not be specialized.
        count: usize,
        /// Structured specialization failures.
        errors: Vec<ori_arc::LambdaSpecializationError>,
    },
    /// A user-defined operator lacked one exact callable identity.
    #[error("operator-call resolution produced {count} error(s): {errors:?}")]
    OperatorCallResolution {
        /// Number of unresolved operator sites.
        count: usize,
        /// Structured resolution failures.
        errors: Vec<ori_arc::OperatorCallResolutionError>,
    },
    /// A source-selected method handle did not resolve against typed producer
    /// metadata before callable closure.
    #[error(
        "selected-method producer resolution produced {count} error(s): {errors:?}. This is an internal compiler error; report this complete message"
    )]
    SelectedMethodProducerResolution {
        /// Number of invalid selected call sites.
        count: usize,
        /// Exact invalid handle/conflict descriptions.
        errors: Vec<String>,
    },
    /// The pre-AIMS generic callable census could not reach a closed inventory.
    #[error("generic target census failed: {message}")]
    GenericMonoClosure {
        /// Exact closure failure retained without erasing its actionable context.
        message: String,
    },
    /// Two lowering sources claimed the same parent callable identity.
    #[error(
        "ARC batch contains duplicate parent callable `{parent}` because multiple lowering sources claimed one executable body. Run with `ORI_LOG=oric::realization::arc_batch=debug` and report this compiler error"
    )]
    DuplicateArcParent { parent: String },
    /// One body identity appeared in more than one parent/lambda position.
    #[error(
        "ARC batch body `{body}` appears under both `{first_parent}` and `{second_parent}`; every executable body must belong to exactly one family. Run with `ORI_LOG=oric::realization::arc_batch=debug` and report this compiler error"
    )]
    DuplicateArcBody {
        body: String,
        first_parent: String,
        second_parent: String,
    },
    /// More than one realized impl body claimed the same user-drop operation.
    #[error("user-drop target resolution for type {ty:?} found {targets} callable bodies")]
    AmbiguousUserDropTarget {
        /// Canonical type carrying the user-drop burden.
        ty: Idx,
        /// Number of candidate qualified impl bodies.
        targets: usize,
    },
    /// A type declares a user-drop burden but no exact realized implementation
    /// body was bound before AIMS.
    #[error("user-drop target resolution for type {ty:?} found no callable body")]
    MissingUserDropTarget { ty: Idx },
    /// A typed impl role claimed user-drop semantics for a type whose burden
    /// has no such logical operation.
    #[error("user-drop impl role for type {ty:?} has no matching burden identity")]
    UnexpectedUserDropRole { ty: Idx },
    /// The typed impl role and burden registry disagree on logical identity.
    #[error(
        "user-drop impl role for type {ty:?} carries logical identity {found:?}, expected {expected:?}"
    )]
    UserDropLogicalIdentityMismatch {
        ty: Idx,
        expected: ori_registry::burden::FnSym,
        found: ori_registry::burden::FnSym,
    },
    /// ARC verification rejected post-AIMS IR.
    #[error("post-AIMS verification produced {count} error(s): {errors:?}")]
    ArcVerification {
        /// Number of verification failures.
        count: usize,
        /// Structured verifier failures.
        errors: Vec<ori_arc::verify::VerifyError>,
    },
    /// AIMS completed but reported semantic lowering problems.
    #[error("post-AIMS realization produced {count} problem(s): {problems:?}")]
    Aims {
        /// Number of AIMS problems.
        count: usize,
        /// Structured AIMS problems.
        problems: Vec<ori_arc::ArcProblem>,
    },
    /// The immutable string interner could not allocate the entry-point name.
    #[error(transparent)]
    Intern(#[from] ori_ir::InternError),
    /// Closed-program validation failed.
    #[error(transparent)]
    Executable(#[from] RealizationError),
}
