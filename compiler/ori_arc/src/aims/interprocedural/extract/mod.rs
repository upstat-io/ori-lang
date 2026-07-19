//! Contract extraction from converged intraprocedural state maps.

mod alias_flow;
mod contract;
mod param_facts;
mod return_contract;

pub(crate) use alias_flow::build_subject_independent_alias_to_param_map;
#[cfg(test)]
pub(crate) use contract::extract_contract;
pub(crate) use contract::{extract_contract_with_call_ownership, ContractExtractionInput};
pub(crate) use param_facts::{
    find_borrowed_cow_consumed_params, find_iter_consume_call_args, CowConsumeScope,
};
