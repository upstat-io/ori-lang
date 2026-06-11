//! Unit tests for the borrowed-`Invoke`-collection lineage scan: root
//! classification (incl. string-literal decline), gate declines (owned-position
//! consume, no-borrowed-invoke), death-point selection, and the toggle skip.

use super::death_point::{
    choose_dead_param_release_site, choose_death_point, choose_no_sink_carrier,
};
use super::*;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, LitValue,
};
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

fn vv(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn vet_or_panic(f: &ArcFunction, root: ArcVarId) -> FxHashSet<ArcVarId> {
    match same_alloc_closure_vetted(f, root) {
        Some(m) => m,
        None => panic!("closure vetting unexpectedly declined for root {root:?}"),
    }
}

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn block_with_params(
    id: u32,
    params: Vec<(ArcVarId, Idx)>,
    body: Vec<ArcInstr>,
    terminator: ArcTerminator,
) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params,
        body,
        terminator,
    }
}

fn jump(target: u32, args: Vec<ArcVarId>) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args,
    }
}

fn func(n_vars: u32, blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        var_types: (0..n_vars).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks,
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

fn list_construct(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: vv(dst),
        ty: Idx::INT,
        ctor: CtorKind::ListLiteral,
        args: vec![],
    }
}

fn borrowed_invoke(dst: u32, recv: u32, normal: u32, unwind: u32) -> ArcTerminator {
    ArcTerminator::Invoke {
        dst: vv(dst),
        ty: Idx::INT,
        func: Name::from_raw(99),
        args: vec![vv(recv)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        normal: ArcBlockId::new(normal),
        unwind: ArcBlockId::new(unwind),
        mono_instance_id: None,
    }
}

#[test]
fn collect_roots_admits_list_construct_declines_string_literal() {
    // bb0: %0 = Construct List() ; %1 = "lit" ; Unreachable
    let f = func(
        2,
        vec![block(
            0,
            vec![
                list_construct(0),
                ArcInstr::Let {
                    dst: vv(1),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(7))),
                },
            ],
            ArcTerminator::Unreachable,
        )],
    );
    let roots = collect_fresh_collection_construct_roots(&f);
    assert_eq!(
        roots,
        vec![vv(0)],
        "the List Construct is the sole root; the string literal has no Construct definer (declines naturally)",
    );
}

