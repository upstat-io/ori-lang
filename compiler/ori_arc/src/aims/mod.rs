//! AIMS — ARC Intelligent Memory System.
//!
//! Unified ownership analysis replacing the sequential analysis passes
//! (borrow inference, liveness, uniqueness, RC insertion, reset/reuse,
//! RC elimination) with a single formally-grounded lattice.
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
//! - [`emit_rc`] — RC emission helpers (submodules used by `realize/`)
//! - [`emit_reuse`] — reuse emission helpers (submodules used by `realize/`)
//! - [`normalize`] — Stage 3a: TRMC normalization (detection, lifting,
//!   rewriting, and verification)
//! - [`realize`] — unified realization (two-phase decision surface)

pub mod builtins;
pub mod contract;
pub mod emit_rc;
pub mod emit_reuse;
pub mod immortal;
pub mod interprocedural;
pub mod intraprocedural;
pub mod lattice;
pub mod normalize;
pub mod realize;
pub mod transfer;
pub mod verify;
