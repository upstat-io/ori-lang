//! Theorem + proof AST consumed by the dispatch loop in
//! `crate::checker` and the per-engine dispatchers in `crate::engine`.
//!
//! Type shapes mirror the canonical proof notation theorem-block
//! grammar:
//!
//! ```text
//! Theorem <CATEGORY>-<N>: <human-readable name>
//! Preconditions:
//! - <p1>
//! - <p2>
//! Soundness property:
//! forall <quantifier> in <domain>. <invariant>
//! Proof obligation:
//! <constructive proof sketch in canonical notation,
//! OR `sorry` placeholder per sec01 skeleton authoring>
//! ```
//!
//! Theorem identifiers MUST carry the category prefix from
//! `Annex E §AIMS` (L / CN / TF / DP / IC / IA / PL / RL / VF) plus the
//! CH category for sec11 composition theorems per
//! the proof-checker design sec-Engine-per-Category-Inventory.

use std::path::PathBuf;

/// Top-level container for a parsed proof file. One file may carry
/// multiple theorem blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofFile {
    /// Absolute path of the file on disk (audit-trail field; never
    /// consumed by the dispatch logic itself).
    pub source_path: PathBuf,

    /// Ordered list of theorem blocks in the file.
    pub theorems: Vec<Theorem>,
}

/// Single theorem block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theorem {
    /// Theorem identifier, e.g. "L-1", "CN-6", "CH-§11" — MUST carry a
    /// category prefix matching the proof-checker design
    /// sec-Engine-per-Category-Inventory.
    pub id: TheoremId,

    /// Human-readable theorem name.
    pub name: String,

    /// Preconditions block (bulleted list of formal claims the theorem
    /// rests on).
    pub preconditions: Preconditions,

    /// Soundness property: the universally-quantified invariant the
    /// proof discharges.
    pub soundness: SoundnessProperty,

    /// Proof obligation: either a constructive proof sketch or a
    /// `sorry` placeholder per the canonical proof notation sec-Theorem-Statement-Format.
    pub obligation: ProofObligation,

    /// Author-declared `Expected:` block per the canonical proof notation
    /// sec-Theorem-Statement-Format. Captures the scaffold-time
    /// outcome the dispatch loop should produce so the §00 PASS-gate
    /// .expected golden files match without the engines having
    /// per-shape reason text baked into them. `None` when the proof
    /// file omits the `Expected:` block (engines fall back to a
    /// category-derived default reason).
    pub expected: Option<ExpectedOutcome>,
}

/// Captured `Expected:` block from a theorem's source.
///
/// Carried verbatim from the proof file so the dispatch loop can echo
/// the author-declared `reason` string into the JSON output. The
/// `status` field maps onto the §00 PASS-gate verdict enum:
/// `valid` / `fail` / `unimplemented_engine_shape`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedOutcome {
    /// Verbatim status string (e.g. `"unimplemented_engine_shape"`).
    pub status: String,

    /// Verbatim reason string (e.g. `"L-shape scaffold-time; engine implementation pending"`).
    pub reason: String,
}

/// Theorem category + numeric suffix.
///
/// `category` maps directly to the proof-checker design
/// sec-Engine-per-Category-Inventory rows; the dispatch loop selects
/// engines per category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremId {
    /// Category prefix: `L`, `CN`, `TF`, `DP`, `IC`, `IA`, `PL`, `RL`,
    /// `VF`, or `CH`.
    pub category: Category,

    /// Numeric suffix (e.g. `1` in `L-1`, `15a` in `TF-15a`). Free-form
    /// to accommodate letter-suffix variants.
    pub suffix: String,
}

/// Closed enum of theorem categories per
/// `Annex E §AIMS` sec1-sec9 + the proof-checker design CH row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Lattice algebra (L-1..L-10).
    Lattice,
    /// Canonicalization (CN-1..CN-8).
    Canonicalization,
    /// Transfer functions (TF-1..TF-15a).
    TransferFunction,
    /// Decision predicates (DP-1..DP-9).
    DecisionPredicate,
    /// Interprocedural contracts (IC-1..IC-7, IC-8a).
    InterproceduralContract,
    /// Intraprocedural analysis (IA-1..IA-9).
    IntraproceduralAnalysis,
    /// Pipeline ordering + TRMC (PL-1..PL-11).
    Pipeline,
    /// Realization + post-pipeline (RL-1..RL-34).
    Realization,
    /// Verification layers (VF-1..VF-8).
    VerificationLayer,
    /// Coexistence handshake (CH) — sec11 composition theorems per
    /// the proof-checker design sec-Engine-per-Category-Inventory CH row.
    CoexistenceHandshake,
}

/// Preconditions block: bulleted formal claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preconditions {
    /// Each entry is one bullet from the `Preconditions:` block.
    pub items: Vec<String>,
}

