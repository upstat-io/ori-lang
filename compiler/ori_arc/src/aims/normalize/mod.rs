//! AIMS Stage 3: normalization pass for TRMC opportunity creation.
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
//! # Scope (v1)
//!
//! Lifting verification + detection only — no IR rewriting. The lifting
//! pre-pass verifies A-normal form (guaranteed by `Construct.args:
//! Vec<ArcVarId>`). Detection produces structural metadata that enables:
//! - `ContextOpen`/`ContextClose` event recording in the state map
//! - Downstream passes to identify TRMC-eligible allocation sites
//!
//! The full TRMC 4-equation rewrite (Leijen & Lorenzen, JFP 2025) is
//! implemented in a subsequent stage.
//!
//! # References
//!
//! - Leijen & Lorenzen, "Tail Recursion Modulo Context" (JFP 2025)
//! - FP² (Lorenzen et al., ICFP 2023) — FIP/FBIP certification

mod detect;
mod lift;

#[cfg(test)]
mod tests;

pub(crate) use detect::collect_recursive_call_sites;

use super::contract::ContextRegion;
use crate::ir::ArcFunction;

/// Result of the normalization pass on a single function.
///
/// In v1 (detection only), `was_transformed` is always `false` — no IR
/// rewriting occurs. `context_regions` contains structural metadata for
/// TRMC candidates detected in the function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationResult {
    /// Whether the IR was rewritten (always `false` in v1).
    pub was_transformed: bool,
    /// Detected TRMC constructor-context regions.
    pub context_regions: Vec<ContextRegion>,
}

/// Run the normalization pass on a single function.
///
/// Detects TRMC candidates: `Construct` instructions where at least one
/// field argument is produced by a recursive call to the same function.
/// Returns [`NormalizationResult`] with context region metadata.
///
/// # When no candidates exist
///
/// Non-recursive functions or functions without constructor-context patterns
/// return `NormalizationResult { was_transformed: false, context_regions: [] }`.
pub fn normalize_function(func: &ArcFunction) -> NormalizationResult {
    // Verify A-normal form invariant before detection (invariant I4:
    // lifting precedes detection). No-op in ARC IR — type-enforced.
    lift::lift_constructor_args(func);

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
