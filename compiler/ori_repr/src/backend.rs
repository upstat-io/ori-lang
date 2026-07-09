//! Backend-agnostic codegen dispatch boundary.
//!
//! [`CodegenBackend`] is the trait every codegen backend implements;
//! [`RealizedProgram`] is the backend-agnostic bundle of realized-IR inputs
//! every backend consumes. `ori_repr` depends on neither `ori_llvm` nor
//! `oric`, so this boundary cannot name a concrete backend type — see
//! `.claude/rules/canon.md §1` (pipeline phase 8, sub-layer 7a).

use ori_ir::canon::CanonResult;
use ori_types::{ExportedTypeMetadata, Pool, TypeCheckResult};

use crate::plan::NarrowingPolicy;

/// A codegen backend's failure to compile a [`RealizedProgram`].
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

/// The backend-agnostic subset of a realized program a [`CodegenBackend`]
/// compiles.
///
/// Bundles today's existing inputs to
/// `oric::commands::codegen_pipeline::run_codegen_pipeline` that are already
/// reachable from `ori_repr` (types from `ori_ir`, `ori_types`, and
/// `ori_repr` itself) — no field reshaping yet, per this section's own
/// scope decision. Backend-specific inputs the codegen pipeline also
/// consumes (the LLVM `Context`, the Salsa `CompilerDb`, cross-module
/// import linkage) are NOT bundled here — they stay on the concrete
/// backend's own construction, since `ori_repr` cannot name their types.
///
/// Two lifetimes: `'ctx` is the artifact's own lifetime (the pool must
/// outlive an LLVM `Module<'ctx>` built from it); `'p` is the call-scoped
/// lifetime of every other borrow, independent of `'ctx`.
#[derive(Clone, Copy)]
pub struct RealizedProgram<'ctx, 'p> {
    pub pool: &'ctx Pool,
    pub type_result: &'p TypeCheckResult,
    pub canon: &'p CanonResult,
    pub source_path: &'p str,
    pub module_name: &'p str,
    pub symbol_prefix: &'p str,
    pub target_triple: Option<&'p str>,
    pub narrowing_policy: NarrowingPolicy,
    pub imported_type_metadata: &'p [ExportedTypeMetadata],
    pub imported_collection_surfaces: &'p [u64],
}

/// A codegen backend: compiles a [`RealizedProgram`] into a backend-owned
/// artifact.
///
/// `'ctx` is the artifact's own lifetime (see [`RealizedProgram`]);
/// `Artifact` is the backend's own output type (an LLVM `Module<'ctx>`, a
/// native object file, a bytecode chunk — `ori_repr` never names it).
/// Enum dispatch over a fixed backend set (per `.claude/rules/compiler.md
/// §Dispatch`), not `dyn Trait` — Ori has no out-of-tree backend loading
/// requirement `rustc`'s `dyn CodegenBackend` solves for.
pub trait CodegenBackend<'ctx> {
    /// This backend's compiled output type.
    type Artifact;

    /// Compile `program` into this backend's artifact.
    fn compile<'p>(
        &self,
        program: &RealizedProgram<'ctx, 'p>,
    ) -> Result<Self::Artifact, BackendError>;
}
