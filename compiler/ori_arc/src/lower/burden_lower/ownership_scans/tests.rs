//! Unit pins for the construct-fed dead-param ALT-CONSUMED mode
//! (`compute_construct_fed_dead_param_lineage` gate (d) admission by
//! FUNDED-consume proof) and the loop back-edge rep-resolution boundary of
//! `dead_param_single_feeding_rep`. Spec: Annex E §AIMS RL-1 + RL-2 + RL-5.

use ori_ir::{Name, StringInterner};
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind,
};

use super::dead_param::dead_param_single_feeding_rep;
use super::union_find::ForwarderUnionFind;

/// `Let` alias of `src` into `dst`, typed STR (heap).
fn alias_of(dst: u32, src: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        value: ArcValue::Var(ArcVarId::new(src)),
    }
}

/// Aggregate store consuming `arg` into `dst` (a Tuple Construct — an owned
/// transfer position that is NOT a sum-aggregate / collection root).
fn store_of(dst: u32, arg: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(arg)],
    }
}

/// FRESH collection-buffer root: `dst = Construct ListLiteral []`.
fn list_construct_of(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        ctor: CtorKind::ListLiteral,
        args: Vec::new(),
    }
}

fn block(
    id: u32,
    params: Vec<(ArcVarId, Idx)>,
    body: Vec<ArcInstr>,
    term: ArcTerminator,
) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params,
        body,
        terminator: term,
    }
}

fn jump(target: u32, args: Vec<u32>) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args: args.into_iter().map(ArcVarId::new).collect(),
    }
}