/// Soundness property — universally-quantified invariant the proof
/// discharges.
///
/// Carries the canonical-notation quantifier prefix (`forall`,
/// `exists`) + domain + body. Body is a lattice expression authored in
/// the canonical proof notation canonical notation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundnessProperty {
    /// Verbatim canonical-notation source for the property body. The
    /// dispatch loop parses this into `LatticeExpr` instances via
    /// per-engine accept-predicates.
    pub source: String,
}

/// Proof obligation: either constructive proof body or `sorry`
/// placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
    /// Constructive proof sketch — list of proof steps the dispatch
    /// loop discharges per engine.
    Steps(Vec<ProofStep>),

    /// `sorry` placeholder per the canonical proof notation sec-Theorem-Statement-Format
    /// (skeleton authoring at sec01 mathematical foundation; discharged
    /// in sec02-sec09 + sec11 proof corpora).
    Sorry,
}

/// Individual proof step — atomic unit the dispatch loop routes to one
/// engine via the engine `accepts(step)` predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofStep {
    /// Verbatim canonical-notation source for this step (audit trail +
    /// engine-side re-parsing).
    pub source: String,
}

/// AimsState tuple literal per the canonical proof notation sec-AimsState-Representation.
///
/// Canonical form: parenthesized, comma-separated dimension values in
/// fixed order (Access -> Effect). E.g.
/// `(O, L, 1, u, bl, R<S>, {alloc})`.
///
/// Per the canonical proof notation sec-Notation-Evolution-Discipline, dimensions are
/// added at the END of the tuple; existing positions remain stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AimsStateLiteral {
    /// Ordered list of per-dimension values, each captured as the raw
    /// canonical-notation token. Engines re-parse to their typed
    /// per-dimension representations.
    pub dimensions: Vec<String>,
}

/// Lattice expression — the body of a `Soundness property` or
/// `Proof obligation` step.
///
/// Modelled as a discriminated union over the lattice operators in
/// the canonical proof notation sec-Lattice-Operators. Variants currently scaffolded
/// for the operators consumed by sec02 lattice algebra proofs +
/// sec03-sec09 successors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatticeExpr {
    /// Variable / identifier reference.
    Var(String),

    /// AimsState tuple literal.
    State(AimsStateLiteral),

    /// `a join b` — componentwise lattice join (per the canonical proof notation
    /// sec-Lattice-Operators).
    Join(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `a meet b` — componentwise lattice meet.
    Meet(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `a <= b` — lattice partial order.
    LessEqual(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `a = b` — definitional / structural equality.
    Equal(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `forall x in D. body` — universal quantification.
    Forall {
        /// Quantifier variable (bound name).
        var: String,
        /// Domain symbol (e.g. `AimsState`, `Functions`).
        domain: String,
        /// Body expression.
        body: Box<LatticeExpr>,
    },

    /// `phi /\ psi` — logical conjunction.
    And(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `phi \/ psi` — logical disjunction.
    Or(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `phi => psi` — material implication.
    Implies(Box<LatticeExpr>, Box<LatticeExpr>),

    /// `phi <=> psi` — biconditional (DP-1..DP-9 truth tables;
    /// CN-1 bidirectional).
    Biconditional(Box<LatticeExpr>, Box<LatticeExpr>),

    /// Opaque placeholder for canonical-notation surface this scaffold
    /// has not yet structured. Engines route opaque expressions through
    /// the case_analysis engine's finite-enumeration shape per
    /// the foundational-axiom policy sec-Per-Engine-Constructive-Proof-Shape.
    Opaque(String),
}

impl TheoremId {
    /// Canonical `<PREFIX>-<SUFFIX>` rendering used by the dispatch
    /// loop + JSON output (e.g. `L-1`, `TF-15a`, `CH-11`).
    pub fn canonical(&self) -> String {
        format!("{}-{}", self.category.prefix(), self.suffix)
    }
}

impl Category {
    /// All categories per the proof-checker design sec-Engine-per-Category-Inventory.
    /// Used by coverage-manifest validators + cli `coverage-corpus`.
    pub fn all() -> &'static [Category] {
        &[
            Category::Lattice,
            Category::Canonicalization,
            Category::TransferFunction,
            Category::DecisionPredicate,
            Category::InterproceduralContract,
            Category::IntraproceduralAnalysis,
            Category::Pipeline,
            Category::Realization,
            Category::VerificationLayer,
            Category::CoexistenceHandshake,
        ]
    }

    /// Canonical short prefix used in theorem identifiers per
    /// the canonical proof notation sec-Theorem-Statement-Format.
    pub fn prefix(&self) -> &'static str {
        match self {
            Category::Lattice => "L",
            Category::Canonicalization => "CN",
            Category::TransferFunction => "TF",
            Category::DecisionPredicate => "DP",
            Category::InterproceduralContract => "IC",
            Category::IntraproceduralAnalysis => "IA",
            Category::Pipeline => "PL",
            Category::Realization => "RL",
            Category::VerificationLayer => "VF",
            Category::CoexistenceHandshake => "CH",
        }
    }
}
