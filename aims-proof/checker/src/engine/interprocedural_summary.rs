//! Interprocedural-contract-soundness engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **IC** (IC-2 / IC-3 / IC-4 / IC-5 SCC join + ContextBehavior +
//! EffectSummary derivation): interprocedural contract correctness per
//! Annex E §AIMS sec5.
//! - **IA** (IA-5 step (1) alias-transfer via `seq_add`): co-dispatch
//! with structural_induction + case_analysis.
//! - **PL** (PL-11): `ContextBehavior` initialization + join per
//! Annex E §AIMS sec7.
//! - **VF** (VF-3 oracle cross-check): re-derives `MemoryContract` from
//! realized IR + compares against inferred contract.
//! - **RL** (RL-31 cross-function provenance disjointness): co-dispatch
//! per Annex E §AIMS sec8 RL-31.
//! - **CH** (sec11 coexistence handshake): burden-registry-as-typed
//! pre-pass + DP-2/DP-3 elimination contract per the proof-checker design CH row.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: SCC topological order
//! witness; per-node finite enumeration over contract dimensions;
//! constructive contract-join derivation. FORBIDDEN absent extension:
//! classical choice over unenumerable call-site sets (IC-8a
//! CONSERVATIVE initialization is the constructive cure).

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Interprocedural-contract-soundness engine — discharges
/// IC-2..IC-5 + IA-5 + PL-11 + VF-3 + RL-31 + CH composition via SCC
/// topology + per-node finite enumeration.
pub struct InterproceduralSummaryEngine;

impl Engine for InterproceduralSummaryEngine {
    fn name(&self) -> &'static str {
        "interprocedural_summary"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; SCC-topology + contract
        // predicates ship with the IC / VF-3 / RL-31 / CH discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::InterproceduralContract
                | Category::VerificationLayer
                | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "interprocedural_summary engine does not serve category {} for theorem {}",
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
        // §06 interprocedural-contract discharge (PRIMARY engine for §06.1
        // IC-1/IC-2/IC-3) per
        // Annex E §AIMS §7.
        if let Some(result) = super::interprocedural_contracts::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §09 verification-layer discharge (PRIMARY engine for VF-8 stack-
        // applies-to-all-rules coverage check; SECONDARY gracious-accept for the
        // structural / oracle / FIP VF rules) per
        // Annex E §AIMS §9.
        if let Some(result) = super::verification_layers::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // sec-11 coexistence-handshake discharge (PRIMARY engine for CH-5
        // phase-ordering composition; SECONDARY gracious-accept for CH-1 / CH-2 /
        // CH-3 / CH-4 / CH-comp).
        if let Some(result) = super::coexistence_handshake::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "interprocedural_summary engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
