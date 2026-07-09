//! Codegen backend dispatch — the `BackendChoice` enum wrapping the LLVM
//! backend behind `ori_repr::CodegenBackend`.
//!
//! Single-variant today (`Llvm`); routes both `compile_common.rs` entry
//! points through `run_codegen_pipeline` unchanged. See
//! `.claude/rules/canon.md §1` (pipeline phase 8, sub-layer 7a `ori_repr`);
//! `.claude/rules/compiler.md §Dispatch` (enum for fixed sets).

#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;
#[cfg(feature = "llvm")]
use ori_llvm::monomorphize::ImportSig;
#[cfg(feature = "llvm")]
use ori_repr::{BackendError, CodegenBackend, RealizedProgram};
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::CompilerDb;

#[cfg(feature = "llvm")]
use super::codegen_pipeline::run_codegen_pipeline;
#[cfg(feature = "llvm")]
use super::ImportedSurfaces;

/// The LLVM codegen backend — wraps the existing `run_codegen_pipeline`
/// with the driver-side inputs `RealizedProgram` does not carry (the LLVM
/// `Context`, the Salsa `CompilerDb`, cross-module import linkage).
#[cfg(feature = "llvm")]
pub struct LlvmBackend<'ctx, 'a> {
    pub context: &'ctx Context,
    pub db: &'a CompilerDb,
    pub parse_result: &'a ParseOutput,
    pub import_sigs: &'a [ImportSig],
    pub imported: ImportedSurfaces<'a>,
}

#[cfg(feature = "llvm")]
impl<'ctx> CodegenBackend<'ctx> for LlvmBackend<'ctx, '_> {
    type Artifact = ori_llvm::inkwell::module::Module<'ctx>;

    fn compile<'p>(
        &self,
        program: &RealizedProgram<'ctx, 'p>,
    ) -> Result<Self::Artifact, BackendError> {
        run_codegen_pipeline(
            self.context,
            self.db,
            self.parse_result,
            program.type_result,
            program.pool,
            program.canon,
            program.source_path,
            program.module_name,
            program.symbol_prefix,
            self.import_sigs,
            self.imported,
            program.target_triple,
            program.narrowing_policy,
            program.imported_type_metadata,
            program.imported_collection_surfaces,
        )
        .map_err(BackendError::from)
    }
}

/// The fixed set of codegen backends `oric` may select. Single-variant
/// today per the walking-skeleton's scope decision; enum dispatch per
/// `.claude/rules/compiler.md §Dispatch`.
#[cfg(feature = "llvm")]
pub enum BackendChoice<'ctx, 'a> {
    Llvm(LlvmBackend<'ctx, 'a>),
}

#[cfg(feature = "llvm")]
impl<'ctx> BackendChoice<'ctx, '_> {
    pub fn compile<'p>(
        &self,
        program: &RealizedProgram<'ctx, 'p>,
    ) -> Result<ori_llvm::inkwell::module::Module<'ctx>, BackendError> {
        match self {
            BackendChoice::Llvm(backend) => backend.compile(program),
        }
    }
}
