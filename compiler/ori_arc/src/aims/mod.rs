//! AIMS — Ori's backend-neutral ownership calculus.
//!
//! “ARC Intelligent Memory System” is the historical name associated with the
//! first compiled counter projection. Neither that name nor this crate's
//! `ori_arc` path makes LLVM, counters, headers, or target instructions part of
//! the calculus.
//!
//! Unified ownership analysis replacing the sequential analysis passes
//! (borrow inference, liveness, uniqueness, logical ownership-event
//! realization, reset/reuse, and event elimination) with a single
//! formally-grounded lattice. Current `RcInc`/`RcDec` carrier names do not
//! prescribe a counter-based physical projection.
//!
//! # Architecture
//!
//! - [`lattice`] — `AimsState` product lattice (7 dimensions), join,
//!   canonicalization, query predicates
//! - [`transfer`] — per-instruction transfer functions
//! - [`contract`] — `MemoryContract` per function
//! - [`intraprocedural`] — backward dataflow analysis
//! - [`interprocedural`] — SCC fixed-point loop
//! - [`builtins`] — hardcoded contracts for builtin methods
//! - [`emit_rc`] — transitional logical ownership-event carrier helpers
//!   (submodules used by `realize/`)
//! - [`emit_reuse`] — reuse-eligibility and current-carrier helpers
//!   (submodules used by `realize/`)
//! - [`normalize`] — Stage 3a: TRMC normalization (detection, lifting,
//!   rewriting, and verification)
//! - [`realize`] — unified realization (two-phase decision surface)

pub mod builtins;
pub(crate) mod class_ledger;
pub mod contract;
pub(crate) mod demand;
pub mod emit_rc;
pub mod emit_reuse;
pub mod immortal;
pub mod interprocedural;
pub mod intraprocedural;
pub mod lattice;
pub mod normalize;
pub(crate) mod primitive;
pub use primitive::{freeze_primitive_facts, validate_primitive_facts};
pub mod realize;
pub mod transfer;
pub mod verify;
