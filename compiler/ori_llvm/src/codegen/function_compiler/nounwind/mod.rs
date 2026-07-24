//! Two-pass nounwind analysis: prepare → analyze → emit.
//!
//! Sound nounwind analysis requires seeing all functions before emitting any
//! LLVM IR. The pipeline:
//! 1. **Prepare**: Lower all functions through the ARC pipeline, buffering
//!    the results as [`PreparedFunction`] / [`PreparedLambda`].
//! 2. **Analyze**: Fixed-point iteration over all prepared functions to build
//!    the complete nounwind set ([`FunctionCompiler::compute_nounwind_set`]).
//! 3. **Emit**: Emit LLVM IR using the complete nounwind set, ensuring
//!    callers of nounwind callees use `call` instead of `invoke`.
//!
//! # Known limitations
//!
//! Impl methods use [`FunctionCompiler::emit_arc_function`] before the two-pass
//! analysis. Calls from impl methods to monomorphized generic functions
//! therefore use the conservative `invoke` form even for nounwind callees.
//!
//! # Derived methods
//!
//! Open-world derived methods are emitted by `derive_codegen`; closed-executable
//! derived artifact bodies enter this two-pass pipeline. Both paths use
//! `DerivedTrait::is_nounwind_derived()` so Printable and Debug stay
//! conservatively may-unwind.
//!
//! # Submodules
//!
//! - [`types`] — `PreparedFunction` and `PreparedLambda` struct definitions
//! - [`prepare`] — ARC pipeline preparation (no LLVM emission)
//! - [`analyze`] — nounwind fixed-point + purity/readonly analysis
//! - [`emit`] — LLVM IR emission using the pre-computed nounwind set

mod analyze;
mod emit;
mod policy;
mod prepare;
mod types;

pub use types::{NounwindAnalyzedFunctions, PreparedFunction};

use policy::derived_artifact_allows_nounwind;
