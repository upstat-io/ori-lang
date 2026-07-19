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
//! # Derived methods
//!
//! Legacy derived methods are emitted by `derive_codegen`; closed-executable
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
mod prepare;
mod types;

pub use types::PreparedFunction;

fn derived_artifact_allows_nounwind(name: &str) -> bool {
    ori_ir::DerivedTrait::from_executable_body_name(name)
        .is_none_or(|(trait_kind, _)| trait_kind.is_nounwind_derived())
}

#[cfg(test)]
mod tests {
    use super::derived_artifact_allows_nounwind;

    #[test]
    fn derived_artifact_nounwind_policy_uses_trait_metadata() {
        assert!(derived_artifact_allows_nounwind("eq$derived$0"));
        assert!(!derived_artifact_allows_nounwind("to_str$derived$1"));
        assert!(!derived_artifact_allows_nounwind("debug$derived$2"));
        assert!(derived_artifact_allows_nounwind("ordinary_function"));
    }
}
