//! Backend-agnostic codegen contract.
//!
//! [`CodegenBackend`] is the trait every codegen backend implements;
//! [`CodegenInput`] is the backend-agnostic bundle of inputs every backend
//! consumes. This crate owns only the input contract, the error type, and the
//! trait. It owns no semantic analysis, layout computation, target policy,
//! linker, or concrete backend, and it names no concrete backend type.
//!
//! # Contract scope
//!
//! [`CodegenInput`] currently carries the frontend inputs a backend receives
//! today. The backend-neutral executable artifact, the explicit target
//! description, and the matching compiled layout plan are not yet expressible:
//! the executable artifact is still constructed inside the backend call, and
//! neither a target-description type nor a compiled-layout-plan type exists in
//! the workspace. Widening this struct is gated on those types landing; until
//! then a field named for one of them would misname a transitional carrier.
//!
//! Artifact finalization (`finalize`, a typed artifact request, and a
//! backend-neutral emitted artifact) is likewise absent: today a backend's
//! `compile` returns its own artifact and the driver consumes it directly.

#![deny(unsafe_code)]

use ori_ir::canon::CanonResult;
use ori_repr::NarrowingPolicy;
use ori_types::{ExportedTypeMetadata, Pool, TypeCheckResult};

/// A codegen backend's failure to compile a [`CodegenInput`].
///
/// Backend-agnostic by construction (a `String` message, not a concrete
/// backend's error type) so an outside crate can implement
/// [`CodegenBackend`] without depending on any specific backend's error
/// representation.
#[derive(Debug, Clone)]
pub struct BackendError(pub String);

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BackendError {}

impl From<String> for BackendError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// The backend-agnostic input a [`CodegenBackend`] compiles.
///
/// Contains only inputs whose types belong to the backend-independent compiler
/// layers. Backend contexts, databases, and linkage state remain owned by each
/// concrete backend.
///
/// Two lifetimes: `'ctx` is the artifact's own lifetime (the pool must
/// outlive an LLVM `Module<'ctx>` built from it); `'p` is the call-scoped
/// lifetime of every other borrow, independent of `'ctx`.
#[derive(Clone, Copy)]
pub struct CodegenInput<'ctx, 'p> {
    /// Monomorphized type pool that outlives the compiled artifact.
    pub pool: &'ctx Pool,
    /// Type-checking facts for the module under compilation.
    pub type_result: &'p TypeCheckResult,
    /// Canonical expression arena and function bodies.
    pub canon: &'p CanonResult,
    /// Source path used for diagnostics and debug metadata.
    pub source_path: &'p str,
    /// Logical module name used for symbols and metadata.
    pub module_name: &'p str,
    /// Prefix applied to generated symbols.
    pub symbol_prefix: &'p str,
    /// Optional target triple override.
    pub target_triple: Option<&'p str>,
    /// Integer narrowing policy frozen before backend selection.
    pub narrowing_policy: NarrowingPolicy,
    /// Type metadata imported from dependency modules.
    pub imported_type_metadata: &'p [ExportedTypeMetadata],
    /// Collection surfaces imported from dependency modules.
    pub imported_collection_surfaces: &'p [u64],
}

/// A codegen backend: compiles a [`CodegenInput`] into a backend-owned
/// artifact.
///
/// `'ctx` is the artifact's own lifetime (see [`CodegenInput`]); `Artifact` is
/// the backend's own output type (an LLVM `Module<'ctx>`, native object file,
/// or bytecode chunk). Backend selection uses a closed enum because Ori does
/// not load out-of-tree code generators.
pub trait CodegenBackend<'ctx> {
    /// This backend's compiled output type.
    type Artifact;

    /// Compile `input` into this backend's artifact.
    fn compile<'p>(&self, input: &CodegenInput<'ctx, 'p>) -> Result<Self::Artifact, BackendError>;
}
