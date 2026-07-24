use super::*;

use crate::aims::contract::FipContract;
use crate::ir::{ArcBlock, ArcBlockId, ArcVarId};
use crate::test_helpers::make_func_named;
use ori_types::Idx;

fn effect_contract(may_deallocate: bool) -> MemoryContract {
    let mut contract = MemoryContract::all_borrowed(0, FipContract::Certified);
    contract.effects.may_deallocate = may_deallocate;
    contract
}

fn function_calling(name: Name, callees: &[Name]) -> ArcFunction {
    let mut next_var = 1u32;
    let body = callees
        .iter()
        .map(|&callee| {
            let dst = ArcVarId::new(next_var);
            next_var += 1;
            ArcInstr::Apply {
                dst,
                ty: Idx::UNIT,
                func: callee,
                args: vec![],
                arg_ownership: vec![],
                mono_instance_id: None,
            }
        })
        .collect();
    let return_value = ArcVarId::new(next_var.saturating_sub(1));
    make_func_named(
        name,
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return {
                value: return_value,
            },
        }],
        vec![Idx::UNIT; 16],
    )
}

fn function_invoking(name: Name, callee: Name) -> ArcFunction {
    make_func_named(
        name,
        vec![],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(1),
                    ty: Idx::UNIT,
                    func: callee,
                    args: vec![],
                    arg_ownership: vec![],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(1),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::UNIT; 2],
    )
}

#[test]
fn post_emission_join_zero_local_evidence_preserves_preexisting_true() {
    let name = Name::from_raw(10);
    let functions = vec![function_calling(name, &[])];
    let mut contracts = FxHashMap::from_iter([(name, effect_contract(true))]);

    let downgrades =
        reconcile_post_emission_may_deallocate(&functions, &mut contracts, &[(name, 0)]);

    assert!(
        contracts[&name].effects.may_deallocate,
        "local false evidence must not erase a converged true effect"
    );
    assert_eq!(contracts[&name].fip, FipContract::Never);
    assert_eq!(downgrades, 1);
}

#[test]
fn runtime_invoke_true_callee_effect_propagates_and_oracle_accepts() {
    let interner = ori_ir::StringInterner::default();
    let caller = interner.intern("caller");
    let runtime = interner.intern("ori_panic");
    let functions = vec![function_invoking(caller, runtime)];
    let mut runtime_contract = effect_contract(true);
    runtime_contract.fip = FipContract::Never;
    let mut contracts = FxHashMap::from_iter([
        (caller, effect_contract(false)),
        (runtime, runtime_contract),
    ]);

    reconcile_post_emission_may_deallocate(&functions, &mut contracts, &[(caller, 0)]);

    assert!(
        contracts[&caller].effects.may_deallocate,
        "Invoke caller must inherit the runtime deallocation effect"
    );
    let mismatches = crate::aims::verify::oracle::verify_coherence(
        &functions[0],
        &contracts[&caller],
        &contracts,
        &interner,
        0,
    );
    assert!(
        mismatches.is_empty(),
        "caller contract should cover the runtime effect: {mismatches:?}"
    );
}

#[test]
fn post_emission_closure_call_chain_and_scc_propagates_to_all_callers() {
    let leaf = Name::from_raw(20);
    let middle = Name::from_raw(21);
    let cycle_a = Name::from_raw(22);
    let cycle_b = Name::from_raw(23);
    let top = Name::from_raw(24);
    // Deliberately place callers before callees: convergence must not depend
    // on the function slice order.
    let functions = vec![
        function_calling(top, &[cycle_a]),
        function_calling(cycle_a, &[cycle_b, middle]),
        function_calling(cycle_b, &[cycle_a]),
        function_calling(middle, &[leaf]),
        function_calling(leaf, &[]),
    ];
    let mut contracts = FxHashMap::from_iter(
        [top, cycle_a, cycle_b, middle, leaf].map(|name| (name, effect_contract(false))),
    );
    let reuse_updates = [(top, 0), (cycle_a, 0), (cycle_b, 0), (middle, 0), (leaf, 1)];

    let downgrades =
        reconcile_post_emission_may_deallocate(&functions, &mut contracts, &reuse_updates);

    for name in [leaf, middle, cycle_a, cycle_b, top] {
        assert!(
            contracts[&name].effects.may_deallocate,
            "post-emission effect did not reach {name:?}"
        );
        assert_eq!(contracts[&name].fip, FipContract::Never);
    }
    assert_eq!(downgrades, 5);
}

#[test]
fn post_emission_closure_disconnected_function_without_evidence_stays_false() {
    let source = Name::from_raw(30);
    let disconnected = Name::from_raw(31);
    let functions = vec![
        function_calling(source, &[]),
        function_calling(disconnected, &[]),
    ];
    let mut contracts = FxHashMap::from_iter([
        (source, effect_contract(false)),
        (disconnected, effect_contract(false)),
    ]);

    let downgrades = reconcile_post_emission_may_deallocate(
        &functions,
        &mut contracts,
        &[(source, 1), (disconnected, 0)],
    );

    assert!(contracts[&source].effects.may_deallocate);
    assert!(
        !contracts[&disconnected].effects.may_deallocate,
        "closure must not promote a disconnected function"
    );
    assert_eq!(contracts[&disconnected].fip, FipContract::Certified);
    assert_eq!(downgrades, 1);
}
