//! Structural closure-adapter validation and stable executable ordering.
//!
//! AIMS owns action eligibility and retain-topology derivation. This transport
//! seam validates only coverage, cross-fact identity, type/arity agreement,
//! and graph closure; it never re-derives policy from the type pool.

use ori_arc::{
    ArcFunction, ArcInstr, CalleeOwnerDemand, ClosureAdapterAction, ClosureAdapterPlan,
    ClosureAdapterSource, CtorKind, MemoryContract, RetainPlanId, RetainPlanKind, RetainPlanTable,
};
use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use super::RealizationError;

type FrozenClosureAdapters = (Box<[Option<ClosureAdapterPlan>]>, RetainPlanTable);

pub(super) fn freeze_closure_adapters(
    functions: &[ArcFunction],
    contracts: &[MemoryContract],
    supplied: &FxHashMap<Name, ClosureAdapterPlan>,
    retain_plans: RetainPlanTable,
    symbols: &StringInterner,
) -> Result<FrozenClosureAdapters, RealizationError> {
    validate_retain_plan_table(&retain_plans)?;
    let by_name: FxHashMap<_, _> = functions
        .iter()
        .zip(contracts)
        .map(|(function, contract)| (function.name, (function, contract)))
        .collect();
    let targets = closure_targets(functions);
    let mut reachable_plans = FxHashSet::default();

    for (&target, sites) in &targets {
        let target_symbol: Box<str> = symbols.lookup(target).into();
        let (target_function, target_contract) =
            by_name
                .get(&target)
                .ok_or_else(|| RealizationError::InvalidClosureAdapterFacts {
                    target,
                    target_symbol: target_symbol.clone(),
                    details: "closure target has no realized function body".into(),
                })?;
        let plan =
            supplied
                .get(&target)
                .ok_or_else(|| RealizationError::MissingClosureAdapterFacts {
                    target,
                    target_symbol: target_symbol.clone(),
                })?;
        validate_target_plan(
            target_function,
            target_contract,
            plan,
            &retain_plans,
            &target_symbol,
            &mut reachable_plans,
        )?;
        for site in sites {
            validate_closure_site(
                target_function,
                plan,
                site.enclosing,
                site.capture_args,
                &target_symbol,
            )?;
        }
    }

    if let Some((&target, _)) = supplied
        .iter()
        .find(|(target, _)| !targets.contains_key(target))
    {
        return Err(RealizationError::UnexpectedClosureAdapterFacts { target });
    }
    validate_plan_reachability(&retain_plans, &mut reachable_plans)?;

    let ordered = functions
        .iter()
        .map(|function| supplied.get(&function.name).cloned())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok((ordered, retain_plans))
}

struct ClosureSite<'a> {
    enclosing: &'a ArcFunction,
    capture_args: &'a [ori_arc::ArcVarId],
}

fn closure_targets<'a>(functions: &'a [ArcFunction]) -> FxHashMap<Name, Vec<ClosureSite<'a>>> {
    let mut targets: FxHashMap<Name, Vec<ClosureSite<'a>>> = FxHashMap::default();
    for enclosing in functions {
        for block in &enclosing.blocks {
            for instruction in &block.body {
                let site = match instruction {
                    ArcInstr::PartialApply { func, args, .. }
                    | ArcInstr::Construct {
                        ctor: CtorKind::Closure { func },
                        args,
                        ..
                    } => Some((*func, args.as_slice())),
                    _ => None,
                };
                if let Some((target, capture_args)) = site {
                    targets.entry(target).or_default().push(ClosureSite {
                        enclosing,
                        capture_args,
                    });
                }
            }
        }
    }
    targets
}

