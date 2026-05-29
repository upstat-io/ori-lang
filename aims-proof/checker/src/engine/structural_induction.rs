//! Pipeline-DAG structural-induction engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **PL** (PL-1..PL-11): pipeline-DAG ordering proofs (PL-1..PL-6) +
//! TRMC rollback structural-equivalence (PL-7..PL-11) per
//! Annex E §AIMS sec7.
//! - **IA** (IA-1 bidirectional interaction + IA-5 step (1)
//! alias-transfer soundness): structural induction over instruction
//! lists.
//! - **VF** (VF-5 + VF-6 + VF-8): layered-verifier composition over
//! structural / contract / oracle / FIP layers.
//! - **CH** (sec11 coexistence handshake): structural induction over
//! sec02-sec09 outputs per the proof-checker design CH row.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: inductive-type elimination
//! on Ori-defined ASTs (instructions, blocks, pipeline DAG);
//! per-constructor case; explicit IH usage. FORBIDDEN absent extension:
//! strong induction without explicit measure; transfinite induction.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Pipeline-DAG structural-induction engine — discharges
/// PL-1..PL-11 + IA-1 + IA-5 + VF-5/6/8 + CH composition via
/// inductive-type elimination over Ori-defined ASTs.
pub struct StructuralInductionEngine;

impl Engine for StructuralInductionEngine {
    fn name(&self) -> &'static str {
        "structural_induction"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; per-constructor IH
        // predicates ship with the PL / IA / VF / CH discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::Pipeline
                | Category::IntraproceduralAnalysis
                | Category::VerificationLayer
                | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "structural_induction engine does not serve category {} for theorem {}",
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
        // §04.3 IA-5-step-1 gracious accept (PRIMARY engines for IA-5 are
        // monotonicity + case_analysis; SECONDARY accept pattern).
        if let Some(result) = super::section_04::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §07 pipeline-ordering discharge (PRIMARY engine for PL-1/PL-1a) per
        // Annex E §AIMS §6.
        if let Some(result) = super::section_07::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §09 verification-layer discharge (PRIMARY engine for VF-1/VF-2/VF-5/
        // VF-7/VF-comp structural-check + layered-stack soundness; SECONDARY
        // gracious-accept for VF-3/VF-4/VF-6/VF-8) per
        // Annex E §AIMS §9.
        if let Some(result) = super::section_09::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // sec-11 coexistence-handshake discharge (PRIMARY engine for CH-1 / CH-2 /
        // CH-4 / CH-comp lattice-bridge + single-elimination + AimsStateMap-
        // immutability + layered-handshake soundness; SECONDARY gracious-accept
        // for CH-3 / CH-5) per
        // the coexistence-handshake proofs.
        if let Some(result) = super::section_11::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "structural_induction engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
