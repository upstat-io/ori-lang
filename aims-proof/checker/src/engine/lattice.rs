//! Lattice-algebra engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **L** (L-1..L-10): algebraic identities; commutativity /
//! associativity / idempotence; partial-order reflexivity /
//! antisymmetry / transitivity; finite-height per dimension +
//! sum-product height for the product lattice.
//! - **CN** (CN-1..CN-8): co-dispatch with case_analysis — lattice
//! discharges L-7 idempotence + L-8 join-preservation obligations.
//! - **TF** (TF-1..TF-15a): co-dispatch with case_analysis +
//! monotonicity — lattice discharges canonicalization commutativity
//! per Annex E §AIMS sec2.
//! - **CH** (sec11 coexistence handshake): co-dispatch — lattice
//! discharges the sec02 lattice transfer composition per the proof-checker design
//! CH row.
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: per-dimension finite
//! enumeration; explicit join-table verification; structural-equality on
//! dimension values. FORBIDDEN absent extension: functional
//! extensionality on the join operator; classical antisymmetry.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Lattice-algebra engine — discharges L-1..L-10 + canonicalization
/// commutativity via per-dimension finite enumeration + explicit
/// join-table verification.
pub struct LatticeEngine;

impl Engine for LatticeEngine {
    fn name(&self) -> &'static str {
        "lattice"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: dispatcher gates by category; per-shape predicates
        // ship with the L / CN-7 / CN-8 / TF / CH discharge implementations.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        let served = matches!(
            theorem.id.category,
            Category::Lattice
                | Category::Canonicalization
                | Category::TransferFunction
                | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "lattice engine does not serve category {} for theorem {}",
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
        // §02 lattice-algebra discharge per
        // Annex E §AIMS §3.
        if let Some(result) = super::section_02::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §03 canonicalization-rule discharge per
        // Annex E §AIMS §5.
        if let Some(result) = super::section_03::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §04 transfer-function discharge (SECONDARY engine for §04 — accepts
        // gracefully; primary engines monotonicity + case_analysis discharge) per
        // Annex E §AIMS §4.
        if let Some(result) = super::section_04::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // sec-11 coexistence-handshake discharge (SECONDARY engine — accepts
        // gracefully on every CH; primary engines structural_induction /
        // case_analysis / interprocedural_summary discharge per the
        // sec-11.0 Per-CH Proof-Status Tracking table).
        if let Some(result) = super::section_11::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "lattice engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
