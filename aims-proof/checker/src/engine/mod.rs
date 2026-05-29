//! Engine-dispatch interfaces per
//! `the proof-checker design`
//! sec-Architecture-Sketch + sec-Engine-per-Category-Inventory.
//!
//! Eight engines bound to rule categories per the inventory body table:
//!
//! - `case_analysis` — finite case-analysis engine (CN / DP / TF case
//! enumeration); the foundational-axiom policy sec-Per-Engine-Constructive-Proof-Shape
//! designates this engine as the CONSTRUCTIVE replacement for classical
//! LEM-based case analysis.
//! - `refinement` — refinement-argument engine (RL post-condition =>
//! pre-condition implications).
//! - `rc_counting` — RC-preservation engine (RL-1..RL-21 +
//! RL-22..RL-26 RC-balance arguments).
//! - `lattice` — lattice-algebra engine (L-1..L-10 commutativity /
//! associativity / idempotence / finite-height).
//! - `monotonicity` — monotone-transfer-function engine (L-6 + TF rule
//! monotonicity).
//! - `fixpoint` — SCC fixpoint convergence engine (IC-7 + IA-7).
//! - `structural_induction` — pipeline-DAG induction engine
//! (PL-1..PL-11 ordering + VF-7/VF-8 layered composition).
//! - `interprocedural_summary` — interprocedural-contract-soundness
//! engine (IC-2/IC-3/IC-4/IC-5 SCC join + ContextBehavior +
//! EffectSummary derivation).
//!
//! Engines are first-class plugins to the dispatch loop. Adding a
//! category requires either an existing engine accommodating the new
//! proof shape OR a new engine module + dispatch registration. Adding
//! an engine without registering it in this module is a kernel
//! violation per the proof-checker design sec-Kernel-Verification-Methodology.
//!
//! Engines are stateless w.r.t. one another (per
//! the gate-vs-implementation split Option A — kernel +
//! engines composed at top-level, not via inter-engine calls).

pub mod bootstrap;
pub mod case_analysis;
pub mod lattice_algebra;
pub mod canonicalization;
pub mod transfer_functions;
pub mod decision_predicates;
pub mod interprocedural_contracts;
pub mod pipeline_ordering;
pub mod realization_rules;
pub mod verification_layers;
pub mod coexistence_handshake;
pub mod fixpoint;
pub mod interprocedural_summary;
pub mod lattice;
pub mod monotonicity;
pub mod rc_counting;
pub mod refinement;
pub mod structural_induction;

use crate::ast::{ProofStep, Theorem};

/// Engine trait — every engine in this module implements `dispatch`.
///
/// Engines accept a `Theorem` (or a `ProofStep` for composition
/// engines) and return an `EngineResult` carrying the verdict + any
/// downstream proof obligation surface.
///
/// Per the proof-checker design sec-Architecture-Sketch, the dispatch loop calls
/// `accepts(step)` to determine routing, then `discharge(step)` to
/// produce the verdict. Tools never `AskUserQuestion` per
/// the non-interactive runner contract.
pub trait Engine {
    /// Engine name — MUST match the canonical names in the proof-checker design
    /// sec-Engine-per-Category-Inventory (one of: `case_analysis`,
    /// `refinement`, `rc_counting`, `lattice`, `monotonicity`,
    /// `fixpoint`, `structural_induction`, `interprocedural_summary`).
    fn name(&self) -> &'static str;

    /// `accepts(step)` — return `true` when this engine recognizes the
    /// proof shape of the step.
    ///
    /// Each engine inspects the step's lattice-expression shape +
    /// category prefix to decide. Multiple engines may accept the same
    /// step in composition proofs; dispatch loop discharges through the
    /// declared engine sequence per the inventory category row.
    fn accepts(&self, step: &ProofStep) -> bool;

    /// `discharge(theorem)` — produce a verdict for the entire theorem.
    ///
    /// Returns `EngineResult` with `verdict` set per the
    /// `EngineVerdict` enum. Failures carry a structured `reason` field
    /// that routes through the §00 FAIL-branch enum
    /// (`checker_smoke_failed` with populated `failing_engine`).
    fn dispatch(&self, theorem: &Theorem) -> EngineResult;
}

/// Engine-level verdict carried out of `Engine::dispatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineResult {
    /// Verdict per the closed `EngineVerdict` enum.
    pub verdict: EngineVerdict,

    /// Structured human-readable diagnostic. Populated on
    /// `EngineVerdict::Fail` / `EngineVerdict::UnimplementedShape`;
    /// empty otherwise.
    pub reason: String,
}

/// Closed enum of engine-level verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineVerdict {
    /// Engine constructively discharged the obligation.
    Valid,

    /// Engine recognized the shape but the proof obligation was
    /// rejected (e.g. case-analysis enumeration uncovered an
    /// inconsistent branch).
    Fail,

    /// Engine cannot discharge this shape with its current
    /// implementation. Routes through `proof_checker_scope_inadequate`
    /// per §00 FAIL-branch enum UNLESS the plan explicitly narrows MS-3
    /// via a matching concession entry in
    /// the proof-checker design sec-Concession-entries.
    UnimplementedShape,
}

/// Instantiate an `Engine` impl by canonical name.
///
/// Returns `None` when `name` is not one of the 8 documented engines
/// (per the proof-checker design sec-Engine-per-Category-Inventory). The dispatch
/// loop in `crate::checker` treats an unknown engine name as a
/// manifest authoring error and surfaces it as a structured
/// diagnostic.
pub fn create_engine(name: &str) -> Option<Box<dyn Engine>> {
    match name {
        "case_analysis" => Some(Box::new(case_analysis::CaseAnalysisEngine)),
        "lattice" => Some(Box::new(lattice::LatticeEngine)),
        "monotonicity" => Some(Box::new(monotonicity::MonotonicityEngine)),
        "fixpoint" => Some(Box::new(fixpoint::FixpointEngine)),
        "structural_induction" => {
            Some(Box::new(structural_induction::StructuralInductionEngine))
        }
        "refinement" => Some(Box::new(refinement::RefinementEngine)),
        "rc_counting" => Some(Box::new(rc_counting::RcCountingEngine)),
        "interprocedural_summary" => {
            Some(Box::new(interprocedural_summary::InterproceduralSummaryEngine))
        }
        _ => None,
    }
}
