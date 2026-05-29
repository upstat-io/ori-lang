//! SCC fixpoint convergence engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **IC** (IC-7): SCC fixpoint convergence — iteration-bound argument
//! `param_count * 13 + 8 + 6 + 4` per Annex E §AIMS sec5 IC-7.
//! - **IA** (IA-7): backward-dataflow convergence — chain-height bound
//! `CHAIN_HEIGHT * |variables| * |blocks|` (CHAIN_HEIGHT = 16) per
//! Annex E §AIMS sec6 IA-7.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: bounded iteration with
//! explicit termination witness (IC-7 / IA-7 bounds); per-iteration
//! progress measure; constructive convergence proof. FORBIDDEN absent
//! extension: classical well-foundedness; Konig's lemma.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// SCC fixpoint convergence engine — discharges IC-7 + IA-7 convergence
/// bounds via bounded iteration with explicit termination witness.
pub struct FixpointEngine;

impl Engine for FixpointEngine {
    fn name(&self) -> &'static str {
        "fixpoint"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; bounded-iteration
        // shape predicates ship with the IC-7 + IA-7 discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::InterproceduralContract | Category::IntraproceduralAnalysis | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "fixpoint engine does not serve category {} for theorem {}",
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
        // §03 canonicalization-rule discharge (cross-dispatch acceptance) per
        // Annex E §AIMS §5.
        if let Some(result) = super::section_03::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §04.3 IA-5-step-1 gracious accept (PRIMARY engines for IA-5 are
        // monotonicity + case_analysis; SECONDARY accept pattern).
        if let Some(result) = super::section_04::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §06 interprocedural-contract discharge (SECONDARY engine; gracious-
        // accept for IC-1/IC-2/IC-3 per coverage-manifest IC row) per
        // Annex E §AIMS §7.
        if let Some(result) = super::section_06::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        if let Some(result) = super::section_11::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "fixpoint engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
