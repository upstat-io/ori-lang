use ori_arc::aims::lattice::AccessClass;
use ori_arc::{
    ArcBlock, ArcBlockId, ArcParam, ArcTerminator, ArcVarId, CalleeOwnerDemand, ClosureAdapterSlot,
    MemoryContract, Ownership, RetainPlanEdge, RetainPlanNode,
};
use ori_ir::SharedInterner;
use ori_types::Idx;

use super::*;

struct Fixture {
    functions: Vec<ArcFunction>,
    contracts: Vec<MemoryContract>,
    adapters: FxHashMap<Name, ClosureAdapterPlan>,
    symbols: SharedInterner,
    target: Name,
}

fn fixture(
    ownership: Ownership,
    demand: CalleeOwnerDemand,
    slot_ty: Idx,
    action: ClosureAdapterAction,
) -> Fixture {
    let symbols = SharedInterner::new();
    let caller_name = symbols.intern("main");
    let target = symbols.intern("lambda");
    let caller = ArcFunction {
        name: caller_name,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: Idx::NONE,
                func: target,
                args: Vec::new(),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        ..ArcFunction::default()
    };
    let target_function = ArcFunction {
        name: target,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership,
        }],
        var_types: vec![Idx::INT],
        ..ArcFunction::default()
    };
    let plan = ClosureAdapterPlan::from_slots(
        0,
        vec![ClosureAdapterSlot {
            source: ClosureAdapterSource::BorrowedCallArgument,
            ty: slot_ty,
            demand,
            action,
        }],
    );
    let caller_contract = MemoryContract::conservative(0);
    let mut target_contract = MemoryContract::conservative(1);
    target_contract.params[0].access = match demand {
        CalleeOwnerDemand::WholeValue => AccessClass::Owned,
        CalleeOwnerDemand::Borrow | CalleeOwnerDemand::ProjectedField(_) => AccessClass::Borrowed,
    };
    target_contract.params[0].borrowed_cow_consumed = false;
    target_contract.params[0].borrowed_cow_mutated = false;
    target_contract.params[0].iter_consumes = false;
    target_contract.params[0].iter_consumes_projected_field = match demand {
        CalleeOwnerDemand::ProjectedField(field) => Some(field),
        CalleeOwnerDemand::Borrow | CalleeOwnerDemand::WholeValue => None,
    };
    Fixture {
        functions: vec![caller, target_function],
        contracts: vec![caller_contract, target_contract],
        adapters: [(target, plan)].into_iter().collect(),
        symbols,
        target,
    }
}

fn freeze(
    fixture: &Fixture,
    table: RetainPlanTable,
) -> Result<FrozenClosureAdapters, RealizationError> {
    freeze_closure_adapters(
        &fixture.functions,
        &fixture.contracts,
        &fixture.adapters,
        table,
        &fixture.symbols,
    )
}

fn expect_freeze_error(fixture: &Fixture, table: RetainPlanTable) -> RealizationError {
    let Err(error) = freeze(fixture, table) else {
        panic!("invalid closure-adapter fixture unexpectedly validated")
    };
    error
}

fn self_node(ty: Idx) -> RetainPlanNode {
    RetainPlanNode {
        ty,
        kind: RetainPlanKind::SelfOwnedIdentity,
    }
}

fn fields_node(ty: Idx, child: u32) -> RetainPlanNode {
    RetainPlanNode {
        ty,
        kind: RetainPlanKind::OwnedFields(
            vec![RetainPlanEdge {
                field: 0,
                child: RetainPlanId::from_raw(child),
            }]
            .into_boxed_slice(),
        ),
    }
}

#[test]
fn rejects_stale_adapter_retain_plan_identity() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::WholeValue,
        Idx::INT,
        ClosureAdapterAction::Retain(RetainPlanId::from_raw(7)),
    );
    let error = expect_freeze_error(&fixture, RetainPlanTable::default());
    assert!(matches!(
        error,
        RealizationError::InvalidClosureAdapterFacts { target, details, .. }
            if target == fixture.target && details.contains("missing retain-plan identity")
    ));
}

#[test]
fn rejects_unreachable_retain_plan_node() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::WholeValue,
        Idx::INT,
        ClosureAdapterAction::Copy,
    );
    let error = expect_freeze_error(
        &fixture,
        RetainPlanTable::from_nodes(vec![self_node(Idx::STR)]),
    );
    assert!(matches!(
        error,
        RealizationError::InvalidRetainPlanFacts { details } if details.contains("unreachable")
    ));
}

#[test]
fn rejects_forward_retain_plan_edge() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::WholeValue,
        Idx::INT,
        ClosureAdapterAction::Retain(RetainPlanId::from_raw(0)),
    );
    let error = expect_freeze_error(
        &fixture,
        RetainPlanTable::from_nodes(vec![fields_node(Idx::INT, 1), self_node(Idx::STR)]),
    );
    assert!(matches!(
        error,
        RealizationError::InvalidRetainPlanFacts { details }
            if details.contains("topological order")
    ));
}

#[test]
fn rejects_cyclic_retain_plan_graph() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::WholeValue,
        Idx::INT,
        ClosureAdapterAction::Retain(RetainPlanId::from_raw(0)),
    );
    let error = expect_freeze_error(
        &fixture,
        RetainPlanTable::from_nodes(vec![fields_node(Idx::INT, 1), fields_node(Idx::STR, 0)]),
    );
    assert!(matches!(
        error,
        RealizationError::InvalidRetainPlanFacts { details } if details.contains("cycle")
    ));
}

#[test]
fn rejects_adapter_slot_type_mismatch() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::WholeValue,
        Idx::STR,
        ClosureAdapterAction::Copy,
    );
    let error = expect_freeze_error(&fixture, RetainPlanTable::default());
    assert!(matches!(
        error,
        RealizationError::InvalidClosureAdapterFacts { target, details, .. }
            if target == fixture.target && details.contains("slot type")
    ));
}

#[test]
fn rejects_adapter_action_owner_demand_mismatch() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::Borrow,
        Idx::INT,
        ClosureAdapterAction::Copy,
    );
    let error = expect_freeze_error(&fixture, RetainPlanTable::default());
    assert!(matches!(
        error,
        RealizationError::InvalidClosureAdapterFacts { target, details, .. }
            if target == fixture.target && details.contains("owner demand")
    ));
}

#[test]
fn rejects_adapter_slot_demand_that_disagrees_with_final_contract() {
    let mut fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::Borrow,
        Idx::INT,
        ClosureAdapterAction::Borrow,
    );
    fixture.adapters.insert(
        fixture.target,
        ClosureAdapterPlan::from_slots(
            0,
            vec![ClosureAdapterSlot {
                source: ClosureAdapterSource::BorrowedCallArgument,
                ty: Idx::INT,
                demand: CalleeOwnerDemand::WholeValue,
                action: ClosureAdapterAction::Copy,
            }],
        ),
    );
    let error = expect_freeze_error(&fixture, RetainPlanTable::default());
    assert!(matches!(
        error,
        RealizationError::InvalidClosureAdapterFacts { target, details, .. }
            if target == fixture.target && details.contains("final parameter contract")
    ));
}

#[test]
fn final_contract_demand_not_arc_param_ownership_controls_adapter() {
    let fixture = fixture(
        Ownership::Owned,
        CalleeOwnerDemand::Borrow,
        Idx::INT,
        ClosureAdapterAction::Borrow,
    );
    assert!(freeze(&fixture, RetainPlanTable::default()).is_ok());
}