#[test]
fn vetted_closure_declines_owned_position_consume() {
    // %0 = Construct List() ; Apply @f(%0 [own]) ; Unreachable.
    // The owned-position consume transfers the buffer out of family -> decline.
    let f = func(
        2,
        vec![block(
            0,
            vec![
                list_construct(0),
                ArcInstr::Apply {
                    dst: vv(1),
                    ty: Idx::INT,
                    func: Name::from_raw(50),
                    args: vec![vv(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            ArcTerminator::Unreachable,
        )],
    );
    assert!(
        same_alloc_closure_vetted(&f, vv(0)).is_none(),
        "an owned-position Apply consume of the buffer declines the closure (no double-free risk)",
    );
}

#[test]
fn no_borrowed_invoke_arg_is_not_a_candidate() {
    // %0 = Construct List() (only a length Project, no borrowed Invoke) -> gate
    // (c) declines: no carrier whose inline dec is the bug.
    let f = func(
        2,
        vec![block(
            0,
            vec![
                list_construct(0),
                ArcInstr::Project {
                    dst: vv(1),
                    ty: Idx::INT,
                    value: vv(0),
                    field: 0,
                },
            ],
            ArcTerminator::Unreachable,
        )],
    );
    let members: FxHashSet<ArcVarId> = vet_or_panic(&f, vv(0));
    assert!(
        !closure_has_borrowed_invoke_arg(&f, &members),
        "a closure with no borrowed-Invoke arg is not a candidate (gate c)",
    );
}

/// The live-across receiver lineage threaded to a DEAD merge block-param.
/// `bb0: %0 = Construct List() ; %1 = %0 ; Invoke @idx(%1 [borrow]) normal bb1 unwind bb2`;
/// `bb1: Jump bb3(%0)` / `bb2: Resume` / `bb3(%4 dead): Return %3`.
/// Death point = the dead block-param `%4` at `bb3`.
fn live_across_dead_param_func() -> ArcFunction {
    func(
        5,
        vec![
            block(
                0,
                vec![
                    list_construct(0),
                    ArcInstr::Let {
                        dst: vv(1),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                ],
                borrowed_invoke(2, 1, 1, 2),
            ),
            // normal successor: carry %0 to the dead merge param.
            block(1, vec![], jump(3, vec![vv(0)])),
            block(2, vec![], ArcTerminator::Resume),
            // bb3: dead block-param %4 (the death sink), Return a scalar.
            block_with_params(
                3,
                vec![(vv(4), Idx::INT)],
                vec![],
                ArcTerminator::Return { value: vv(3) },
            ),
        ],
    )
}

#[test]
fn lineage_admitted_and_release_placed_at_dead_param() {
    let f = live_across_dead_param_func();
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let claimed = FxHashSet::default();
    let live_extract = FxHashSet::default();
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &claimed,
        &live_extract,
        &FxHashMap::default(),
    );
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0))
            && out.suppressed_lineage_vars.contains(&vv(1)),
        "the fresh-collection borrowed-Invoke closure {{%0, %1, %4}} is suppressed",
    );
    let placed: Vec<ArcVarId> = out.releases.values().flatten().copied().collect();
    assert_eq!(
        placed,
        vec![vv(4)],
        "EXACTLY ONE death-point release placed on the dead block-param %4",
    );
}

#[test]
fn construct_fed_claimed_root_declines() {
    // Same shape, but the root is already claimed by the construct-fed dead-param
    // family -> gate (b) declines (no double-suppression).
    let f = live_across_dead_param_func();
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let mut claimed: FxHashSet<ArcVarId> = FxHashSet::default();
    claimed.insert(vv(0));
    let live_extract = FxHashSet::default();
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &claimed,
        &live_extract,
        &FxHashMap::default(),
    );
    assert!(
        out.suppressed_lineage_vars.is_empty() && out.releases.is_empty(),
        "a root already claimed by the construct-fed dead-param family declines (gate b)",
    );
}

#[test]
fn live_extract_claimed_member_declines_whole_closure() {
    // The fresh-sum live-extract scan (SSOT for the niche-family-sum
    // match-extract RESULT lineage) already claimed a MEMBER of this closure
    // (`%1`, the Let-Var alias — NOT the root). Gate (b') declines the whole
    // closure at member grain: admitting it would place a second death-point
    // release on the allocation the live-extract scan already released
    // (double-free). The root `%0` is NOT in the live-extract set; only `%1`
    // is — proving member-grain (not root-only) disjointness.
    let f = live_across_dead_param_func();
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let claimed = FxHashSet::default();
    let mut live_extract: FxHashSet<ArcVarId> = FxHashSet::default();
    live_extract.insert(vv(1));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &claimed,
        &live_extract,
        &FxHashMap::default(),
    );
    assert!(
        out.suppressed_lineage_vars.is_empty() && out.releases.is_empty(),
        "a closure overlapping the live-extract claimed web declines at member grain (gate b')",
    );
}

