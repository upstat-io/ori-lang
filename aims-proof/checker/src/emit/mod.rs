//! Emitter backends — translate the in-memory `ProofFile` AST into
//! foreign-language proof surfaces consumed by external checkers.
//!
//! Per
//! `the SMT / Lean 4 emission strategy`
//! Option C: native Lean 4 emission for the §01A cross-validation arm
//! and the §08 RL-31 + §11 coexistence handshake critical-proof set;
//! SMT-LIB emission is NOT in scope (Z3 instances are hand-authored at
//! `aims-proof/proofs/10-counterexample-corpus/`).
//!
//! Sub-module inventory:
//!
//! - `lean4` — native Lean 4 emitter. Replaces the unimplemented stub
//! at `aims-proof/checker/src/cli.rs:92-97` per
//! `the Lean 4 bootstrap proofs`
//! Implementation Items.

pub mod lean4;
