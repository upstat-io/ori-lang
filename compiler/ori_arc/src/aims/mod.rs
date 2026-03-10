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
//! - [`transfer`] — per-instruction transfer functions
//! - [`contract`] — `MemoryContract` per function
//! - [`intraprocedural`] — backward dataflow analysis
//! - `interprocedural` — SCC fixed-point loop (future)
//! - `emit_rc/` — RC emission from converged state (future)
//! - `emit_reuse/` — reuse emission (future)

pub mod contract;
pub mod intraprocedural;
pub mod lattice;
pub mod transfer;