/// A block-param fed by the lineage root on one arm and a DIFFERENT
/// allocation on a sibling arm is a phi merging distinct allocations. The
/// fixpoint validation DECLINES the WHOLE closure (`None`) rather than grow
/// across the allocation boundary — the conservative cure for the
/// `04B.2-cross-class-uaf` shape (a foreign-allocation merge would mis-suppress
/// / place a release on a merged-in foreign allocation).
///
/// `bb0: %0 = Construct List() ; %1 = Construct List() (distinct) ;
///       Branch %5 -> bb1, bb2`
/// `bb1: Jump bb3(%0)` / `bb2: Jump bb3(%1)`
/// `bb3(%4): Return %3` — `%4` merges `%0` (member) and `%1` (foreign).
#[test]
fn phi_merge_distinct_allocations_declines_closure() {
    let f = func(
        6,
        vec![
            block(
                0,
                vec![list_construct(0), list_construct(1)],
                ArcTerminator::Branch {
                    cond: vv(5),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            ),
            block(1, vec![], jump(3, vec![vv(0)])),
            block(2, vec![], jump(3, vec![vv(1)])),
            block_with_params(
                3,
                vec![(vv(4), Idx::INT)],
                vec![],
                ArcTerminator::Return { value: vv(3) },
            ),
        ],
    );
    assert!(
        same_alloc_closure_vetted(&f, vv(0)).is_none(),
        "the merge param %4 is fed by foreign alloc %1 on the sibling arm — the \
         fixpoint phi-merge validation declines the whole closure (no over-grow \
         across the allocation boundary)",
    );
    // Symmetric: a root grown from the FOREIGN alloc %1 also declines.
    assert!(
        same_alloc_closure_vetted(&f, vv(1)).is_none(),
        "the same phi-merge declines the %1 closure too (symmetric guard)",
    );
}

/// A block-param ALL of whose predecessors pass a same-alloc member is a
/// legitimate same-allocation merge (the loop-carried / single-arm carry case).
/// The fixpoint validation admits it and the param joins the closure.
/// `bb0: %0 = Construct List() ; Jump bb1(%0)` / `bb1(%1): Return %2` — the sole
/// predecessor passes the member, so %1 is the same allocation.
#[test]
fn single_predecessor_member_param_joins_closure() {
    let f = func(
        3,
        vec![
            block(0, vec![list_construct(0)], jump(1, vec![vv(0)])),
            block_with_params(
                1,
                vec![(vv(1), Idx::INT)],
                vec![],
                ArcTerminator::Return { value: vv(2) },
            ),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    assert!(
        members.contains(&vv(1)),
        "the sole predecessor passes member %0 at the param position — %1 is the \
         same allocation and joins the closure",
    );
}

#[test]
fn no_dead_param_dead_param_helper_returns_none() {
    // The receiver dies at the borrowed Invoke (no Jump to a dead merge param) —
    // the DEAD-PARAM helper finds no dead-block-param sink. The no-sink mode
    // (`choose_no_sink_carrier`) takes over in `choose_death_point` — see
    // `no_sink_minimal_classified_as_edge_death`.
    let f = no_sink_minimal_func();
    let used = function_used_vars(&f);
    let members = vet_or_panic(&f, vv(0));
    assert!(
        choose_dead_param_release_site(&f, &members, &used).is_none(),
        "no dead block-param sink -> the dead-param helper returns None",
    );
}

/// The no-sink minimal: `bb0: %0 = Construct List() ;
/// %1 = %0 ; Invoke @is_ref(%1 [borrow]) normal bb1 unwind bb2`; `bb1: Return %3`
/// `bb2: Resume`. The receiver `%0`/`%1` dies on the bb1/bb2 edges directly with
/// NO dead-param sink — the no-sink edge-death case.
fn no_sink_minimal_func() -> ArcFunction {
    func(
        4,
        vec![
            block(
                0,
                vec![
                    list_construct(0),
                    ArcInstr::Let {
                        dst: vv(1),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                ],
                borrowed_invoke(2, 1, 1, 2),
            ),
            block(1, vec![], ArcTerminator::Return { value: vv(3) }),
            block(2, vec![], ArcTerminator::Resume),
        ],
    )
}

#[test]
fn no_sink_minimal_classified_as_edge_death() {
    // `choose_no_sink_carrier` finds the borrowed-Invoke carrier `%1` (the
    // member at a borrowed Invoke arg, may-unwind, execution-final). The lineage
    // enters NO-SINK mode and the carrier is claimed for Category-2.
    let f = no_sink_minimal_func();
    let members = vet_or_panic(&f, vv(0));
    assert_eq!(
        choose_no_sink_carrier(&f, &members),
        Some(vv(1)),
        "the borrowed-Invoke arg %1 is the no-sink edge-death carrier",
    );
}

#[test]
fn no_sink_lineage_suppresses_and_claims_carrier() {
    // End-to-end: the no-sink minimal closure is suppressed (no inline dec /
    // dup inc) AND the carrier `%1` is claimed for the Cat-2 per-edge release.
    // No dead-param `releases` are placed (Cat-2 owns the per-edge dec).
    let f = no_sink_minimal_func();
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let claimed = FxHashSet::default();
    let live_extract = FxHashSet::default();
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &claimed,
        &live_extract,
        &FxHashMap::default(),
    );
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0))
            && out.suppressed_lineage_vars.contains(&vv(1)),
        "the no-sink closure {{%0, %1}} is suppressed (inline dec + dup inc killed)",
    );
    assert!(
        out.releases.is_empty(),
        "no dead-param release is placed — Cat-2 owns the per-edge release",
    );
    assert_eq!(
        out.claimed_no_sink_vars.iter().copied().collect::<Vec<_>>(),
        vec![vv(1)],
        "EXACTLY the carrier %1 is claimed for the Cat-2 deadAtSucc per-edge release",
    );
}

