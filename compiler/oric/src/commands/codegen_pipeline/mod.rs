//! AOT LLVM codegen orchestration.
//!
//! The pipeline lowers the complete ARC batch, projects callable exports, checks
//! PC-2 invariants, emits LLVM, and finalizes dumps and verification.

mod borrow_inference;
mod exports;
mod finalize;
pub(crate) mod imported_mono;
mod pc2_hooks;
mod pipeline;

pub(crate) use pipeline::RealizedCallableExport;
pub(super) use pipeline::{run_codegen_pipeline, CodegenPipelineInput, LlvmCodegenOutput};
