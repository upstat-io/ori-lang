//! Tests for the post-emission RC-incremented variable tracking.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{FipContract, MemoryContract, ReturnContract};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, RcAtomicity, RcStrategy,
};
use crate::ownership::Ownership;

use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::test_helpers::{make_block, make_invoke};

use super::collect_rc_incremented_vars;

fn alias_of(dst: u32, src: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        value: ArcValue::Var(ArcVarId::new(src)),
    }
}

fn rc_inc_of(var: u32) -> ArcInstr {
    ArcInstr::RcInc {
        var: ArcVarId::new(var),
        count: 1,
        strategy: RcStrategy::HeapPointer,
        atomicity: RcAtomicity::default_atomic(),
    }
}

fn one_block_func(n_vars: u32, body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        var_types: (0..n_vars).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

fn one_param_func(ownership: Ownership, body: Vec<ArcInstr>) -> ArcFunction {
    let mut func = one_block_func(2, body);
    func.params = vec![ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::STR,
        ownership,
    }];
    func
}

#[test]
fn borrowed_cow_mutator_contract_seeds_incoming_credit_and_aliases() {
    let func = one_param_func(Ownership::Borrowed, vec![alias_of(1, 0)]);
    let mut contract = MemoryContract::all_borrowed(1, FipContract::Never);
    contract.params[0].borrowed_cow_mutated = true;
    let contracts = FxHashMap::from_iter([(func.name, contract)]);

    let set = collect_rc_incremented_vars(&func, None, &contracts);
    let expected = FxHashSet::from_iter([ArcVarId::new(0), ArcVarId::new(1)]);
    assert_eq!(
        set, expected,
        "the callee must see the caller-funded competing owner through every alias"
    );
}

#[test]
fn iterator_only_borrowed_contract_does_not_seed_cow_credit() {
    let func = one_param_func(Ownership::Borrowed, vec![alias_of(1, 0)]);
    let mut contract = MemoryContract::all_borrowed(1, FipContract::Never);
    contract.params[0].iter_consumes = true;
    let contracts = FxHashMap::from_iter([(func.name, contract)]);

    assert!(
        collect_rc_incremented_vars(&func, None, &contracts).is_empty(),
        "iterator transfer accounting must not masquerade as a COW-sharing credit"
    );
}

#[test]
fn owned_mutator_parameter_does_not_seed_borrowed_boundary_credit() {
    let func = one_param_func(Ownership::Owned, vec![alias_of(1, 0)]);
    let mut contract = MemoryContract::all_borrowed(1, FipContract::Never);
    contract.params[0].borrowed_cow_mutated = true;
    let contracts = FxHashMap::from_iter([(func.name, contract)]);

    assert!(
        collect_rc_incremented_vars(&func, None, &contracts).is_empty(),
        "owned parameters already transfer their one credit into the callee"
    );
}

#[test]
fn rc_incremented_propagates_forward_through_alias_chain() {
    // RcInc on the root %0; aliases %1 = %0 and %2 = %1 share the object.
    let func = one_block_func(3, vec![rc_inc_of(0), alias_of(1, 0), alias_of(2, 1)]);
    let set = collect_rc_incremented_vars(&func, None, &FxHashMap::default());
    let expected: FxHashSet<ArcVarId> = (0..3).map(ArcVarId::new).collect();
    assert_eq!(
        set, expected,
        "an inc on the root marks every alias of the same object"
    );
}

#[test]
fn rc_incremented_propagates_backward_from_alias_to_root_and_siblings() {
    // Incrementing one alias increases the shared allocation's count, so peer
    // aliases cannot later qualify for static-unique COW mutation.
    let func = one_block_func(3, vec![alias_of(1, 0), rc_inc_of(1), alias_of(2, 0)]);
    let set = collect_rc_incremented_vars(&func, None, &FxHashMap::default());
    let expected: FxHashSet<ArcVarId> = (0..3).map(ArcVarId::new).collect();
    assert_eq!(
        set, expected,
        "an inc on a duplication alias marks the root and every sibling alias"
    );
}

#[test]
fn rc_incremented_empty_without_incs() {
    let func = one_block_func(3, vec![alias_of(1, 0), alias_of(2, 0)]);
    assert!(
        collect_rc_incremented_vars(&func, None, &FxHashMap::default()).is_empty(),
        "no RcInc anywhere — no member is physically incremented"
    );
}

#[test]
fn rc_incremented_propagates_from_construct_arg_to_projected_field() {
    // A projected payload inherits its pre-insertion duplication credit and
    // must use a dynamic COW sharing check.
    let func = one_block_func(
        3,
        vec![
            rc_inc_of(0),
            ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: Idx::from_raw(2),
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(0)],
            },
            ArcInstr::Project {
                dst: ArcVarId::new(2),
                ty: Idx::from_raw(1),
                value: ArcVarId::new(1),
                field: 0,
            },
        ],
    );
    let state_map = AimsStateMap::new(&func);
    let partition = compute_birth_site_partition(&func, &state_map);

    let set = collect_rc_incremented_vars(&func, Some(&partition), &FxHashMap::default());
    assert!(
        set.contains(&ArcVarId::new(0)),
        "incremented source missing"
    );
    assert!(
        set.contains(&ArcVarId::new(2)),
        "projected field did not inherit its payload's added-owner fact"
    );
    assert!(
        !set.contains(&ArcVarId::new(1)),
        "aggregate and payload are different allocation identities"
    );
}

#[test]
fn cross_block_sharing_view_invoke_marks_original_sibling_alias() {
    // A view producer's hidden retain creates a competing owner across the
    // normal edge; successor aliases must inherit that fact without an ARC
    // `RcInc` instruction.
    let callee = Name::from_raw(7);
    let func = ArcFunction {
        var_types: vec![Idx::STR; 4],
        blocks: vec![
            make_block(
                ArcBlockId::new(0),
                vec![alias_of(1, 0)],
                make_invoke(
                    ArcVarId::new(2),
                    Idx::STR,
                    callee,
                    vec![ArcVarId::new(1)],
                    vec![ArgOwnership::Borrowed],
                    ArcBlockId::new(1),
                    ArcBlockId::new(2),
                ),
            ),
            make_block(
                ArcBlockId::new(1),
                vec![alias_of(3, 0)],
                ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            ),
            make_block(ArcBlockId::new(2), Vec::new(), ArcTerminator::Resume),
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let mut sharing = MemoryContract::all_borrowed(1, FipContract::Never);
    sharing.return_info = ReturnContract {
        returns_sharing_view: true,
        ..ReturnContract::CONSERVATIVE
    };
    let contracts = FxHashMap::from_iter([(callee, sharing)]);

    let set = collect_rc_incremented_vars(&func, None, &contracts);
    let expected: FxHashSet<ArcVarId> = (0..4).map(ArcVarId::new).collect();
    assert_eq!(
        set, expected,
        "the hidden sharing-view retain must reach every source alias"
    );
}
