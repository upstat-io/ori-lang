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
//! - Owned-set collection + SCC fixpoint borrow inference
//!   (counting-immutable-beans technique); see also the crate's
//!   `borrow/per_scc.rs` SCC borrow inference
//! - FP² (Lorenzen et al., ICFP 2023): FIP certification

mod demand_propagation;
mod extract;
#[cfg(test)]
mod impl_method_contracts;
mod scc_driver;
mod use_count;

#[cfg(test)]
mod tests;

#[cfg(test)]
use impl_method_contracts::{augment_contracts_with_impl_callees, compute_impl_method_contracts};
pub(crate) use scc_driver::analyze_program_with_external_contracts_and_boundaries;
pub use scc_driver::{analyze_program, analyze_program_with_external_contracts};

#[cfg(test)]
pub(crate) use extract::extract_contract;
pub(crate) use extract::{
    build_subject_independent_alias_to_param_map, extract_contract_with_call_ownership,
    find_iter_consume_call_args,
};
