//! Logical ownership-event preservation engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **RL** (RL-1..RL-5 + RL-22..RL-26 + composition balance arguments):
//! per-event owner-credit arithmetic + balance-equation witness construction
//! per Annex E §AIMS sec8. Backend-neutral fact and projection rules use this
//! only as a secondary engine after their constructive primary discharge.
//! - **VF** (VF-4 FIP certification): proves
//! `FipContract::Certified` functions have zero unmatched
//! alloc/dealloc per Annex E §AIMS VF-4 + VF-6.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: structural induction on
//! instruction lists; per-event owner-credit arithmetic over integers;
//! balance-equation witness construction. FORBIDDEN absent extension:
//! Markov's Principle; classical existence of a balancing witness.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Logical ownership-event preservation engine. `rc_counting` is the historical
/// dispatcher key; the engine does not require a physical reference counter.
pub struct RcCountingEngine;

impl Engine for RcCountingEngine {
    fn name(&self) -> &'static str {
        "rc_counting"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; event-delta + balance
        // predicates ship with the RL-1..26 + VF-4 discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::Realization | Category::VerificationLayer | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "rc_counting engine does not serve category {} for theorem {}",
                    theorem.id.category.prefix(),
                    theorem.id.canonical()
                ),
            };
        }
        // §01A bootstrap-proof discharge per
        // the Lean 4 bootstrap proofs.
        if let Some(result) = super::bootstrap::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §08 realization-rule discharge (PRIMARY engine for RL-1..RL-5
        // ownership events + RL-22..RL-26 pair refinement + composition) per
        // Annex E §AIMS §8.
        if let Some(result) = super::realization_rules::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §09 verification-layer discharge (PRIMARY engine for VF-4 FIP
        // certification alloc-balance proof; SECONDARY gracious-accept for the
        // structural / oracle / coverage VF rules) per
        // Annex E §AIMS §9.
        if let Some(result) = super::verification_layers::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        if let Some(result) = super::coexistence_handshake::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "rc_counting engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
