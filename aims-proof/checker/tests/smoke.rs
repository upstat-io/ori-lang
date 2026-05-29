//! Integration-test harness.
//!
//! Per `the design-validation gate`
//! Implementation Item 115 + 122:
//!
//! - One smoke test for the CN-1 bidirectional smoke proof at
//! `aims-proof/proofs/00-smoke-test/cn-1-bidirectional.proof`.
//! - One positive + one negative test per engine (8 engines * 2 = 16
//! minimum). Each engine's positive test asserts the engine accepts
//! theorems in a category it serves per the proof-checker design
//! sec-Engine-per-Category-Inventory; each negative test asserts the
//! engine rejects (with `does not serve category` diagnostic) for a
//! category it does NOT serve.
//!
//! At scaffold time the engines return `EngineVerdict::UnimplementedShape`
//! for valid categories (full per-shape discharge ships in subsequent
//! Agent dispatches); the engine smoke tests pin the dispatch contract
//! (category gate + scaffold-time reason text) so future engine work
//! does not regress the routing surface.

use aims_proof_checker::ast::{
    Category, ExpectedOutcome, Preconditions, ProofObligation, ProofStep, SoundnessProperty,
    Theorem, TheoremId,
};
use aims_proof_checker::checker::{check_proof_file, CheckResult};
use aims_proof_checker::engine::{
    case_analysis::CaseAnalysisEngine, fixpoint::FixpointEngine,
    interprocedural_summary::InterproceduralSummaryEngine, lattice::LatticeEngine,
    monotonicity::MonotonicityEngine, rc_counting::RcCountingEngine,
    refinement::RefinementEngine, structural_induction::StructuralInductionEngine, Engine,
    EngineVerdict,
};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR -> aims-proof/checker. One pop reaches aims-proof/.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p
}

/// Build a synthetic theorem for engine-contract testing.
fn synthetic_theorem(category: Category, suffix: &str, name: &str) -> Theorem {
    Theorem {
        id: TheoremId {
            category,
            suffix: suffix.to_string(),
        },
        name: name.to_string(),
        preconditions: Preconditions { items: vec![] },
        soundness: SoundnessProperty {
            source: String::new(),
        },
        obligation: ProofObligation::Steps(vec![ProofStep {
            source: String::new(),
        }]),
        expected: Some(ExpectedOutcome {
            status: "unimplemented_engine_shape".to_string(),
            reason: "synthetic scaffold-time fixture".to_string(),
        }),
    }
}

/// Assert that `engine` returns `UnimplementedShape` with a scaffold-time
/// reason for a theorem in a category it serves (positive test).
fn assert_engine_accepts_category(engine: &dyn Engine, category: Category, suffix: &str) {
    let theorem = synthetic_theorem(category, suffix, "positive fixture");
    let result = engine.dispatch(&theorem);
    assert_eq!(
        result.verdict,
        EngineVerdict::UnimplementedShape,
        "engine {} should return UnimplementedShape for served category {}",
        engine.name(),
        category.prefix()
    );
    assert!(
        result.reason.contains("scaffold-time stub"),
        "engine {} positive verdict for {}-{} should mention scaffold-time stub; got {:?}",
        engine.name(),
        category.prefix(),
        suffix,
        result.reason
    );
}

/// Assert that `engine` returns the `does not serve category` diagnostic
/// for a theorem in a category outside its inventory (negative test).
fn assert_engine_rejects_category(engine: &dyn Engine, category: Category, suffix: &str) {
    let theorem = synthetic_theorem(category, suffix, "negative fixture");
    let result = engine.dispatch(&theorem);
    assert_eq!(
        result.verdict,
        EngineVerdict::UnimplementedShape,
        "engine {} should still return UnimplementedShape on category mismatch",
        engine.name()
    );
    assert!(
        result.reason.contains("does not serve category"),
        "engine {} negative verdict for {}-{} should diagnose category mismatch; got {:?}",
        engine.name(),
        category.prefix(),
        suffix,
        result.reason
    );
}

// ---------------------------------------------------------------------------
// IT 115 — CN-1 bidirectional smoke
// ---------------------------------------------------------------------------

#[test]
fn cn_1_bidirectional_smoke() {
    // CN-1 ships via section_03 (canonicalization-rule discharge); the
    // smoke proof now discharges GREEN per proofs/00-smoke-test/
    // cn-1-bidirectional.expected = {"status": "valid"}.
    let path = workspace_root().join("proofs/00-smoke-test/cn-1-bidirectional.proof");
    let result = check_proof_file(&path);
    assert!(
        matches!(result, CheckResult::Valid),
        "expected Valid for CN-1 smoke; got {:?}",
        result
    );
    assert_eq!(result.exit_code(), 0);
    assert_eq!(result.exit_reason(), "smoke_passes_in_ori_checker");
}

