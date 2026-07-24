use super::*;
use crate::aims::lattice::AccessClass;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcParam, ArcTerminator, ArcVarId, RcStrategy, ValueRepr,
    VariableMetadataState,
};
use crate::Ownership;

fn realized_target(
    name: Name,
    params: &[(Idx, Ownership, ValueRepr, Option<RcStrategy>)],
    captures: usize,
) -> ArcFunction {
    let arc_params = params
        .iter()
        .enumerate()
        .map(|(index, (ty, ownership, _, _))| {
            let Ok(index) = u32::try_from(index) else {
                panic!("test target has more parameters than ArcVarId can represent")
            };
            ArcParam {
                var: ArcVarId::new(index),
                ty: *ty,
                ownership: *ownership,
            }
        })
        .collect();
    ArcFunction {
        name,
        params: arc_params,
        var_types: params.iter().map(|entry| entry.0).collect(),
        var_reprs: params.iter().map(|entry| entry.2).collect(),
        var_rc_strategies: params.iter().map(|entry| entry.3).collect(),
        var_metadata_state: VariableMetadataState::Realized,
        num_captures: captures,
        ..ArcFunction::default()
    }
}

fn caller(target: Name, captures: &[(ArcVarId, Idx)]) -> ArcFunction {
    let Ok(destination) = u32::try_from(captures.len()) else {
        panic!("test closure has more captures than ArcVarId can represent")
    };
    ArcFunction {
        name: Name::from_raw(99),
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(destination),
                ty: Idx::NONE,
                func: target,
                args: captures.iter().map(|entry| entry.0).collect(),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        var_types: captures.iter().map(|entry| entry.1).collect(),
        ..ArcFunction::default()
    }
}

fn contract_for(target: &ArcFunction) -> MemoryContract {
    let mut contract = MemoryContract::conservative(target.params.len());
    for (param, facts) in target.params.iter().zip(&mut contract.params) {
        facts.access = match param.ownership {
            Ownership::Owned => AccessClass::Owned,
            Ownership::Borrowed => AccessClass::Borrowed,
        };
        facts.borrowed_cow_consumed = false;
        facts.borrowed_cow_mutated = false;
        facts.iter_consumes = false;
    }
    contract
}

fn contract_map(target: Name, contract: MemoryContract) -> FxHashMap<Name, MemoryContract> {
    [(target, contract)].into_iter().collect()
}

fn freeze_successfully(
    functions: &[ArcFunction],
    contracts: &FxHashMap<Name, MemoryContract>,
    pool: &Pool,
    registry: &TypeRegistry,
) -> FrozenClosureAdapters {
    match freeze_closure_adapter_plans(functions, contracts, pool, registry) {
        Ok(frozen) => frozen,
        Err(errors) => panic!("test closure adapter facts must freeze: {errors:?}"),
    }
}

#[test]
fn plan_adapts_captures_and_explicit_arguments_through_logical_ids() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(10);
    let target = realized_target(
        target_name,
        &[
            (
                Idx::STR,
                Ownership::Owned,
                ValueRepr::FatValue,
                Some(RcStrategy::FatPointer),
            ),
            (Idx::INT, Ownership::Owned, ValueRepr::Scalar, None),
            (
                Idx::STR,
                Ownership::Borrowed,
                ValueRepr::FatValue,
                Some(RcStrategy::FatPointer),
            ),
        ],
        1,
    );
    let caller = caller(target_name, &[(ArcVarId::new(0), Idx::STR)]);
    let contracts = contract_map(target_name, contract_for(&target));

    let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
    let plan = &frozen.adapters[&target_name];
    assert_eq!(plan.capture_count(), 1);
    assert_eq!(plan.explicit_arity(), 2);
    let ClosureAdapterAction::Retain(id) = plan.slots()[0].action else {
        panic!("owned str must retain through a logical plan")
    };
    assert!(matches!(
        frozen.retain_plans.get(id).map(|node| &node.kind),
        Some(RetainPlanKind::SelfOwnedIdentity)
    ));
    assert_eq!(plan.slots()[1].action, ClosureAdapterAction::Copy);
    assert_eq!(plan.slots()[2].action, ClosureAdapterAction::Borrow);
}

#[test]
fn retain_plan_root_preserves_nominal_adapter_slot_identity() {
    let mut pool = Pool::new();
    let bundle_name = Name::from_raw(30);
    let items_name = Name::from_raw(31);
    let nominal_bundle = pool.named(bundle_name);
    let list_str = pool.list(Idx::STR);
    let bundle_body = pool.struct_type(bundle_name, &[(items_name, list_str)]);
    pool.set_resolution(nominal_bundle, bundle_body);
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(32);
    let target = realized_target(
        target_name,
        &[(
            nominal_bundle,
            Ownership::Owned,
            ValueRepr::Aggregate,
            Some(RcStrategy::AggregateFields),
        )],
        0,
    );
    let caller = caller(target_name, &[]);
    let contracts = contract_map(target_name, contract_for(&target));

    let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
    let slot = frozen.adapters[&target_name].slots()[0];
    let ClosureAdapterAction::Retain(root) = slot.action else {
        panic!("owned Bundle must retain its nested list")
    };
    let Some(root_node) = frozen.retain_plans.get(root) else {
        panic!("Bundle retain root must exist")
    };

    assert_ne!(nominal_bundle, bundle_body);
    assert_eq!(slot.ty, nominal_bundle);
    assert_eq!(root_node.ty, nominal_bundle);
    assert!(matches!(root_node.kind, RetainPlanKind::OwnedFields(_)));
}