fn func_of(blocks: Vec<ArcBlock>, n_vars: u32) -> ArcFunction {
    ArcFunction {
        var_types: (0..n_vars).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks,
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

fn run_construct_fed(
    func: &ArcFunction,
    owned: &FxHashSet<ArcVarId>,
) -> super::ConstructFedDeadParamLineage {
    let interner = StringInterner::new();
    super::compute_construct_fed_dead_param_lineage(func, &FxHashMap::default(), owned, &interner)
}

/// Canonical every-arm-funded-store shape: a fresh list `%0` store-aliased on
/// BOTH branch arms (each alias is a FUNDED duplication — `%0` rides the same
/// arm's `Jump` after the store) and Jump-threaded into the dead merge param
/// `%7`.
///
/// ```text
/// bb0: %0 = Construct ListLiteral; Branch %1 -> bb1 | bb2
/// bb1: %2 = %0; %3 = Construct Tuple(%2); Jump bb3(%3, %0)
/// bb2: %4 = %0; %5 = Construct Tuple(%4); Jump bb3(%5, %0)
/// bb3: (%6: used, %7: DEAD); Return %6
/// ```
fn every_arm_funded_store_func() -> ArcFunction {
    func_of(
        vec![
            block(
                0,
                Vec::new(),
                vec![list_construct_of(0)],
                ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            ),
            block(
                1,
                Vec::new(),
                vec![alias_of(2, 0), store_of(3, 2)],
                jump(3, vec![3, 0]),
            ),
            block(
                2,
                Vec::new(),
                vec![alias_of(4, 0), store_of(5, 4)],
                jump(3, vec![5, 0]),
            ),
            block(
                3,
                vec![(ArcVarId::new(6), Idx::INT), (ArcVarId::new(7), Idx::STR)],
                Vec::new(),
                ArcTerminator::Return {
                    value: ArcVarId::new(6),
                },
            ),
        ],
        8,
    )
}

#[test]
fn alt_consumed_admits_every_arm_funded_store_dead_param() {
    let func = every_arm_funded_store_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert_eq!(
        out.releases.get(&3).map(Vec::as_slice),
        Some(&[ArcVarId::new(7)][..]),
        "every lineage consume is FUNDED — the dead merge param owes the RL-5 release: {:?}",
        out.releases
    );
}

#[test]
fn alt_consumed_release_is_releases_only_no_lineage_suppression() {
    let func = every_arm_funded_store_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert!(
        out.suppressed_lineage_vars.is_empty(),
        "ALT-CONSUMED emission is a pure addition — suppressing a funded store \
         inc would double-free the container drop: {:?}",
        out.suppressed_lineage_vars
    );
}

#[test]
fn alt_consumed_declines_partial_funded_direct_consume() {
    // bb2 consumes %0 DIRECTLY (no funded alias): the store transferred the
    // birth reference itself, so the dead-param dec would double-free.
    let func = func_of(
        vec![
            block(
                0,
                Vec::new(),
                vec![list_construct_of(0)],
                ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            ),
            block(
                1,
                Vec::new(),
                vec![alias_of(2, 0), store_of(3, 2)],
                jump(3, vec![3, 0]),
            ),
            block(2, Vec::new(), vec![store_of(5, 0)], jump(3, vec![5, 0])),
            block(
                3,
                vec![(ArcVarId::new(6), Idx::INT), (ArcVarId::new(7), Idx::STR)],
                Vec::new(),
                ArcTerminator::Return {
                    value: ArcVarId::new(6),
                },
            ),
        ],
        8,
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert!(
        out.releases.is_empty(),
        "one UNFUNDED consume declines the whole lineage (leak over double-free): {:?}",
        out.releases
    );
}

#[test]
fn alt_consumed_declines_non_collection_construct_root() {
    // Root-kind FENCE: the same funded-store shape rooted at a Tuple
    // `Construct` (struct-family — partial-move territory) must DECLINE at
    // gate (a); the alt-consumed mode never widens the root gate.
    let mut func = every_arm_funded_store_func();
    func.blocks[0].body[0] = store_of(0, 1); // Tuple Construct root, not ListLiteral
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert!(
        out.releases.is_empty(),
        "a non-sum, non-collection Construct root stays outside gate (a): {:?}",
        out.releases
    );
}

#[test]
fn alt_consumed_declines_lineage_used_past_merge() {
    // A lineage member read in the merge block still observes the allocation
    // the dead-param dec would free at block entry.
    let mut func = every_arm_funded_store_func();
    func.blocks[3].body.push(alias_of(8, 0));
    func.var_types.push(Idx::from_raw(9));
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert!(
        out.releases.is_empty(),
        "a forward use of the lineage past the merge declines the release: {:?}",
        out.releases
    );
}

#[test]
fn alt_consumed_declines_live_same_rep_sibling_param() {
    // The dead param's sibling `%6` is LIVE and fed by the SAME rep — its own
    // release path owns the allocation; a dead-param dec would double it.
    let func = func_of(
        vec![
            block(
                0,
                Vec::new(),
                vec![list_construct_of(0)],
                ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            ),
            block(
                1,
                Vec::new(),
                vec![alias_of(2, 0), store_of(3, 2)],
                jump(3, vec![0, 0]),
            ),
            block(
                2,
                Vec::new(),
                vec![alias_of(4, 0), store_of(5, 4)],
                jump(3, vec![0, 0]),
            ),
            block(
                3,
                vec![(ArcVarId::new(6), Idx::STR), (ArcVarId::new(7), Idx::STR)],
                Vec::new(),
                ArcTerminator::Return {
                    value: ArcVarId::new(6),
                },
            ),
        ],
        8,
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert!(
        out.releases.is_empty(),
        "a live same-rep sibling param declines the dead-param release: {:?}",
        out.releases
    );
}

#[test]
fn alt_consumed_admits_switch_three_arm_with_no_use_arm() {
    // The Switch facet: store arm (funded) + two arms that never consume the
    // lineage; every arm Jump-threads `%0` into the dead merge param.
    let func = func_of(
        vec![
            block(
                0,
                Vec::new(),
                vec![list_construct_of(0)],
                ArcTerminator::Switch {
                    scrutinee: ArcVarId::new(1),
                    cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
                    default: ArcBlockId::new(3),
                },
            ),
            block(
                1,
                Vec::new(),
                vec![alias_of(2, 0), store_of(3, 2)],
                jump(4, vec![3, 0]),
            ),
            block(
                2,
                Vec::new(),
                vec![ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                jump(4, vec![4, 0]),
            ),
            block(
                3,
                Vec::new(),
                vec![ArcInstr::Let {
                    dst: ArcVarId::new(5),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                jump(4, vec![5, 0]),
            ),
            block(
                4,
                vec![(ArcVarId::new(6), Idx::INT), (ArcVarId::new(7), Idx::STR)],
                Vec::new(),
                ArcTerminator::Return {
                    value: ArcVarId::new(6),
                },
            ),
        ],
        8,
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert_eq!(
        out.releases.get(&4).map(Vec::as_slice),
        Some(&[ArcVarId::new(7)][..]),
        "the no-use arms add no consumes; the single funded store admits: {:?}",
        out.releases
    );
    assert!(out.suppressed_lineage_vars.is_empty());
}

#[test]
fn dead_param_single_feeding_rep_fractures_across_loop_back_edge() {
    // Loop-invariant base shape: `%0` enters the loop through a Jump-arg /
    // header-param hop (`bb0: Jump bb1(%0)`), the body stores a funded alias
    // per iteration, and the exit hands the HEADER PARAM `%1` to the dead
    // merge param `%5`. `ForwarderUnionFind` excludes Jump-arg → block-param
    // edges BY DESIGN (a phi over distinct allocations is not a
    // same-allocation relation), so the feeding rep RESOLVES to `%1`'s class —
    // which has FRACTURED from the `Construct` root `%0`: gate (a) cannot see
    // the collection root and the scan declines. Pins the loop-facet boundary
    // the AOT loop cells remain ignored on (BUG-04-180).
    let func = func_of(
        vec![
            block(0, Vec::new(), vec![list_construct_of(0)], jump(1, vec![0])),
            block(
                1,
                vec![(ArcVarId::new(1), Idx::STR)],
                Vec::new(),
                ArcTerminator::Branch {
                    cond: ArcVarId::new(4),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(3),
                },
            ),
            block(
                2,
                Vec::new(),
                vec![alias_of(2, 1), store_of(3, 2)],
                jump(1, vec![1]),
            ),
            block(3, Vec::new(), Vec::new(), jump(4, vec![1])),
            block(
                4,
                vec![(ArcVarId::new(5), Idx::STR)],
                Vec::new(),
                ArcTerminator::Unreachable,
            ),
        ],
        6,
    );
    let mut uf = ForwarderUnionFind::build(&func, &FxHashMap::default());
    let rep = dead_param_single_feeding_rep(&func, &mut uf, 4, ArcVarId::new(5));
    let header_rep = uf.find(ArcVarId::new(1));
    assert_eq!(
        rep,
        Some(header_rep),
        "the exit edge resolves to the single header-param rep"
    );
    let construct_rep = uf.find(ArcVarId::new(0));
    assert_ne!(
        header_rep, construct_rep,
        "the Construct-root identity does NOT survive the Jump-arg hop — the rep fractures"
    );
    assert!(
        !uf.is_fresh_collection_construct_rep(header_rep),
        "the fractured rep is not a collection root — gate (a) declines"
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0), ArcVarId::new(1)].into_iter().collect();
    let out = run_construct_fed(&func, &owned);
    assert!(
        out.releases.is_empty(),
        "the loop-facet dead param stays unreleased (BUG-04-180): {:?}",
        out.releases
    );
}

#[test]
fn sole_carrier_borrowed_invoke_alias_claimed() {
    // bb0: %0 = Invoke @callee() normal bb1; bb1: %2 = %0;
    // Invoke @wrap(%2 [borrow]) — the alias carries the lineage's only
    // release; it must be claimed for the Cat-2 edge release, never an
    // inline pre-call dec.
    let func = func_of(
        vec![
            block(
                0,
                Vec::new(),
                Vec::new(),
                ArcTerminator::Invoke {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    func: Name::from_raw(7),
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(
                1,
                Vec::new(),
                vec![alias_of(2, 0)],
                ArcTerminator::Invoke {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    func: Name::from_raw(8),
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(3),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(2, Vec::new(), Vec::new(), ArcTerminator::Resume),
            block(
                3,
                Vec::new(),
                Vec::new(),
                ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            ),
        ],
        4,
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2)].into_iter().collect();
    let claims = super::compute_sole_carrier_borrowed_invoke_aliases(
        &func,
        &owned,
        &FxHashMap::default(),
        &StringInterner::new(),
    );
    assert_eq!(
        claims.into_iter().collect::<Vec<_>>(),
        vec![ArcVarId::new(2)],
        "the sole-carrier alias is claimed for the edge release"
    );
}

#[test]
fn sole_carrier_claim_declines_forward_live_source() {
    // The source is read again after the borrowed call — the alias is NOT
    // the sole carrier; the inline arrangement stays.
    let func = func_of(
        vec![
            block(
                0,
                Vec::new(),
                Vec::new(),
                ArcTerminator::Invoke {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    func: Name::from_raw(7),
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(
                1,
                Vec::new(),
                vec![alias_of(2, 0)],
                ArcTerminator::Invoke {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    func: Name::from_raw(8),
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(3),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(2, Vec::new(), Vec::new(), ArcTerminator::Resume),
            block(
                3,
                Vec::new(),
                vec![alias_of(1, 0)],
                ArcTerminator::Return {
                    value: ArcVarId::new(1),
                },
            ),
        ],
        4,
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2)].into_iter().collect();
    let claims = super::compute_sole_carrier_borrowed_invoke_aliases(
        &func,
        &owned,
        &FxHashMap::default(),
        &StringInterner::new(),
    );
    assert!(
        claims.is_empty(),
        "a forward-live source declines the claim: {claims:?}"
    );
}
