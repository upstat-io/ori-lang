//! Monotone-transfer-function engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **L** (L-6): transfer-function monotonicity
//! `a <= b => f(a) <= f(b)`.
//! - **TF** (TF-1..TF-15a): per-TF-rule monotonicity obligations —
//! co-dispatch with case_analysis + lattice per Annex E §AIMS sec3.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: per-pair (`a <= b`) finite
//! check of `f(a) <= f(b)`; constructive composition of monotone steps.
//! FORBIDDEN absent extension: LEM for arbitrary lattice elements.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Monotone-transfer-function engine — discharges L-6 + TF-rule
/// monotonicity via per-pair finite checks.
pub struct MonotonicityEngine;

impl Engine for MonotonicityEngine {
    fn name(&self) -> &'static str {
        "monotonicity"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; per-pair monotonicity
        // predicates ship with the L-6 + TF discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::Lattice | Category::TransferFunction | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "monotonicity engine does not serve category {} for theorem {}",
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
        // §02 lattice-algebra discharge (cross-dispatch acceptance) per
        // Annex E §AIMS §3.
        if let Some(result) = super::section_02::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §03 canonicalization-rule discharge (cross-dispatch acceptance) per
        // Annex E §AIMS §5.
        if let Some(result) = super::section_03::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §04 transfer-function discharge (PRIMARY engine for §04 L-6 layer b) per
        // Annex E §AIMS §4.
        if let Some(result) = super::section_04::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        if let Some(result) = super::section_11::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "monotonicity engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