#[test]
fn owned_iterator_is_rejected_before_physical_projection() {
    let mut pool = Pool::new();
    let iterator = pool.iterator(Idx::INT);
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(11);
    let target = realized_target(
        target_name,
        &[(
            iterator,
            Ownership::Owned,
            ValueRepr::RcPointer,
            Some(RcStrategy::Iterator),
        )],
        0,
    );
    let caller = caller(target_name, &[]);
    let contracts = contract_map(target_name, contract_for(&target));

    assert!(matches!(
        freeze_closure_adapter_plans(&[caller, target], &contracts, &pool, &registry),
        Err(errors)
            if matches!(errors.as_slice(), [ClosureAbiError::OwnedParameterNotShareable { parameter: 0, .. }])
    ));
}

#[test]
fn inline_variant_retains_exact_active_payload_topology() {
    let mut pool = Pool::new();
    let option_str = pool.option(Idx::STR);
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(12);
    let target = realized_target(
        target_name,
        &[(
            option_str,
            Ownership::Owned,
            ValueRepr::Aggregate,
            Some(RcStrategy::InlineEnum),
        )],
        0,
    );
    let caller = caller(target_name, &[]);
    let contracts = contract_map(target_name, contract_for(&target));

    let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
    let ClosureAdapterAction::Retain(root) = frozen.adapters[&target_name].slots()[0].action else {
        panic!("Option<str> must retain its active payload")
    };
    let Some(RetainPlanNode {
        kind: RetainPlanKind::OwnedVariants(variants),
        ..
    }) = frozen.retain_plans.get(root)
    else {
        panic!("Option<str> needs a variant-aware root")
    };
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].len(), 1);
    assert!(variants[1].is_empty());
}

#[test]
fn zero_capture_copy_and_borrow_signature_needs_no_wrapper_retain() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(13);
    let target = realized_target(
        target_name,
        &[
            (Idx::INT, Ownership::Owned, ValueRepr::Scalar, None),
            (
                Idx::STR,
                Ownership::Borrowed,
                ValueRepr::FatValue,
                Some(RcStrategy::FatPointer),
            ),
        ],
        0,
    );
    let caller = caller(target_name, &[]);
    let contracts = contract_map(target_name, contract_for(&target));

    let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
    assert!(!frozen.adapters[&target_name].requires_retain());
}

#[test]
fn batch_freezer_rejects_capture_count_drift() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(14);
    let target = realized_target(
        target_name,
        &[(Idx::INT, Ownership::Owned, ValueRepr::Scalar, None)],
        1,
    );
    let caller = caller(target_name, &[]);
    let contracts = contract_map(target_name, contract_for(&target));

    assert!(matches!(
        freeze_closure_adapter_plans(&[caller, target], &contracts, &pool, &registry),
        Err(errors)
            if matches!(errors.as_slice(), [ClosureAbiError::CaptureArityMismatch { .. }])
    ));
}

#[test]
fn borrowed_iter_consume_demands_a_whole_value_credit() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    let registry = TypeRegistry::new();
    let target_name = Name::from_raw(15);
    let target = realized_target(
        target_name,
        &[(
            list,
            Ownership::Borrowed,
            ValueRepr::RcPointer,
            Some(RcStrategy::HeapPointer),
        )],
        0,
    );
    let caller = caller(target_name, &[]);
    let mut contract = contract_for(&target);
    contract.params[0].iter_consumes = true;
    let contracts = contract_map(target_name, contract);

    let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
    let slot = frozen.adapters[&target_name].slots()[0];
    assert_eq!(slot.demand, CalleeOwnerDemand::WholeValue);
    assert!(matches!(slot.action, ClosureAdapterAction::Retain(_)));
}

#[cfg(target_pointer_width = "64")]
#[test]
fn owned_parameter_failure_preserves_index_conversion_source() {
    let overflow = (u32::MAX as usize).saturating_add(1);
    let Err(source) = u32::try_from(overflow) else {
        panic!("a retain-plan index above u32::MAX must be rejected");
    };
    let error = ClosureAbiError::OwnedParameterNotShareable {
        target: Name::from_raw(1),
        parameter: 0,
        ty: Idx::INT,
        failure: DuplicationFailure::RetainPlanIndexOverflow(source),
    };

    assert!(std::error::Error::source(&error).is_some());
}
