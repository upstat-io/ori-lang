//! Refinement-argument engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **RL** (RL-29 / RL-30 / RL-31 LLVM-fact-export soundness):
//! refinement of CONSERVATIVE state by callee `ReturnContract` per
//! Annex E §AIMS TF-6 `refine()`.
//! - **VF** (VF-2 contract-consistency): contract-vs-realization
//! refinement check.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: definitional rewriting;
//! structural congruence; constructive `->`-introduction. FORBIDDEN
//! absent extension: proof irrelevance; propositional extensionality.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Refinement-argument engine — discharges post-condition =>
/// pre-condition implications via definitional rewriting.
pub struct RefinementEngine;

impl Engine for RefinementEngine {
    fn name(&self) -> &'static str {
        "refinement"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; definitional-rewrite
        // predicates ship with the RL-29..31 + VF-2 discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::TransferFunction
                | Category::Realization
                | Category::VerificationLayer
                | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "refinement engine does not serve category {} for theorem {}",
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
        // §04 transfer-function discharge (PRIMARY engine for TF-6 / TF-6a
        // refine(CONSERVATIVE, contract); SECONDARY graceful-accept for other
        // §04.1 theorems) per
        // Annex E §AIMS §4.
        if let Some(result) = super::transfer_functions::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §08 realization-rule discharge (PRIMARY engine for RL-29/RL-30/RL-31
        // LLVM fact export; SECONDARY gracious-accept for RC-emission +
        // case-analysis RL rules) per
        // Annex E §AIMS §8.
        if let Some(result) = super::realization_rules::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §09 verification-layer discharge (PRIMARY engine for VF-3 oracle
        // re-derivation + VF-6 contracts↔realization agreement; SECONDARY
        // gracious-accept for the structural / FIP / coverage VF rules) per
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
                "refinement engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
