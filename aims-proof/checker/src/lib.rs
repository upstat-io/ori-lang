//! aims-proof-checker — Ori-domain proof checker for the AIMS calculus.
//!
//! Mechanizes the soundness argument for AIMS rules cited in
//! `Annex E §AIMS` sec1-sec9, where the constitutional
//! mission per Annex E §AIMS ("RC rare, not RC ops
//! faster") demands every surviving RC op trace to a specific proof
//! failure.
//!
//! Top-level module layout per
//! `the proof-checker design`
//! sec-Architecture-Sketch:
//!
//! - `parser` — canonical-notation parser per the canonical proof notation.
//! - `ast` — theorem + proof AST + engine-annotation enum.
//! - `checker` — top-level orchestration + engine dispatch loop.
//! - `engine` — 8 engines: case_analysis, refinement, rc_counting,
//! lattice, monotonicity, fixpoint, structural_induction,
//! interprocedural_summary.
//! - `cli` — CLI entry (`check`, `coverage-corpus`, `smoke-test`
//! subcommands). Lean proofs are hand-authored at
//! `aims-proof/lean/AimsProof/*.lean` and cross-validated via the
//! dual-discharge gate (`aims-proof/scripts/dual-discharge.sh`); the
//! checker emits no Lean source.
//!
//! Foundational logic per the foundational-axiom policy is CONSTRUCTIVE. No LEM, no AC,
//! no functional/propositional extensionality, no proof irrelevance, no
//! Markov's Principle. Each engine's primitives enumerated in
//! the foundational-axiom policy sec-Per-Engine-Constructive-Proof-Shape.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod ast;
pub mod checker;
pub mod cli;
pub mod engine;
pub mod parser;

/// Shipped-lattice probe constants emitted by `build.rs` for §17
/// locality shipped-conformance. Consumed by the locality-conformance
/// binary at `src/bin/locality_conformance.rs` and by verdict-boundary
/// tests via mock-injection (the constants themselves are read-only).
#[allow(missing_docs)]
pub mod shipped_lattice_probe {
    include!(concat!(env!("OUT_DIR"), "/shipped_lattice_probe.rs"));
}

// Public re-exports for binary + integration-test consumers.
pub use ast::{
    AimsStateLiteral, ExpectedOutcome, LatticeExpr, Preconditions, ProofFile, ProofObligation,
    ProofStep, SoundnessProperty, Theorem,
};
pub use checker::{check_proof_file, CheckResult};
pub use engine::{Engine, EngineResult, EngineVerdict};
pub use parser::{parse_proof_file, ParseError};