fn validate_target_plan(
    target: &ArcFunction,
    contract: &MemoryContract,
    plan: &ClosureAdapterPlan,
    retain_plans: &RetainPlanTable,
    target_symbol: &str,
    reachable: &mut FxHashSet<RetainPlanId>,
) -> Result<(), RealizationError> {
    if plan.capture_count() != target.num_captures {
        return invalid_target(
            target,
            target_symbol,
            "adapter capture count disagrees with the realized target",
        );
    }
    if plan.slots().len() != target.params.len() {
        return invalid_target(
            target,
            target_symbol,
            "adapter slot count disagrees with the realized target",
        );
    }
    for (index, ((slot, parameter), param_contract)) in plan
        .slots()
        .iter()
        .zip(&target.params)
        .zip(&contract.params)
        .enumerate()
    {
        let expected_source = if index < target.num_captures {
            ClosureAdapterSource::EnvironmentCapture
        } else {
            ClosureAdapterSource::BorrowedCallArgument
        };
        if slot.source != expected_source {
            return invalid_target(
                target,
                target_symbol,
                "adapter slot source disagrees with the target capture prefix",
            );
        }
        if slot.ty != parameter.ty
            || target.var_types.get(parameter.var.index()) != Some(&parameter.ty)
        {
            return invalid_target(
                target,
                target_symbol,
                "adapter slot type disagrees with the realized parameter type",
            );
        }
        let demand = param_contract.callee_owner_demand().map_err(|_| {
            invalid_target_error(
                target,
                target_symbol,
                "final parameter contract carries contradictory owner demand",
            )
        })?;
        if slot.demand != demand {
            return invalid_target(
                target,
                target_symbol,
                "adapter slot demand disagrees with the final parameter contract",
            );
        }
        match (demand, slot.action) {
            (CalleeOwnerDemand::Borrow, ClosureAdapterAction::Borrow)
            | (
                CalleeOwnerDemand::WholeValue | CalleeOwnerDemand::ProjectedField(_),
                ClosureAdapterAction::Copy,
            ) => {}
            (
                CalleeOwnerDemand::WholeValue | CalleeOwnerDemand::ProjectedField(_),
                ClosureAdapterAction::Retain(root),
            ) => {
                let node = retain_plans.get(root).ok_or_else(|| {
                    invalid_target_error(
                        target,
                        target_symbol,
                        "adapter references a missing retain-plan identity",
                    )
                })?;
                if node.ty != slot.ty {
                    return invalid_target(
                        target,
                        target_symbol,
                        "adapter retain-plan root type disagrees with its slot type",
                    );
                }
                mark_reachable(root, retain_plans, reachable);
            }
            _ => {
                return invalid_target(
                    target,
                    target_symbol,
                    "adapter action disagrees with the final parameter owner demand",
                );
            }
        }
    }
    Ok(())
}

fn validate_closure_site(
    target: &ArcFunction,
    plan: &ClosureAdapterPlan,
    enclosing: &ArcFunction,
    capture_args: &[ori_arc::ArcVarId],
    target_symbol: &str,
) -> Result<(), RealizationError> {
    if capture_args.len() != plan.capture_count() {
        return invalid_target(
            target,
            target_symbol,
            "closure construction capture count disagrees with its adapter",
        );
    }
    for (index, &argument) in capture_args.iter().enumerate() {
        if enclosing.var_types.get(argument.index()) != Some(&plan.slots()[index].ty) {
            return invalid_target(
                target,
                target_symbol,
                "closure construction capture type disagrees with its adapter prefix",
            );
        }
    }
    Ok(())
}

fn validate_retain_plan_table(table: &RetainPlanTable) -> Result<(), RealizationError> {
    for node in table.nodes() {
        if node.ty == ori_types::Idx::NONE {
            return invalid_table("retain-plan node carries the unresolved NONE type identity");
        }
        match &node.kind {
            RetainPlanKind::SelfOwnedIdentity => {}
            RetainPlanKind::OwnedFields(edges) => validate_edges(table.nodes().len(), edges)?,
            RetainPlanKind::OwnedVariants(variants) => {
                for edges in variants {
                    validate_edges(table.nodes().len(), edges)?;
                }
            }
        }
    }
    validate_acyclic(table)?;
    for (parent, node) in table.nodes().iter().enumerate() {
        let mut invalid_order = false;
        for_each_child(node, |child| invalid_order |= child.index() >= parent);
        if invalid_order {
            return invalid_table(
                "retain-plan graph is not in child-before-parent topological order",
            );
        }
    }
    Ok(())
}

