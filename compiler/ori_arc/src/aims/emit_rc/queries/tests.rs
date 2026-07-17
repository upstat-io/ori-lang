//! Tests for the post-emission RC-incremented variable tracking.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{FipContract, MemoryContract, ReturnContract};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, RcAtomicity, RcStrategy,
};

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
    // The kept duplication-alias inc shape: %1 = %0 (inc'd), %2 = %0 (sibling).
    // The inc on %1 bumps the SHARED object's refcount — %0 and %2 are
    // physically incremented too; classifying them un-incremented would
    // promote a later COW site to StaticUnique and mutate the shared buffer
    // in place (the store-then-`.set` holder-view corruption).
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
    // %0 receives a duplication credit before moving into %1.field0. A
    // later projection %2 names that same payload allocation, while %1 is a
    // distinct aggregate allocation. COW on %2 must therefore observe the
    // added owner and use a dynamic sharing check.
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
    // %1 aliases the original %0 and is borrowed by a view producer. The
    // producer's hidden runtime retain creates %2 as another owner on the
    // normal edge. In the successor block, COW receiver %3 aliases the
    // original root, so it must inherit the competing-owner fact even though
    // no ARC RcInc spells the retain across that block boundary.
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