// ---------------------------------------------------------------------------
// IT 122 — Per-engine positive + negative dispatch tests (8 engines)
// ---------------------------------------------------------------------------

#[test]
fn case_analysis_engine_smoke() {
    let engine = CaseAnalysisEngine;
    // Positive: case_analysis serves CN per the proof-checker design inventory. Suffix "99"
    // is unimplemented in section_03 so the scaffold-time contract still holds.
    assert_engine_accepts_category(&engine, Category::Canonicalization, "99");
    // Negative: case_analysis does NOT serve L.
    assert_engine_rejects_category(&engine, Category::Lattice, "1");
}

#[test]
fn refinement_engine_smoke() {
    let engine = RefinementEngine;
    // Positive: refinement serves RL (RL-29..31 LLVM fact export via section_08
    // PRIMARY verifiers; secondary gracious-accept on the other RL rules).
    // Suffix "99" is outside the section_08 roster so the scaffold-time
    // contract still holds.
    assert_engine_accepts_category(&engine, Category::Realization, "99");
    // Negative: refinement does NOT serve L.
    assert_engine_rejects_category(&engine, Category::Lattice, "1");
}

#[test]
fn rc_counting_engine_smoke() {
    let engine = RcCountingEngine;
    // Positive: rc_counting serves RL (RL-1..5 + RL-22..26 via section_08
    // PRIMARY verifiers; secondary gracious-accept on case-analysis /
    // refinement RL rules). Suffix "99" is outside the section_08 roster so
    // the scaffold-time contract still holds.
    assert_engine_accepts_category(&engine, Category::Realization, "99");
    // Negative: rc_counting does NOT serve L.
    assert_engine_rejects_category(&engine, Category::Lattice, "1");
}

#[test]
fn lattice_engine_smoke() {
    let engine = LatticeEngine;
    // Positive: lattice serves L (L-1..L-10) per inventory. Suffix "99" is
    // unimplemented in section_02 so the scaffold-time contract still holds.
    assert_engine_accepts_category(&engine, Category::Lattice, "99");
    // Negative: lattice does NOT serve IC.
    assert_engine_rejects_category(&engine, Category::InterproceduralContract, "1");
}

#[test]
fn monotonicity_engine_smoke() {
    let engine = MonotonicityEngine;
    // Positive: monotonicity serves L (L-6) per inventory. Suffix "99" is
    // unimplemented in section_02 so the scaffold-time contract still holds.
    assert_engine_accepts_category(&engine, Category::Lattice, "99");
    // Negative: monotonicity does NOT serve RL.
    assert_engine_rejects_category(&engine, Category::Realization, "1");
}

#[test]
fn fixpoint_engine_smoke() {
    let engine = FixpointEngine;
    // Positive: fixpoint serves IC (IC-6/IC-7/IC-8a/IC-8-REMOVED via §06.3
    // PRIMARY verifiers; secondary on IC-1..IC-5). Suffix "99" is outside
    // the section_06 roster so the scaffold-time contract still holds.
    assert_engine_accepts_category(&engine, Category::InterproceduralContract, "99");
    // Negative: fixpoint does NOT serve L.
    assert_engine_rejects_category(&engine, Category::Lattice, "1");
}

#[test]
fn structural_induction_engine_smoke() {
    let engine = StructuralInductionEngine;
    // Positive: structural_induction serves PL (PL-1..11) per inventory.
    // Suffix "99" is unimplemented in section_07 (§07.1 covers PL-1/PL-1a
    // only) so the scaffold-time stub contract still holds.
    assert_engine_accepts_category(&engine, Category::Pipeline, "99");
    // Negative: structural_induction does NOT serve L.
    assert_engine_rejects_category(&engine, Category::Lattice, "1");
}

#[test]
fn interprocedural_summary_engine_smoke() {
    let engine = InterproceduralSummaryEngine;
    // Positive: interprocedural_summary serves IC (IC-2..5) per inventory.
    // Suffix "99" is unimplemented in section_06 (§06.1 covers IC-1/IC-2/IC-3
    // only) so the scaffold-time contract still holds.
    assert_engine_accepts_category(&engine, Category::InterproceduralContract, "99");
    // Negative: interprocedural_summary does NOT serve L.
    assert_engine_rejects_category(&engine, Category::Lattice, "1");
}
