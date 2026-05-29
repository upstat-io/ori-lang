//! Finite case-analysis engine.
//!
//! Per `the proof-checker design`
//! sec-Engine-per-Category-Inventory, this engine serves:
//!
//! - **CN** (CN-1..CN-8): bidirectional implications + ceiling rules;
//! finite enumeration of canonical-state grid per Appendix B
//! Infeasible State Table.
//! - **TF** (TF-1..TF-15a): per-instruction transfer rule case grid per
//! Appendix A Forward Transfer Matrix.
//! - **DP** (DP-1..DP-9): Boolean predicate truth tables per Appendix C;
//! DP-5 specifically consumes `borrow_sources` + `project_alias_sources`
//! + live-variable set as typed pre-pass per Annex E §AIMS sec1.9
//! side-table caveat.
//! - **IC** (IC-8a address-taken / closures with non-enumerable call
//! sites): case-grid component of the InterproceduralContract row.
//! - **IA** (IA-9 N-ary join permutation-invariance): reduces to L-1 +
//! L-2 via case_analysis enumeration.
//! - **PL**, **RL**, **CH**: per-class coexistence well-formedness +
//! per-step preconditions + per-instruction RC-delta case grid
//! (RL-14/14a/15/15a stack-promotion case grid).
//!
//! Constructive primitives consumed per the foundational-axiom policy
//! sec-Per-Engine-Constructive-Proof-Shape: finite enumeration of
//! canonical-state grid; per-branch constructive proof; closed-world
//! coverage check (every case exhibited). FORBIDDEN absent extension:
//! LEM for arbitrary phi; Classical.byCases; uncovered branches.

use crate::ast::{Category, ProofStep, Theorem};
use crate::engine::{Engine, EngineResult, EngineVerdict};

/// Finite case-analysis engine — primary CONSTRUCTIVE replacement for
/// classical LEM-based case analysis per the foundational-axiom policy.
pub struct CaseAnalysisEngine;

impl Engine for CaseAnalysisEngine {
    fn name(&self) -> &'static str {
        "case_analysis"
    }

    fn accepts(&self, _step: &ProofStep) -> bool {
        // Scaffold-time: every step routed by category in the manifest is
        // accepted. Per-shape accept predicates (Appendix A / B / C grid
        // matchers) ship in subsequent /fix-bug dispatches per
        // the proof-checker design sec-Kernel-Verification-Methodology.
        true
    }

    fn dispatch(&self, theorem: &Theorem) -> EngineResult {
        // Categories served per the proof-checker design sec-Engine-per-Category-Inventory.
        let served = matches!(
            theorem.id.category,
            Category::Canonicalization
                | Category::TransferFunction
                | Category::DecisionPredicate
                | Category::InterproceduralContract
                | Category::IntraproceduralAnalysis
                | Category::Pipeline
                | Category::Realization
                | Category::VerificationLayer
                | Category::CoexistenceHandshake
        );
        if !served {
            return EngineResult {
                verdict: EngineVerdict::UnimplementedShape,
                reason: format!(
                    "case_analysis engine does not serve category {} for theorem {}",
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
        // §03 canonicalization-rule discharge (PRIMARY engine for §03) per
        // Annex E §AIMS §5.
        if let Some(result) = super::canonicalization::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §04 transfer-function discharge (PRIMARY engine for §04 per-instruction
        // Appendix A enumeration) per
        // Annex E §AIMS §4.
        if let Some(result) = super::transfer_functions::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §05 decision-predicate discharge (PRIMARY engine for §05 Appendix C
        // truth-table enumeration) per
        // the decision-predicate proofs.
        if let Some(result) = super::decision_predicates::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §06 interprocedural-contract discharge (SECONDARY engine; gracious-
        // accept for IC-1/IC-2/IC-3 per coverage-manifest IC row) per
        // Annex E §AIMS §7.
        if let Some(result) = super::interprocedural_contracts::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §07 pipeline-ordering discharge (SECONDARY engine; gracious-accept
        // for PL-1/PL-1a per coverage-manifest PL row) per
        // Annex E §AIMS §6.
        if let Some(result) = super::pipeline_ordering::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // §08 realization-rule discharge (PRIMARY engine for COW / reuse /
        // stack-promotion / header-compression / non-atomic-RC / selective-
        // barriers / borrow-inference RL rules; SECONDARY gracious-accept for
        // RC-emission + LLVM-fact-export RL rules) per
        // Annex E §AIMS §8.
        if let Some(result) = super::realization_rules::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        // sec-11 coexistence-handshake discharge (PRIMARY engine for CH-3
        // per-class partition; SECONDARY gracious-accept for CH-1 / CH-2 / CH-4 /
        // CH-5 / CH-comp).
        if let Some(result) = super::coexistence_handshake::discharge_for_engine(self.name(), theorem) {
            return result;
        }
        EngineResult {
            verdict: EngineVerdict::UnimplementedShape,
            reason: format!(
                "case_analysis engine: scaffold-time stub for theorem {}; full implementation pending per the proof-checker design sec-Kernel-Verification-Methodology",
                theorem.id.canonical()
            ),
        }
    }
}
