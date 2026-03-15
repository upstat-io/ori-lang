//! AIMS Stage 3a: TRMC normalization pass.
//!
//! Detects self-recursive constructor functions (Tail Recursive Modulo
//! Constructor candidates) and produces [`ContextRegion`] metadata that
//! the intraprocedural analysis uses to record `ContextOpen`/`ContextClose`
//! events in the sparse event table.
//!
//! # Pipeline position
//!
//! Step 3a — between `compute_var_reprs()` (step 3) and `analyze_function()`
//! (step 4). The entry point is [`normalize_function`], which returns a
//! [`NormalizationResult`] containing context regions for the analysis.
//!
//! # Phases
//!
//! 1. **Lifting**: Verify A-normal form (no-op — type-enforced in ARC IR).
//! 2. **Detection**: Identify TRMC candidates (`ContextRegion` metadata).
//! 3. **Rewrite**: [`rewrite_trmc`] converts self-recursion into a loop
//!    with block params carrying the context. Uses [`LitValue::Null`] for
//!    hole field placeholders. Function signature is unchanged.
//! 4. **Verification**: [`verify::verify_trmc_rewrite`] checks structural
//!    correctness of the rewritten IR. On failure, the function is rolled
//!    back to its pre-rewrite state.
//!
//! The full rewrite pipeline (phases 1–4) is called from
//! [`normalize_function`] when context regions are detected and
//! the function has a converged contract from interprocedural analysis.
//!
//! # References
//!
//! - Leijen & Lorenzen, "Tail Recursion Modulo Context" (JFP 2025)
//! - FP² (Lorenzen et al., ICFP 2023) — FIP/FBIP certification

mod detect;
mod lift;
pub(crate) mod rewrite;
pub(crate) mod verify;

#[cfg(test)]
mod tests;

pub(crate) use detect::collect_recursive_call_sites;
pub(crate) use detect::detect_context_regions;

use super::contract::{ContextRegion, MemoryContract};
use crate::ir::ArcFunction;

/// Result of the normalization pass on a single function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationResult {
    /// Whether the IR was structurally rewritten (TRMC loop-header transform).
    ///
    /// `true` when the 4-equation rewrite was applied and passed
    /// post-rewrite verification. When `true`, the caller must re-run
    /// dependent pipeline steps (`var_reprs`, immortals, analysis).
    pub was_transformed: bool,
    /// Detected TRMC constructor-context regions.
    pub context_regions: Vec<ContextRegion>,
}

/// Run the normalization pass on a single function.
///
/// 1. **Lift**: Verify A-normal form (no-op in ARC IR — type-enforced).
/// 2. **Detect**: Identify TRMC candidates (`ContextRegion` metadata).
/// 3. **Rewrite + verify**: When candidates exist and a converged contract
///    is available, apply the 4-equation rewrite and verify.
///    On verification failure, roll back to the pre-rewrite state.
///
/// The `contract` parameter gates the rewrite: when `None` (e.g., first
/// SCC fixpoint iteration before contracts are computed), TRMC is
/// conservatively skipped. When `Some`, the rewrite proceeds — the sole
/// enforced soundness condition is per-variable uniqueness (checked in
/// `detect_trmc_candidates()` and `populate_context_events()` in
/// `intraprocedural/post_convergence.rs`).
///
/// # Effect purity gate (`may_share`)
///
/// The `may_share` flag is NOT used as a gate. The `HeapEscaping →
/// may_share` accumulation rule (Section 09.1) means every TRMC-eligible
/// function has `may_share == true` (returning a Construct triggers
/// `HeapEscaping`), so a `may_share` gate would block ALL candidates.
/// Per-variable uniqueness alone is sound because Ori has no effect
/// handlers — there is no mechanism for non-linear resumption that could
/// break the unique linear chain invariant (Lemma 2, Leijen & Lorenzen
/// JFP 2025). When effect handlers are implemented, an effect purity gate
/// must be added here.
pub fn normalize_function(
    func: &mut ArcFunction,
    contract: Option<&MemoryContract>,
) -> NormalizationResult {
    // Step 1: Verify A-normal form invariant (invariant I4: lifting
    // precedes detection). No-op in ARC IR — type-enforced.
    lift::lift_constructor_args(func);

    // Step 2: Detect TRMC candidates.
    let context_regions = detect::detect_context_regions(func);

    if !context_regions.is_empty() {
        tracing::debug!(
            func = ?func.name,
            regions = context_regions.len(),
            "TRMC context regions detected"
        );
    }

    // Step 3: Rewrite + verify (when candidates exist and a converged contract
    // is available). The sole enforced soundness condition is per-variable
    // uniqueness, checked in detect_trmc_candidates() (post_convergence.rs).
    // Effect purity gate (may_share): out of scope — Ori has no effect
    // handlers, so non-linear resumption cannot occur. This gate activates
    // when effect handlers are added (see normalize_function doc above).
    let mut was_transformed = false;
    if !context_regions.is_empty() {
        if contract.is_some() {
            was_transformed = rewrite_and_verify(func, &context_regions);
        } else {
            tracing::debug!(
                func = ?func.name,
                "TRMC rewrite skipped: no converged contract (first SCC iteration)"
            );
        }
    }

    NormalizationResult {
        was_transformed,
        context_regions,
    }
}

/// Attempt TRMC rewrite with post-rewrite verification and rollback.
///
/// Clones the function before rewriting so that verification failures
/// can be rolled back. Returns `true` if the rewrite was applied and
/// verified successfully.
///
/// # Rollback
///
/// If `verify_trmc_rewrite()` finds structural errors, the function is
/// restored to its pre-rewrite state (from the clone). The cost of the
/// clone is bounded: it's only taken when TRMC candidates exist (most
/// functions have zero candidates).
///
/// # Error handling
///
/// - Debug builds: `debug_assert!` on verification failure (catch bugs early).
/// - Release builds: `tracing::warn!` and skip the rewrite (graceful fallback).
pub(crate) fn rewrite_and_verify(func: &mut ArcFunction, regions: &[ContextRegion]) -> bool {
    if regions.is_empty() {
        return false;
    }

    let original = func.clone();
    let was_rewritten = rewrite::rewrite_trmc(func, regions);

    if !was_rewritten {
        return false;
    }

    let errors = verify::verify_trmc_rewrite(func, regions);
    if errors.is_empty() {
        tracing::debug!(
            func = ?func.name,
            "TRMC rewrite verified successfully"
        );
        return true;
    }

    // Verification failed — rollback.
    *func = original;

    for error in &errors {
        tracing::warn!("{error}");
    }

    debug_assert!(
        false,
        "TRMC verification failed with {} error(s): {:?}",
        errors.len(),
        errors
    );

    false
}
