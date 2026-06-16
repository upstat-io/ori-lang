//! Interprocedural analysis for AIMS.
//!
//! Computes a [`MemoryContract`](super::contract::MemoryContract) for every
//! function in the program via SCC-based fixed-point iteration. The contract
//! encodes per-parameter access class, consumption, cardinality, and return
//! value uniqueness.
//!
//! # Architecture
//!
//! 1. Build call graph + Tarjan SCCs (reusing `graph::call_graph` + `graph::scc`)
//! 2. Process SCCs in topological order (callees before callers)
//! 3. Non-recursive SCCs: single intraprocedural analysis pass
//! 4. Recursive SCCs: iterate until all contracts converge
//!
//! At each step, [`super::intraprocedural::analyze_function`] runs the
//! backward dataflow analysis, then [`extract::extract_contract`] reads the
//! converged state map to produce a `MemoryContract`.
//!
//! # Module structure
//!
//! - [`scc_driver`] — call-graph SCC driver + fixed-point loop (`analyze_program`)
//! - [`demand_propagation`] — post-fixpoint uniqueness tightening
//! - [`use_count`] — variable use-counting (load-bearing for BUG-04-069)
//! - [`extract`] — contract extraction from converged state maps
//!
//! # References
//!
//! - Lean 4 `src/Lean/Compiler/IR/Borrow.lean`: `collect_O` + SCC loop
//! - `ori_arc` `borrow/per_scc.rs`: existing SCC borrow inference
//! - FP² (Lorenzen et al., ICFP 2023): FIP certification

mod demand_propagation;
mod extract;
mod impl_method_contracts;
mod scc_driver;
mod use_count;

#[cfg(test)]
mod tests;

pub use impl_method_contracts::{
    augment_contracts_with_impl_callees, compute_impl_method_contracts,
};
pub use scc_driver::analyze_program;

pub(crate) use extract::extract_contract;
