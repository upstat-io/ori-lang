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
//! Impl methods are compiled via the old immediate-emit path
//! ([`FunctionCompiler::emit_arc_function`]) **before** the two-pass analysis
//! runs. This means impl methods calling monomorphized generic functions will
//! use `invoke` instead of `call`, even if the callee is trivially nounwind.
//! This is safe (using `invoke` is always correct) but generates unnecessary
//! overhead. A future refactor could fold impl methods into the two-pass batch.
//!
//! # NOTE: Derived methods handled separately (§02.3)
//!
//! Derived trait methods (`$eq`, `$compare`, `$hash`, `$clone`, `$default`)
//! are emitted by `derive_codegen` outside this two-pass pipeline. Pure
//! derives are marked `nounwind` directly in `setup_derive_function()` using
//! `DerivedTrait::is_nounwind_derived()`. Impure derives (`$to_str`, `$debug`)
//! are left unmarked.
//!
//! # Submodules
//!
//! - [`types`] — `PreparedFunction` and `PreparedLambda` struct definitions
//! - [`prepare`] — ARC pipeline preparation (no LLVM emission)
//! - [`analyze`] — nounwind fixed-point + purity/readonly analysis
//! - [`emit`] — LLVM IR emission using the pre-computed nounwind set

mod analyze;
mod emit;
mod prepare;
mod types;

pub use types::PreparedFunction;
