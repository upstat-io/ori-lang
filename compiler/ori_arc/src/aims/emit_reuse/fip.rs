//! FIP contract consultation during reuse emission.
//!
//! When a function's [`FipContract`] indicates FIP certification, reuse
//! emission can upgrade `MaybeShared` opportunities to static-unique
//! (skipping the `IsShared` runtime check). This is sound because FIP
//! certification guarantees the function operates allocation-free when
//! preconditions hold, implying all reusable values are unique.
//!
//! [`FipGateRecord`]s are emission-phase artifacts (NOT stored in the
//! `AimsStateMap`) tracking FIP-influenced reuse decisions. Consumed
//! by verification (Section 08).

use crate::aims::contract::FipContract;
use crate::ir::{ArcBlockId, ArcVarId};

use super::ReuseOpportunity;

/// A FIP gate record: a reuse decision influenced by FIP certification.
///
/// Produced during reuse emission (step 7) when the function's `FipContract`
/// influences whether a reuse opportunity uses static-unique or dynamic
/// emission. Consumed by verification (Section 08).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FipGateRecord {
    /// The source variable whose reuse was influenced by FIP.
    pub source_var: ArcVarId,
    /// Block where the FIP-guided reuse decision was made.
    pub block: ArcBlockId,
    /// The FIP-guided decision.
    pub decision: FipGateDecision,
}

/// The FIP-guided reuse decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FipGateDecision {
    /// FIP-certified: `MaybeShared` source upgraded to static-unique reuse
    /// (no `IsShared` check needed since FIP guarantees uniqueness).
    UpgradedToStaticUnique,
    /// FIP-conditional: standard dynamic reuse retained, but the call site
    /// satisfies the callee's unique-parameter preconditions.
    ConditionalDynamic,
}

/// Apply FIP contract upgrades to reuse opportunities.
///
/// - [`FipContract::Certified`]: upgrade `MaybeShared` → static-unique
///   (all code paths are allocation-free, so uniqueness is guaranteed).
/// - [`FipContract::Conditional`]: record gate but keep dynamic reuse
///   (the function may hit slow paths on non-unique inputs).
/// - [`FipContract::Never`] or absent: no change.
///
/// Returns the (possibly modified) opportunities and FIP gate records.
pub(super) fn apply_fip_upgrades(
    opportunities: Vec<ReuseOpportunity>,
    fip: Option<&FipContract>,
) -> (Vec<ReuseOpportunity>, Vec<FipGateRecord>) {
    let Some(fip) = fip else {
        return (opportunities, Vec::new());
    };

    match fip {
        FipContract::Never => (opportunities, Vec::new()),
        FipContract::Certified => apply_certified_upgrades(opportunities),
        FipContract::Conditional { .. } => apply_conditional_gates(opportunities),
    }
}

/// FIP-certified: upgrade all `MaybeShared` opportunities to static-unique.
fn apply_certified_upgrades(
    opportunities: Vec<ReuseOpportunity>,
) -> (Vec<ReuseOpportunity>, Vec<FipGateRecord>) {
    let mut gates = Vec::new();
    let upgraded: Vec<_> = opportunities
        .into_iter()
        .map(|mut opp| {
            if !opp.is_static_unique {
                tracing::debug!(
                    var = opp.source_var.raw(),
                    block = opp.source_block.raw(),
                    "FIP-certified: upgrading MaybeShared reuse to static-unique"
                );
                gates.push(FipGateRecord {
                    source_var: opp.source_var,
                    block: opp.source_block,
                    decision: FipGateDecision::UpgradedToStaticUnique,
                });
                opp.is_static_unique = true;
            }
            opp
        })
        .collect();
    (upgraded, gates)
}

/// FIP-conditional: record gates for `MaybeShared` but keep dynamic reuse.
fn apply_conditional_gates(
    opportunities: Vec<ReuseOpportunity>,
) -> (Vec<ReuseOpportunity>, Vec<FipGateRecord>) {
    let mut gates = Vec::new();
    for opp in &opportunities {
        if !opp.is_static_unique {
            tracing::debug!(
                var = opp.source_var.raw(),
                block = opp.source_block.raw(),
                "FIP-conditional: recording gate for dynamic reuse"
            );
            gates.push(FipGateRecord {
                source_var: opp.source_var,
                block: opp.source_block,
                decision: FipGateDecision::ConditionalDynamic,
            });
        }
    }
    (opportunities, gates)
}
