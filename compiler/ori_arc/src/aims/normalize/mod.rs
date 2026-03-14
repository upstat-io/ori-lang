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
//! 3. **Rewrite** (Section 13.6): [`rewrite_trmc`] converts self-recursion
//!    into a loop with block params carrying the context. Uses
//!    [`LitValue::Null`] for hole field placeholders. Function signature
//!    is unchanged. Not yet wired into the live pipeline — requires
//!    contract recomputation and the `may_share` false-positive resolution
//!    from Section 13.6.
//!
//! # References
//!
//! - Leijen & Lorenzen, "Tail Recursion Modulo Context" (JFP 2025)
//! - FP² (Lorenzen et al., ICFP 2023) — FIP/FBIP certification

mod detect;
mod lift;
pub(crate) mod rewrite;

#[cfg(test)]
mod tests;

pub(crate) use detect::collect_recursive_call_sites;

use super::contract::ContextRegion;
use crate::ir::ArcFunction;

/// Result of the normalization pass on a single function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationResult {
    /// Whether the IR was structurally rewritten (TRMC loop-header transform).
    ///
    /// Currently always `false` in the live pipeline — the rewrite is
    /// implemented and tested but not yet wired in (Section 13.6).
    pub was_transformed: bool,
    /// Detected TRMC constructor-context regions.
    pub context_regions: Vec<ContextRegion>,
}

/// Run the normalization pass on a single function (detection only).
///
/// Detects TRMC candidates: `Construct` instructions where at least one
/// field argument is produced by a recursive call to the same function.
/// Returns [`NormalizationResult`] with context region metadata.
///
/// The TRMC rewrite ([`rewrite::rewrite_trmc`]) is implemented and tested
/// but not called from this function — it requires contract recomputation
/// and the `may_share` false-positive resolution from Section 13.6.
pub fn normalize_function(func: &ArcFunction) -> NormalizationResult {
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

    NormalizationResult {
        was_transformed: false,
        context_regions,
    }
}
