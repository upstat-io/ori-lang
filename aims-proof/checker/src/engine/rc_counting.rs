//! RC-preservation engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **RL** (RL-1..RL-21 + RL-22..RL-26 RC-balance arguments):
//! per-instruction RC-delta arithmetic + balance-equation witness
//! construction per Annex E §AIMS sec8.
//! - **VF** (VF-4 FIP certification): proves
//! `FipContract::Certified` functions have zero unmatched
//! alloc/dealloc per Annex E §AIMS VF-4 + VF-6.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: structural induction on
//! instruction lists; per-instruction RC-delta arithmetic over integers;
//! balance-equation witness construction. FORBIDDEN absent extension:
//! Markov's Principle; classical existence of a balancing witness.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// RC-preservation engine — discharges per-instruction RC-balance
/// arguments via structural induction on instruction lists.
pub struct RcCountingEngine;

impl Engine for RcCountingEngine {
    fn name(&self) -> &'static str {
        "rc_counting"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; RC-delta + balance
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
        // §08 realization-rule discharge (PRIMARY engine for RL-1..RL-5 RC
        // emission + RL-22..RL-26 KnownSafe/PRE motion + composition) per
        // Annex E §AIMS §8.
        if let Some(result) = super::section_08::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §09 verification-layer discharge (PRIMARY engine for VF-4 FIP
        // certification alloc-balance proof; SECONDARY gracious-accept for the
        // structural / oracle / coverage VF rules) per
        // Annex E §AIMS §9.
        if let Some(result) = super::section_09::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        if let Some(result) = super::section_11::discharge_for_engine(self.name(), theorem) {
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
