//! SCC-based interprocedural analysis for AIMS memory contracts.
//!
//! # Algorithm
//!
//! 1. Build the call graph and its Tarjan SCCs.
//! 2. Process SCCs from callees to callers.
//! 3. Analyze acyclic SCCs once and recursive SCCs to a fixed point.
//! 4. Tighten uniqueness from the converged contracts.
//!
//! Each iteration analyzes local demand and extracts parameter, effect, and
//! return facts. Diagnostics share the `ori_arc::aims::interprocedural` target.

mod demand_propagation;
mod extract;
mod scc_driver;
mod use_count;

#[cfg(test)]
mod tests;

pub use scc_driver::analyze_program;
pub(crate) use scc_driver::analyze_program_with_external_contracts_boundaries_and_types;

pub(crate) use extract::{
    build_subject_independent_alias_to_param_map, extract_contract_with_call_ownership,
    find_borrowed_cow_consumed_params, find_iter_consume_call_args, ContractExtractionInput,
    CowConsumeScope, ExactAggregateTransferWitness, ExactTransferCommitWitness,
};
#[cfg(test)]
pub(crate) use extract::{extract_contract, extract_contract_and_transfers_with_call_ownership};
