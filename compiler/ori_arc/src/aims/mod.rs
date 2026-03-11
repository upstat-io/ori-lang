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
//! - [`emit_rc`] — RC emission from converged state
//! - [`emit_reuse`] — reuse emission from converged state

pub mod builtins;
pub mod contract;
pub mod emit_rc;
pub mod emit_reuse;
pub mod interprocedural;
pub mod intraprocedural;
pub mod lattice;
pub mod transfer;
