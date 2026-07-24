//! Per-parameter fact detection: iter-consume transfer and
//! borrowed-read-only forwarding safety.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ExactTransferState, MemoryContract, ReturnAliasShape};
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcVarId};
use crate::ArcClassification;

use super::alias_flow::{
    build_alias_to_param_map, find_consumed_params, find_payload_containment_params,
    find_return_alias_shapes, find_return_flow_params,
};

mod aggregate_transfer;
mod borrowed_facts;
mod iter_consume;
mod ownership_credit;

pub(super) use borrowed_facts::find_borrowed_read_only_params;
pub(crate) use borrowed_facts::{find_borrowed_cow_consumed_params, CowConsumeScope};
pub(crate) use iter_consume::find_iter_consume_call_args;
pub(super) use iter_consume::find_iter_consume_params;
pub(super) use ownership_credit::find_borrowed_root_credit_params;

/// Structural facts used to construct one contract per parameter.
pub(super) struct ParamFacts {
    pub(super) consumed: FxHashSet<usize>,
    pub(super) return_flow: FxHashSet<usize>,
    pub(super) return_alias_shapes: FxHashMap<usize, ReturnAliasShape>,
    pub(super) payload_containment: FxHashSet<usize>,
    pub(super) iter_consume: FxHashSet<usize>,
    pub(super) borrowed_read_only: FxHashSet<usize>,
    pub(super) borrowed_cow_consumed: FxHashSet<usize>,
    pub(super) borrowed_cow_mutated: FxHashSet<usize>,
    pub(super) owner_credit: FxHashSet<usize>,
    pub(super) exact_transfer_states: FxHashMap<usize, ExactTransferState>,
    pub(super) exact_transfer_witnesses: Vec<aggregate_transfer::ExactAggregateTransferWitness>,
}

/// Detect all structural facts for one function's parameters.
#[expect(
    clippy::too_many_arguments,
    reason = "fact extraction consumes the complete frozen analysis authority"
)]
pub(super) fn detect_param_facts(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    param_vars: &FxHashMap<ArcVarId, usize>,
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
    type_registry: Option<&ori_types::TypeRegistry>,
    context_regions_present: bool,
) -> ParamFacts {
    let alias_to_param = build_alias_to_param_map(func, param_vars, Some(sigs));
    let mut consumed = find_consumed_params(func, sigs, &alias_to_param);
    let mut return_flow = find_return_flow_params(func, &alias_to_param);
    let return_alias_shapes = find_return_alias_shapes(func, &alias_to_param, sigs);
    for (&idx, &shape) in &return_alias_shapes {
        if shape == ReturnAliasShape::Direct {
            return_flow.insert(idx);
        }
    }
    let containment = find_payload_containment_params(func, &alias_to_param);
    let iter_consume = find_iter_consume_params(func, sigs, &alias_to_param, interner);
    let borrowed_read_only = find_borrowed_read_only_params(func, sigs, &alias_to_param, interner);
    let borrowed_cow_consumed = find_borrowed_cow_consumed_params(
        func,
        sigs,
        &alias_to_param,
        interner,
        CowConsumeScope::AnyConsume,
    );
    let borrowed_cow_mutated = find_borrowed_cow_consumed_params(
        func,
        sigs,
        &alias_to_param,
        interner,
        CowConsumeScope::MutatorOnly,
    );
    let owner_credit = find_borrowed_root_credit_params(
        func,
        sigs,
        &alias_to_param,
        classifier,
        builtins,
        exact_callables,
    );
    let aggregate_transfer = aggregate_transfer::find_exact_aggregate_transfers(
        func,
        sigs,
        &alias_to_param,
        classifier,
        exact_callables,
        interner,
        type_registry,
        context_regions_present,
    );
    consumed.extend(&return_flow);
    consumed.extend(&aggregate_transfer.consumed_params);
    ParamFacts {
        consumed,
        return_flow,
        return_alias_shapes,
        payload_containment: containment.any,
        iter_consume,
        borrowed_read_only,
        borrowed_cow_consumed,
        borrowed_cow_mutated,
        owner_credit,
        exact_transfer_states: aggregate_transfer.states,
        exact_transfer_witnesses: aggregate_transfer.witnesses,
    }
}

pub(crate) use aggregate_transfer::{ExactAggregateTransferWitness, ExactTransferCommitWitness};
