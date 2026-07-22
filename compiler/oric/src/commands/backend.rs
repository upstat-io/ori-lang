//! Codegen backend dispatch — the `CodegenBackendChoice` enum wrapping the
//! LLVM backend behind `ori_codegen::CodegenBackend`.
//!
//! Single-variant today (`Llvm`); routes both `compile_common.rs` entry
//! points through `run_codegen_pipeline` unchanged. The fixed backend set uses
//! enum dispatch; backend implementations share the `CodegenBackend` contract.

#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;

#[cfg(feature = "llvm")]
use ori_repr::monomorphize::ImportSig;

#[cfg(feature = "llvm")]
use ori_codegen::{BackendError, CodegenBackend, CodegenInput};

#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;

#[cfg(feature = "llvm")]
use oric::CompilerDb;

#[cfg(feature = "llvm")]
use super::codegen_pipeline::{run_codegen_pipeline, CodegenPipelineInput, LlvmCodegenOutput};

#[cfg(feature = "llvm")]
use super::ImportedSurfaces;

/// The LLVM codegen backend — wraps the existing `run_codegen_pipeline`
/// with the driver-side inputs `CodegenInput` does not carry (the LLVM
/// `Context`, the Salsa `CompilerDb`, cross-module import linkage).
#[cfg(feature = "llvm")]
pub(super) struct LlvmBackend<'ctx, 'a> {
    pub(super) context: &'ctx Context,
    pub(super) db: &'a CompilerDb,
    pub(super) parse_result: &'a ParseOutput,
    pub(super) import_sigs: &'a [ImportSig],
    pub(super) imported: ImportedSurfaces<'a>,
}

#[cfg(feature = "llvm")]
impl<'ctx> CodegenBackend<'ctx> for LlvmBackend<'ctx, '_> {
    type Artifact = LlvmCodegenOutput<'ctx>;

    fn compile<'p>(
        &self,
        program: &CodegenInput<'ctx, 'p>,
    ) -> Result<Self::Artifact, BackendError> {
        run_codegen_pipeline(CodegenPipelineInput {
            context: self.context,
            db: self.db,
            parse: self.parse_result,
            typed: program.type_result,
            pool: program.pool,
            canon: program.canon,
            source_path: program.source_path,
            module_name: program.module_name,
            symbol_prefix: program.symbol_prefix,
            import_sigs: self.import_sigs,
            imported: self.imported,
            target_triple: program.target_triple,
            realization_policy: oric::realization::RealizationPolicy::with_narrowing(
                program.narrowing_policy,
            ),
            imported_type_metadata: program.imported_type_metadata,
            imported_collection_surfaces: program.imported_collection_surfaces,
        })
        .map_err(BackendError::from)
    }
}

/// The fixed set of codegen backends `oric` may select.
///
/// This is single-variant today; enum dispatch keeps the closed set explicit.
#[cfg(feature = "llvm")]
pub(super) enum CodegenBackendChoice<'ctx, 'a> {
    Llvm(LlvmBackend<'ctx, 'a>),
}

#[cfg(feature = "llvm")]
impl<'ctx> CodegenBackendChoice<'ctx, '_> {
    pub(super) fn compile<'p>(
        &self,
        program: &CodegenInput<'ctx, 'p>,
    ) -> Result<LlvmCodegenOutput<'ctx>, BackendError> {
        match self {
            CodegenBackendChoice::Llvm(backend) => backend.compile(program),
        }
    }
}