#[test]
fn no_sink_self_loop_carrier_declines() {
    // A carrier whose Invoke self-loops (the carrier block is its own successor,
    // a CFG cycle) declines — a re-reached per-edge dec double-frees. Gate n1.
    // `bb0: %0 = Construct ; %1 = %0 ; Invoke @f(%1 [borrow]) normal bb0 unwind bb1`.
    let f = func(
        4,
        vec![
            block(
                0,
                vec![
                    list_construct(0),
                    ArcInstr::Let {
                        dst: vv(1),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                ],
                borrowed_invoke(2, 1, 0, 1),
            ),
            block(1, vec![], ArcTerminator::Resume),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    assert!(
        choose_no_sink_carrier(&f, &members).is_none(),
        "a carrier whose Invoke re-reaches its own block declines (CFG cycle, gate n1)",
    );
}

#[test]
fn no_sink_live_across_carrier_is_last_invoke() {
    // Live-across receiver: `%0` read at a FIRST borrowed Invoke (`@f`), aliased
    // to `%5`, then read at a SECOND borrowed Invoke (`@len`). The execution-final
    // carrier is the SECOND (last) Invoke's arg `%5` — every member-read block
    // (incl. bb0) forward-reaches the last carrier block. The release lands at the
    // post-call last read.
    // bb0: %0 = Construct ; %1 = %0 ; Invoke @f(%1 [borrow]) normal bb1 unwind bb2
    // bb1: %5 = %0 ; Invoke @len(%5 [borrow]) normal bb3 unwind bb4
    // bb2/bb4: Resume ; bb3: Return %6
    let f = func(
        7,
        vec![
            block(
                0,
                vec![
                    list_construct(0),
                    ArcInstr::Let {
                        dst: vv(1),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                ],
                borrowed_invoke(3, 1, 1, 2),
            ),
            block(
                1,
                vec![ArcInstr::Let {
                    dst: vv(5),
                    ty: Idx::INT,
                    value: ArcValue::Var(vv(0)),
                }],
                borrowed_invoke(4, 5, 3, 4),
            ),
            block(2, vec![], ArcTerminator::Resume),
            block(3, vec![], ArcTerminator::Return { value: vv(6) }),
            block(4, vec![], ArcTerminator::Resume),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    assert_eq!(
        choose_no_sink_carrier(&f, &members),
        Some(vv(5)),
        "the execution-final carrier is the LAST borrowed Invoke's arg %5 (the \
         post-call last read), not the first %1",
    );
}

#[test]
fn no_sink_disabled_toggle_skips_treatment() {
    // The `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE=1` toggle gates the whole
    // scan (dead-param + no-sink modes). The accessor is the single switch
    // `scan_helpers::apply_borrowed_invoke_collection_lineage` consults; unset in
    // the test environment it returns false (treatment active).
    assert!(
        !borrowed_invoke_lineage_release_disabled(),
        "the toggle is unset in the test env -> the no-sink treatment is active",
    );
}

#[test]
fn choose_death_point_dispatch_no_sink_fallback_and_gate() {
    // Dispatch order: dead-param site wins when present; otherwise the no-sink
    // carrier ONLY when `allow_no_sink` (fresh-collection roots); else None.
    let f = no_sink_minimal_func();
    let members = vet_or_panic(&f, vv(0));
    let used: FxHashSet<ArcVarId> = members.clone();

    match choose_death_point(&f, &members, &used, true) {
        Some(DeathPoint::NoSink { claim }) => assert_eq!(claim, vv(1)),
        other => panic!("expected the no-sink carrier claim, got {other:?}"),
    }
    assert_eq!(
        choose_death_point(&f, &members, &used, false),
        None,
        "result-root lineages (allow_no_sink=false) take NO death point",
    );
}