fn validate_edges(
    node_count: usize,
    edges: &[ori_arc::RetainPlanEdge],
) -> Result<(), RealizationError> {
    let mut previous = None;
    for edge in edges {
        if edge.child.index() >= node_count {
            return invalid_table("retain-plan edge references a stale or missing plan identity");
        }
        if previous.is_some_and(|field| field >= edge.field) {
            return invalid_table("retain-plan fields are not strictly ordered and unique");
        }
        previous = Some(edge.field);
    }
    Ok(())
}

fn validate_acyclic(table: &RetainPlanTable) -> Result<(), RealizationError> {
    let mut states = vec![0_u8; table.nodes().len()];
    for root in 0..table.nodes().len() {
        let mut stack = vec![(root, false)];
        while let Some((index, exiting)) = stack.pop() {
            if exiting {
                states[index] = 2;
                continue;
            }
            match states[index] {
                2 => continue,
                1 => return invalid_table("retain-plan graph contains a cycle"),
                _ => {}
            }
            states[index] = 1;
            stack.push((index, true));
            push_children_reverse(&table.nodes()[index], &mut stack);
        }
    }
    Ok(())
}

fn for_each_child(node: &ori_arc::RetainPlanNode, mut visit: impl FnMut(RetainPlanId)) {
    match &node.kind {
        RetainPlanKind::SelfOwnedIdentity => {}
        RetainPlanKind::OwnedFields(edges) => {
            for edge in edges {
                visit(edge.child);
            }
        }
        RetainPlanKind::OwnedVariants(variants) => {
            for edges in variants {
                for edge in edges {
                    visit(edge.child);
                }
            }
        }
    }
}

fn push_children_reverse(node: &ori_arc::RetainPlanNode, stack: &mut Vec<(usize, bool)>) {
    match &node.kind {
        RetainPlanKind::SelfOwnedIdentity => {}
        RetainPlanKind::OwnedFields(edges) => {
            for edge in edges.iter().rev() {
                stack.push((edge.child.index(), false));
            }
        }
        RetainPlanKind::OwnedVariants(variants) => {
            for edges in variants.iter().rev() {
                for edge in edges.iter().rev() {
                    stack.push((edge.child.index(), false));
                }
            }
        }
    }
}

fn mark_reachable(
    root: RetainPlanId,
    table: &RetainPlanTable,
    reachable: &mut FxHashSet<RetainPlanId>,
) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let Some(node) = table.get(id) else {
            continue;
        };
        for_each_child(node, |child| stack.push(child));
    }
}

fn validate_plan_reachability(
    table: &RetainPlanTable,
    reachable: &mut FxHashSet<RetainPlanId>,
) -> Result<(), RealizationError> {
    if reachable.len() == table.nodes().len() {
        Ok(())
    } else {
        invalid_table("retain-plan table contains nodes unreachable from every adapter action")
    }
}

fn invalid_target<T>(
    target: &ArcFunction,
    target_symbol: &str,
    details: &'static str,
) -> Result<T, RealizationError> {
    Err(invalid_target_error(target, target_symbol, details))
}

fn invalid_target_error(
    target: &ArcFunction,
    target_symbol: &str,
    details: &'static str,
) -> RealizationError {
    RealizationError::InvalidClosureAdapterFacts {
        target: target.name,
        target_symbol: target_symbol.into(),
        details: details.into(),
    }
}

fn invalid_table<T>(details: &'static str) -> Result<T, RealizationError> {
    Err(RealizationError::InvalidRetainPlanFacts {
        details: details.into(),
    })
}

#[cfg(test)]
mod tests {
    use ori_arc::aims::lattice::AccessClass;
    use ori_arc::{
        ArcBlock, ArcBlockId, ArcParam, ArcTerminator, ArcVarId, CalleeOwnerDemand,
        ClosureAdapterSlot, MemoryContract, Ownership, RetainPlanEdge, RetainPlanNode,
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
            CalleeOwnerDemand::Borrow | CalleeOwnerDemand::ProjectedField(_) => {
                AccessClass::Borrowed
            }
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
            RealizationError::InvalidRetainPlanFacts { details }
                if details.contains("unreachable")
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
            RealizationError::InvalidRetainPlanFacts { details }
                if details.contains("cycle")
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
}
