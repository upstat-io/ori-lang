//! FIP contract verification.
//!
//! Cross-checks [`FipContract`] against [`FipEvidence`] and the function's
//! [`EffectSummary`] to catch mismatches between what interprocedural
//! analysis inferred and what realization actually achieved.
//!
//! # Pipeline position
//!
//! **Step 5a (per-function, pre-check):** Runs after `realize_rc_reuse()`
//! (step 5), before `verify_and_merge()`. At this point, `may_deallocate`
//! is still the optimistic default from interprocedural analysis.
//!
//! - **Structural violations** (`CertifiedButUnboundedStack`,
//!   `BoundedExceeded`) are genuine bugs — these facts are known from
//!   interprocedural analysis, not post-emission. Step 5a uses
//!   `debug_assert!` for these.
//! - **Post-emission mismatches** (`CertifiedButHasMissedReuses`) are
//!   *expected* at this stage (stale `may_deallocate`) and logged as
//!   `tracing::debug`. The second pass corrects these via recomputation.
//!
//! **Second pass (post-loop, authoritative):** After the per-function loop
//! completes, `run_aims_pipeline_all()` updates `may_deallocate` from
//! realization evidence and recomputes `contract.fip` via
//! [`recompute_fip_for_may_deallocate`]. A second FIP verification pass
//! then runs with the corrected contracts. Errors at this stage are genuine
//! bugs (the recomputation should have fixed any mismatches) and trigger
//! `debug_assert!`.
//!
//! # Relationship to `run_aims_verify()`
//!
//! The existing `run_aims_verify()` (step 7) checks structural consistency
//! (e.g., `AbsentParamHasUses`). This verifier checks semantic consistency:
//! does the FIP contract match the realization evidence? Both catch different
//! classes of bugs and run at different pipeline points.

#[cfg(test)]
mod tests;

use ori_ir::Name;

use crate::aims::contract::{FipContract, MemoryContract};
use crate::aims::realize::FipEvidence;

/// FIP verification error — contract vs realization mismatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FipVerificationError {
    /// Contract says `Certified` but realization has missed reuses
    /// (unmatched deallocations).
    CertifiedButHasMissedReuses { function: Name, missed_count: usize },
    /// Contract says `Certified` but `EffectSummary` reports unbounded stack.
    CertifiedButUnboundedStack { function: Name },
    /// Contract says `Bounded(n)` but actual net allocation exceeds `n`.
    BoundedExceeded {
        function: Name,
        declared: u16,
        actual: u16,
    },
}

impl std::fmt::Display for FipVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CertifiedButHasMissedReuses {
                function,
                missed_count,
            } => write!(
                f,
                "FIP Certified but {missed_count} missed reuse(s) in {function:?}"
            ),
            Self::CertifiedButUnboundedStack { function } => {
                write!(
                    f,
                    "FIP Certified but function has unbounded stack in {function:?}"
                )
            }
            Self::BoundedExceeded {
                function,
                declared,
                actual,
            } => write!(
                f,
                "FIP Bounded({declared}) but actual net allocation is {actual} in {function:?}"
            ),
        }
    }
}

/// Verify that a function's FIP contract is consistent with realization evidence.
///
/// Returns a list of verification errors (empty = verification passed).
///
/// # Checks
///
/// 1. **`Certified`**: allocation-balanced (`missed_reuses == 0`) and
///    `has_unbounded_stack == false`. Note: `may_allocate` may be `true`
///    for token-balanced functions (all allocations matched by reuses).
///    The stricter "no allocations" property is tracked by `is_fbip`.
/// 2. **`Bounded(n)`**: net allocation count (from evidence) <= n.
/// 3. **`Conditional`**: no additional checks (params are checked at call sites).
/// 4. **`Never`**: always passes (no FIP claim to verify).
pub fn verify_fip_contract(
    func_name: Name,
    contract: &MemoryContract,
    evidence: &FipEvidence,
) -> Vec<FipVerificationError> {
    let mut errors = Vec::new();

    match &contract.fip {
        FipContract::Certified => {
            verify_certified(func_name, contract, evidence, &mut errors);
        }
        FipContract::Bounded(n) => {
            verify_bounded(func_name, *n, evidence, &mut errors);
        }
        FipContract::Conditional { .. } | FipContract::Never => {
            // Conditional: params are checked at call sites, not here.
            // Never: no FIP claim to verify.
        }
    }

    errors
}

/// Verify a `Certified` FIP contract.
///
/// FIP Certified means "fully in-place" — all allocations are matched by
/// reuses (token-balanced). This is NOT the same as FBIP (allocation-free).
/// A Certified function MAY have `may_allocate == true` if all allocations
/// are balanced by consumed/dead values. The `is_fbip` field on
/// `MemoryContract` tracks the stricter "no allocations" property.
fn verify_certified(
    func_name: Name,
    contract: &MemoryContract,
    evidence: &FipEvidence,
    errors: &mut Vec<FipVerificationError>,
) {
    // FP² Theorem 2: no unmatched deallocation (all consumed values reused).
    if evidence.missed_reuses > 0 {
        errors.push(FipVerificationError::CertifiedButHasMissedReuses {
            function: func_name,
            missed_count: evidence.missed_reuses,
        });
    }

    // Section 12.2: constant stack.
    if contract.effects.has_unbounded_stack {
        errors.push(FipVerificationError::CertifiedButUnboundedStack {
            function: func_name,
        });
    }
}

/// Verify a `Bounded(n)` FIP contract.
fn verify_bounded(
    func_name: Name,
    declared: u16,
    evidence: &FipEvidence,
    errors: &mut Vec<FipVerificationError>,
) {
    // Net allocation = missed reuses (deallocations without matching allocations).
    // If missed_reuses > declared bound, the contract is violated.
    let actual = evidence.missed_reuses;
    if actual > declared as usize {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "missed_reuses capped at u16::MAX for error reporting"
        )]
        let actual_u16 = actual.min(u16::MAX as usize) as u16;
        errors.push(FipVerificationError::BoundedExceeded {
            function: func_name,
            declared,
            actual: actual_u16,
        });
    }
}

/// Recompute `contract.fip` after the post-emission `may_deallocate` update.
///
/// FP² Theorem 2 requires |S| = |S'| (no net allocation AND no net
/// deallocation) for FIP. If `may_deallocate` is `true` (from
/// `FipEvidence.missed_reuses > 0`), then any `Certified` or `Bounded(n)`
/// classification is unsound — the function has unmatched deallocations.
///
/// # Downgrade rules
///
/// - `Certified` + `may_deallocate` → `Never`
/// - `Bounded(n)` + `may_deallocate` → `Never`
/// - `Conditional` — unaffected (call-site uniqueness eliminates deallocation)
/// - `Never` — already bottom, no change
///
/// Returns `true` if the contract was downgraded.
pub fn recompute_fip_for_may_deallocate(contract: &mut MemoryContract) -> bool {
    if !contract.effects.may_deallocate {
        return false;
    }
    match contract.fip {
        FipContract::Certified | FipContract::Bounded(_) => {
            contract.fip = FipContract::Never;
            true
        }
        FipContract::Conditional { .. } | FipContract::Never => false,
    }
}
