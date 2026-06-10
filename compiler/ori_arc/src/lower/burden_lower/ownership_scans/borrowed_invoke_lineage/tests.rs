//! Unit tests for the borrowed-`Invoke`-collection lineage scan: root
//! classification (incl. string-literal decline), gate declines (owned-position
//! consume, no-borrowed-invoke), death-point selection, and the toggle skip.

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
        "the List Construct is the sole root; the string literal has no Construct definer (declines naturally per the re-consensus)",
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

#[test]
fn no_dead_param_death_point_declines() {
    // The receiver dies at the borrowed Invoke (no Jump to a dead merge param) —
    // there is no dead-block-param sink, so gate (e) declines (the Surface-1
    // edge conjunct owns the per-edge release for that shape).
    let f = func(
        3,
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
            block(1, vec![], ArcTerminator::Unreachable),
            block(2, vec![], ArcTerminator::Resume),
        ],
    );
    let used = function_used_vars(&f);
    let members = vet_or_panic(&f, vv(0));
    assert!(
        choose_dead_param_release_site(&f, &members, &used).is_none(),
        "no dead block-param sink -> no death-point site (Surface 1 owns the release)",
    );
}
