//! Unit tests for the borrowed-`Invoke`-collection lineage scan: root
//! classification (incl. string-literal decline), gate declines (owned-position
//! consume, no-borrowed-invoke), death-point selection, and the toggle skip.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use super::death_point::{
    choose_dead_param_release_site, choose_death_point, choose_no_sink_carrier, DeathPointModes,
};
use super::*;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, LitValue, ValueRepr,
};

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
        var_reprs: (0..n_vars).map(|_| ValueRepr::Scalar).collect(),
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

fn enum_variant_construct(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: vv(dst),
        ty: Idx::INT,
        ctor: CtorKind::EnumVariant {
            enum_name: Name::from_raw(11),
            variant: 0,
        },
        args: vec![],
    }
}

fn struct_construct(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: vv(dst),
        ty: Idx::INT,
        ctor: CtorKind::Struct(Name::from_raw(12)),
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
    let (roots, _) = collect_fresh_construct_roots(&f, &FxHashSet::default());
    assert_eq!(
        roots,
        vec![vv(0)],
        "the List Construct is the sole root; the string literal has no Construct definer (declines naturally)",
    );
}

#[test]
fn collect_roots_admits_owned_enum_variant_construct() {
    // bb0: %0 = Construct EnumVariant(..) ; %1 = Construct Struct(..) ; Unreachable.
    // BOTH dsts are admitted when in owned_vars_needing_rc — the EnumVariant arm
    // and the plain-Struct arm share the heap-burden membership discriminator (a
    // let-bound user-drop struct key borrowed by a map-insert Invoke terminator is
    // the failing cell that admitted the Struct family).
    let f = func(
        2,
        vec![block(
            0,
            vec![enum_variant_construct(0), struct_construct(1)],
            ArcTerminator::Unreachable,
        )],
    );
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let (roots, struct_roots) = collect_fresh_construct_roots(&f, &owned);
    assert_eq!(
        roots,
        vec![vv(0), vv(1)],
        "the owned EnumVariant AND the owned plain-Struct Constructs are admitted",
    );
    assert!(
        struct_roots.contains(&vv(1)) && !struct_roots.contains(&vv(0)),
        "only the plain-Struct root carries the struct tag",
    );
}

#[test]
fn collect_roots_declines_struct_construct_without_rc_burden() {
    // A plain Struct Construct whose dst is NOT in owned_vars_needing_rc (an
    // all-scalar struct with no RC-bearing field) carries no heap burden — the
    // membership gate declines it, bounding the Struct-arm admission.
    let f = func(
        1,
        vec![block(
            0,
            vec![struct_construct(0)],
            ArcTerminator::Unreachable,
        )],
    );
    let (roots, _) = collect_fresh_construct_roots(&f, &FxHashSet::default());
    assert!(
        roots.is_empty(),
        "a Struct Construct absent from owned_vars_needing_rc is not a root",
    );
}

#[test]
fn collect_roots_declines_scalar_payload_enum_variant() {
    // An EnumVariant Construct whose dst is NOT in owned_vars_needing_rc (an
    // all-scalar-payload sum like Option<int>, niche-packed, no RC header) is
    // declined -> keeps the treatment scoped to genuine heap lineages.
    let f = func(
        1,
        vec![block(
            0,
            vec![enum_variant_construct(0)],
            ArcTerminator::Unreachable,
        )],
    );
    let (roots, _) = collect_fresh_construct_roots(&f, &FxHashSet::default());
    assert!(
        roots.is_empty(),
        "an EnumVariant Construct absent from owned_vars_needing_rc is not a root",
    );
}

/// An `EnumVariant` root borrowed into a may-unwind Invoke dying on the call edges
/// with no dead-param sink is admitted in NO-SINK mode and the carrier is claimed
/// for the Cat-2 per-edge release.
#[test]
fn enum_variant_root_no_sink_admitted_and_claimed() {
    // bb0: %0 = Construct EnumVariant(..) ; %1 = %0 ;
    //      Invoke @f(%1 [borrow]) normal bb1 unwind bb2
    // bb1: Return %3 ; bb2: Resume.
    let f = func(
        4,
        vec![
            block(
                0,
                vec![
                    enum_variant_construct(0),
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
    );
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &ori_ir::StringInterner::new(),
    );
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0))
            && out.suppressed_lineage_vars.contains(&vv(1)),
        "the EnumVariant no-sink closure {{%0, %1}} is suppressed",
    );
    assert_eq!(
        out.claimed_no_sink_vars.iter().copied().collect::<Vec<_>>(),
        vec![vv(1)],
        "EXACTLY the carrier %1 is claimed for the Cat-2 per-edge release",
    );
    assert!(
        out.releases.is_empty(),
        "no dead-param release is placed — Cat-2 owns the per-edge release",
    );
}

/// A plain Struct root with an RC burden reaching the borrowed-`Invoke` shape
/// IS a candidate: the scan suppresses the lineage and takes a death-point
/// treatment (the no-sink claim for this sink-less shape) instead of leaving
/// the premature inline `[AggFields]` dec in place.
#[test]
fn struct_root_admitted_with_rc_burden() {
    let f = func(
        4,
        vec![
            block(
                0,
                vec![
                    struct_construct(0),
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
    );
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &ori_ir::StringInterner::new(),
    );
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0))
            && out.suppressed_lineage_vars.contains(&vv(1)),
        "the owned Struct lineage is suppressed (premature inline dec removed)",
    );
    assert!(
        !out.claimed_no_sink_vars.is_empty() || !out.releases.is_empty(),
        "the struct lineage takes a death-point treatment (no-sink claim or placed release)",
    );
}

/// A live Project-extract of a member (a same-alloc view read AFTER the carrier
/// in a different block) forces the no-sink mode to DECLINE — the extract would
/// be double-freed by an edge release. `choose_death_point` returns None (no
/// dead-param sink either).
#[test]
fn live_project_extract_declines_no_sink() {
    // bb0: %0 = Construct EnumVariant(..) ; %1 = %0 ; %4 = Project %0.0 (extract) ;
    //      Invoke @f(%1 [borrow]) normal bb1 unwind bb2
    // bb1: Return %4 (the extract LIVE-ACROSS the carrier) ; bb2: Resume.
    let f = func(
        5,
        vec![
            block(
                0,
                vec![
                    enum_variant_construct(0),
                    ArcInstr::Let {
                        dst: vv(1),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                    ArcInstr::Project {
                        dst: vv(4),
                        ty: Idx::INT,
                        value: vv(0),
                        field: 0,
                    },
                ],
                borrowed_invoke(2, 1, 1, 2),
            ),
            block(1, vec![], ArcTerminator::Return { value: vv(4) }),
            block(2, vec![], ArcTerminator::Resume),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: false,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "a live Project-extract of a member declines no-sink (extract live-across \
         the carrier release edge would double-free)",
    );
}

#[test]
fn live_project_of_project_chain_declines_no_sink() {
    // Project-of-Project: %4 = Project %0.0 (extract); %5 = Project %4.0
    // (extract OF the extract — same allocation tree); only the GRAND-extract
    // %5 is live across the carrier. The closure must follow the chain.
    let f = func(
        6,
        vec![
            block(
                0,
                vec![
                    enum_variant_construct(0),
                    ArcInstr::Let {
                        dst: vv(1),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                    ArcInstr::Project {
                        dst: vv(4),
                        ty: Idx::INT,
                        value: vv(0),
                        field: 0,
                    },
                    ArcInstr::Project {
                        dst: vv(5),
                        ty: Idx::INT,
                        value: vv(4),
                        field: 0,
                    },
                ],
                borrowed_invoke(2, 1, 1, 2),
            ),
            block(1, vec![], ArcTerminator::Return { value: vv(5) }),
            block(2, vec![], ArcTerminator::Resume),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: false,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "a live Project-of-Project grand-extract declines no-sink (the chain \
         views the same allocation tree)",
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
        &ori_ir::StringInterner::new(),
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
        &ori_ir::StringInterner::new(),
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
        &ori_ir::StringInterner::new(),
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
        choose_no_sink_carrier(&f, &members, &ori_ir::StringInterner::new()),
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
        &ori_ir::StringInterner::new(),
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
        choose_no_sink_carrier(&f, &members, &ori_ir::StringInterner::new()).is_none(),
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
        choose_no_sink_carrier(&f, &members, &ori_ir::StringInterner::new()),
        Some(vv(5)),
        "the execution-final carrier is the LAST borrowed Invoke's arg %5 (the \
         post-call last read), not the first %1",
    );
}

/// `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE` accessor: unset in the test env
/// -> the whole scan (dead-param + no-sink modes) is active. (Behavioral toggle
/// parity — the disabled path actually reproducing the pre-cure leak — is
/// pinned at the AOT level in `borrowed_invoke_leak.rs`, not here: the accessor
/// is a `LazyLock<bool>` read once per process, so flipping the env var mid-run
/// inside this unit-test binary cannot observe a behavioral difference.)
#[test]
fn borrowed_invoke_lineage_release_toggle_unset_is_active() {
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

    match choose_death_point(
        &f,
        &members,
        &used,
        vv(0),
        DeathPointModes {
            no_sink: true,
            loop_exit: false,
            carrier_succ: false,
        },
        &ori_ir::StringInterner::new(),
    ) {
        Some(DeathPoint::NoSink { claim }) => assert_eq!(claim, vv(1)),
        other => panic!("expected the no-sink carrier claim, got {other:?}"),
    }
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: false,
                loop_exit: false,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "result-root lineages (allow_no_sink=false) take NO death point",
    );
}

#[test]
fn no_sink_declines_heap_result_carrier() {
    // The borrowing Invoke's dst is a heap value (RcPointer) — a possible
    // same-allocation VIEW of the carrier (a slice). Suppressing the carrier's
    // inline pair while the per-edge probe sees the view live at the successor
    // releases NOTHING; the no-sink claim must decline (scalar results only).
    let mut f = no_sink_minimal_func();
    f.var_reprs[2] = ValueRepr::RcPointer;
    let members = vet_or_panic(&f, vv(0));
    assert_eq!(
        choose_no_sink_carrier(&f, &members, &ori_ir::StringInterner::new()),
        None,
        "a heap-result borrowing Invoke may return a same-alloc view; decline",
    );
}

/// LOOP-EXIT mode fixture: a loop-INVARIANT fresh list (`%0`, defined before
/// the loop) borrow-read each iteration via a per-iteration alias (`%2 = %0`)
/// at a may-unwind borrowed `Invoke`; the lineage dies once, at the loop's
/// sole non-unwind exit (bb5).
///
/// ```text
/// bb0: %0 = Construct List()    Jump bb1
/// bb1: (header)                 Branch %1 ? bb2 : bb5
/// bb2: %2 = %0                  Invoke @f(%2 [borrow]) -> %3, normal bb3 unwind bb4
/// bb3: (latch)                  Jump bb1
/// bb4:                          Resume
/// bb5: (loop exit)              Return %1
/// ```
fn loop_invariant_borrowed_func() -> ArcFunction {
    func(
        4,
        vec![
            block(0, vec![list_construct(0)], jump(1, vec![])),
            block(
                1,
                vec![],
                ArcTerminator::Branch {
                    cond: vv(1),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(5),
                },
            ),
            block(
                2,
                vec![ArcInstr::Let {
                    dst: vv(2),
                    ty: Idx::INT,
                    value: ArcValue::Var(vv(0)),
                }],
                borrowed_invoke(3, 2, 3, 4),
            ),
            block(3, vec![], jump(1, vec![])),
            block(4, vec![], ArcTerminator::Resume),
            block(5, vec![], ArcTerminator::Return { value: vv(1) }),
        ],
    )
}

#[test]
fn loop_invariant_borrowed_lineage_takes_loop_exit_release() {
    let f = loop_invariant_borrowed_func();
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: true,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        Some(DeathPoint::LoopExit {
            site_block: 5,
            site_pos: ForwarderReleasePos::BlockEntry,
            dec_var: vv(0),
        }),
        "a loop-invariant borrowed-collection lineage releases ONCE at the \
         loop's sole non-unwind exit, never per iteration",
    );
}

#[test]
fn loop_exit_mode_disabled_flag_declines() {
    let f = loop_invariant_borrowed_func();
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: false,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "with allow_loop_exit off the loop-borrowed shape takes NO death point",
    );
}

#[test]
fn per_iteration_fresh_root_declines_loop_exit() {
    // The root Construct sits INSIDE the loop (per-iteration fresh buffer):
    // one exit release would leak every earlier iteration's instance. (l3).
    let f = func(
        4,
        vec![
            block(0, vec![], jump(1, vec![])),
            block(
                1,
                vec![],
                ArcTerminator::Branch {
                    cond: vv(1),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(5),
                },
            ),
            block(
                2,
                vec![
                    list_construct(0),
                    ArcInstr::Let {
                        dst: vv(2),
                        ty: Idx::INT,
                        value: ArcValue::Var(vv(0)),
                    },
                ],
                borrowed_invoke(3, 2, 3, 4),
            ),
            block(3, vec![], jump(1, vec![])),
            block(4, vec![], ArcTerminator::Resume),
            block(5, vec![], ArcTerminator::Return { value: vv(1) }),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: true,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "a per-iteration fresh root declines loop-exit placement (l3)",
    );
}

#[test]
fn post_loop_member_read_declines_loop_exit() {
    // A member is borrow-read AFTER the loop (bb5 length Project): the read
    // block is outside the cycle, so an exit-entry release would UAF. (l1/l2).
    let f = func(
        5,
        vec![
            block(0, vec![list_construct(0)], jump(1, vec![])),
            block(
                1,
                vec![],
                ArcTerminator::Branch {
                    cond: vv(1),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(5),
                },
            ),
            block(
                2,
                vec![ArcInstr::Let {
                    dst: vv(2),
                    ty: Idx::INT,
                    value: ArcValue::Var(vv(0)),
                }],
                borrowed_invoke(3, 2, 3, 4),
            ),
            block(3, vec![], jump(1, vec![])),
            block(4, vec![], ArcTerminator::Resume),
            block(
                5,
                vec![ArcInstr::Project {
                    dst: vv(4),
                    ty: Idx::INT,
                    value: vv(0),
                    field: 0,
                }],
                ArcTerminator::Return { value: vv(4) },
            ),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: true,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "a post-loop member read declines loop-exit placement (read outside \
         the cycle would UAF after the exit-entry release)",
    );
}

#[test]
fn two_exit_loop_declines_loop_exit() {
    // The cycle has TWO non-unwind exits (bb5 via the header, bb6 via a break
    // branch in the latch): a fork the single release cannot cover. (l4).
    let f = func(
        4,
        vec![
            block(0, vec![list_construct(0)], jump(1, vec![])),
            block(
                1,
                vec![],
                ArcTerminator::Branch {
                    cond: vv(1),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(5),
                },
            ),
            block(
                2,
                vec![ArcInstr::Let {
                    dst: vv(2),
                    ty: Idx::INT,
                    value: ArcValue::Var(vv(0)),
                }],
                borrowed_invoke(3, 2, 3, 4),
            ),
            block(
                3,
                vec![],
                ArcTerminator::Branch {
                    cond: vv(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(6),
                },
            ),
            block(4, vec![], ArcTerminator::Resume),
            block(5, vec![], ArcTerminator::Return { value: vv(1) }),
            block(6, vec![], ArcTerminator::Return { value: vv(1) }),
        ],
    );
    let members = vet_or_panic(&f, vv(0));
    let used = function_used_vars(&f);
    assert_eq!(
        choose_death_point(
            &f,
            &members,
            &used,
            vv(0),
            DeathPointModes {
                no_sink: true,
                loop_exit: true,
                carrier_succ: false,
            },
            &ori_ir::StringInterner::new(),
        ),
        None,
        "a two-exit loop declines loop-exit placement (l4 fork)",
    );
}

// Third root family: self-allocating-builtin Invoke results (carrier-succ)

/// A may-unwind `Invoke` whose callee `Name` + per-arg ownership are caller-chosen
/// (the builtin-family fixtures need a REAL interned builtin name).
fn named_invoke(dst: u32, callee: Name, args: Vec<u32>, normal: u32, unwind: u32) -> ArcTerminator {
    let arg_ownership = vec![ArgOwnership::Borrowed; args.len()];
    ArcTerminator::Invoke {
        dst: vv(dst),
        ty: Idx::STR,
        func: callee,
        args: args.into_iter().map(vv).collect(),
        arg_ownership,
        normal: ArcBlockId::new(normal),
        unwind: ArcBlockId::new(unwind),
        mono_instance_id: None,
    }
}

/// The template-chain link fixture: `%0 = Invoke @concat()` (root birth), `%1 =
/// Invoke @to_str()` (intervening may-unwind — `%0` live across), `%2 = Invoke
/// @concat(%0 [borrow], %1 [borrow])` (the consuming carrier), `Return %3`.
/// Heap reprs on the str results; `%3` stays Scalar.
fn template_chain_link_func(interner: &ori_ir::StringInterner) -> ArcFunction {
    let concat = interner.intern("concat");
    let to_str = interner.intern("to_str");
    let mut f = func(
        4,
        vec![
            block(0, vec![], named_invoke(0, concat, vec![], 1, 4)),
            block(1, vec![], named_invoke(1, to_str, vec![], 2, 4)),
            block(2, vec![], named_invoke(2, concat, vec![0, 1], 3, 4)),
            block(3, vec![], ArcTerminator::Return { value: vv(3) }),
            block(4, vec![], ArcTerminator::Resume),
        ],
    );
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    f.var_reprs[2] = ValueRepr::FatValue;
    f
}

/// POSITIVE: the concat-link root `%0` (live across the `@to_str` Invoke,
/// consumed at the later borrowed `@concat`) is admitted in CARRIER-SUCC mode —
/// suppressed lineage + exactly ONE release at the consuming Invoke's
/// NORMAL-successor entry (bb3).
#[test]
fn builtin_invoke_result_live_across_to_str_takes_carrier_succ_release() {
    let interner = ori_ir::StringInterner::new();
    let f = template_chain_link_func(&interner);
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0)),
        "the concat-result root %0 is suppressed (no inline pre-call dec, no fresh-site inc)",
    );
    assert_eq!(
        out.releases
            .get(&(3, ForwarderReleasePos::BlockEntry))
            .map(Vec::as_slice),
        Some([vv(0)].as_slice()),
        "EXACTLY ONE release placed at the consuming Invoke's normal-successor entry (bb3)",
    );
}

/// DECLINE: the consuming Invoke's result is heap-repr'd and its callee is NOT
/// a known self-allocating builtin (an unknown user callee could return a
/// same-allocation VIEW of the borrowed member — `@slice`/`@substring` shape);
/// a release at the successor would free the buffer the view still holds.
#[test]
fn builtin_invoke_result_declines_unknown_heap_result_consumer() {
    let interner = ori_ir::StringInterner::new();
    let concat = interner.intern("concat");
    let user_fn = interner.intern("user_view");
    let mut f = func(
        4,
        vec![
            block(0, vec![], named_invoke(0, concat, vec![], 1, 4)),
            block(1, vec![], named_invoke(1, concat, vec![], 2, 4)),
            block(2, vec![], named_invoke(2, user_fn, vec![0, 1], 3, 4)),
            block(3, vec![], ArcTerminator::Return { value: vv(3) }),
            block(4, vec![], ArcTerminator::Resume),
        ],
    );
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    f.var_reprs[2] = ValueRepr::FatValue;
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        !out.suppressed_lineage_vars.contains(&vv(0)) && out.releases.is_empty(),
        "a heap-result consumer with an unknown callee declines the carrier-succ mode",
    );
}

/// DECLINE: a member is read in the carrier's normal successor itself — the
/// entry release would precede the read (use-after-free).
#[test]
fn builtin_invoke_result_declines_member_read_at_release_site() {
    let interner = ori_ir::StringInterner::new();
    let concat = interner.intern("concat");
    let mut f = func(
        4,
        vec![
            block(0, vec![], named_invoke(0, concat, vec![], 1, 4)),
            block(1, vec![], named_invoke(1, concat, vec![0], 2, 4)),
            block(
                2,
                vec![ArcInstr::Apply {
                    dst: vv(3),
                    ty: Idx::INT,
                    func: Name::from_raw(77),
                    args: vec![vv(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                }],
                ArcTerminator::Return { value: vv(3) },
            ),
            block(4, vec![], ArcTerminator::Resume),
        ],
    );
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        !out.suppressed_lineage_vars.contains(&vv(0)) && out.releases.is_empty(),
        "a member read at the release site declines the carrier-succ mode",
    );
}

/// DECLINE: the carrier's normal successor has a second predecessor — the
/// release would fire on a path that never passed the borrowed read.
#[test]
fn builtin_invoke_result_declines_multi_pred_release_site() {
    let interner = ori_ir::StringInterner::new();
    let concat = interner.intern("concat");
    let mut f = func(
        4,
        vec![
            block(
                0,
                vec![ArcInstr::Let {
                    dst: vv(3),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                ArcTerminator::Branch {
                    cond: vv(3),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            ),
            block(1, vec![], named_invoke(0, concat, vec![], 5, 4)),
            block(2, vec![], jump(3, vec![])),
            block(3, vec![], ArcTerminator::Return { value: vv(3) }),
            block(4, vec![], ArcTerminator::Resume),
            // bb5: the carrier — its normal successor bb3 is ALSO jumped to by bb2.
            block(5, vec![], named_invoke(1, concat, vec![0], 3, 4)),
        ],
    );
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        !out.suppressed_lineage_vars.contains(&vv(0)) && out.releases.is_empty(),
        "a multi-predecessor release site declines the carrier-succ mode",
    );
}

/// POSITIVE: a per-iteration root inside a loop body — every re-reach of the
/// release site passes the root's defining `Invoke` (re-birth), so each
/// iteration's release pairs with that iteration's fresh allocation.
#[test]
fn builtin_invoke_result_per_iteration_loop_root_admitted() {
    let interner = ori_ir::StringInterner::new();
    let concat = interner.intern("concat");
    let mut f = func(
        4,
        vec![
            block(0, vec![], jump(1, vec![])),
            // bb1: root re-birth each iteration.
            block(1, vec![], named_invoke(0, concat, vec![], 2, 5)),
            // bb2: the consuming carrier.
            block(2, vec![], named_invoke(1, concat, vec![0], 3, 5)),
            // bb3 (release site): loop back-edge THROUGH the re-birth bb1.
            block(
                3,
                vec![ArcInstr::Let {
                    dst: vv(3),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                ArcTerminator::Branch {
                    cond: vv(3),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(4),
                },
            ),
            block(4, vec![], ArcTerminator::Return { value: vv(3) }),
            block(5, vec![], ArcTerminator::Resume),
        ],
    );
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0)),
        "the per-iteration loop root is suppressed",
    );
    assert_eq!(
        out.releases
            .get(&(3, ForwarderReleasePos::BlockEntry))
            .map(Vec::as_slice),
        Some([vv(0)].as_slice()),
        "the per-iteration release lands at the carrier's normal successor inside the loop",
    );
}

/// DECLINE: a loop-INVARIANT root (defined before the loop) consumed by an
/// in-loop carrier — the release site is re-reached WITHOUT passing the
/// root's re-birth, so a second iteration would release the already-released
/// allocation (double-free).
#[test]
fn builtin_invoke_result_declines_loop_invariant_root_in_loop_carrier() {
    let interner = ori_ir::StringInterner::new();
    let concat = interner.intern("concat");
    let mut f = func(
        4,
        vec![
            // bb0: root born ONCE, before the loop.
            block(0, vec![], named_invoke(0, concat, vec![], 1, 5)),
            // bb1: in-loop consuming carrier.
            block(1, vec![], named_invoke(1, concat, vec![0], 2, 5)),
            // bb2 (release site): back-edge re-reaches bb1 + bb2 WITHOUT bb0.
            block(
                2,
                vec![ArcInstr::Let {
                    dst: vv(3),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                ArcTerminator::Branch {
                    cond: vv(3),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(3),
                },
            ),
            block(3, vec![], ArcTerminator::Return { value: vv(3) }),
            block(5, vec![], ArcTerminator::Resume),
        ],
    );
    // Block ids must be dense for index math: rebuild ids 0..=4 with Resume last.
    f.blocks[4].id = ArcBlockId::new(4);
    let fix_unwind = |t: &mut ArcTerminator| {
        if let ArcTerminator::Invoke { unwind, .. } = t {
            *unwind = ArcBlockId::new(4);
        }
    };
    fix_unwind(&mut f.blocks[0].terminator);
    fix_unwind(&mut f.blocks[1].terminator);
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    let out = compute_borrowed_invoke_collection_lineage(
        &f,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        !out.suppressed_lineage_vars.contains(&vv(0)) && out.releases.is_empty(),
        "a loop-invariant root with an in-loop carrier declines (release re-reached without re-birth)",
    );
}

/// Root-collector pins: a heap-repr self-allocating-builtin Invoke result in
/// the owned set is a root; a Scalar-repr result and an unknown callee decline.
#[test]
fn collect_builtin_invoke_result_roots_repr_and_callee_gates() {
    let interner = ori_ir::StringInterner::new();
    let concat = interner.intern("concat");
    let user_fn = interner.intern("not_a_builtin");
    let mut f = func(
        3,
        vec![
            block(0, vec![], named_invoke(0, concat, vec![], 1, 3)),
            block(1, vec![], named_invoke(1, user_fn, vec![], 2, 3)),
            block(2, vec![], ArcTerminator::Return { value: vv(2) }),
            block(3, vec![], ArcTerminator::Resume),
        ],
    );
    f.var_reprs[0] = ValueRepr::FatValue;
    f.var_reprs[1] = ValueRepr::FatValue;
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(vv(0));
    owned.insert(vv(1));
    let roots = collect_fresh_builtin_invoke_result_roots(&f, &owned, &interner);
    assert!(
        roots.contains(&vv(0)) && !roots.contains(&vv(1)),
        "the concat result is a root; the unknown-callee result is not",
    );
    // Scalar-repr gate: the same concat result with a Scalar repr declines.
    f.var_reprs[0] = ValueRepr::Scalar;
    let roots = collect_fresh_builtin_invoke_result_roots(&f, &owned, &interner);
    assert!(
        roots.is_empty(),
        "a Scalar-repr result is never a root (no RC header to release)",
    );
}

/// `ORI_DISABLE_BUILTIN_INVOKE_RESULT_LINEAGE` accessor: unset in the test env
/// -> the third root family is active. (Behavioral toggle parity is pinned at
/// the AOT level: the template-chain cells abort/leak again under the toggle.)
#[test]
fn builtin_invoke_result_lineage_toggle_unset_is_active() {
    assert!(
        !builtin_invoke_result_lineage_disabled(),
        "the toggle is unset in the test env -> the builtin-result family is active",
    );
}

/// Gate (c3) positive + negative pair: a closure member at a borrowed `Invoke`
/// arg to a callee whose param carries `borrowed_cow_mutated` fires the gate
/// (the callee forwards the borrow into a COW-mutator owned position and nets
/// -1 per call — the RL-1 caller funding inc is load-bearing); the same shape
/// without the fact stays admitted.
#[test]
fn gate_c3_fires_only_on_borrowed_cow_consumed_callee_param() {
    use crate::aims::contract::{MemoryContract, ParamContract};

    // bb0: %0 = Construct List() ; Invoke @99(%0 [borrow]) normal bb1 unwind bb2.
    let f = func(
        2,
        vec![
            block(0, vec![list_construct(0)], borrowed_invoke(1, 0, 1, 2)),
            block(1, vec![], ArcTerminator::Return { value: vv(1) }),
            block(2, vec![], ArcTerminator::Resume),
        ],
    );
    let members: FxHashSet<ArcVarId> = std::iter::once(vv(0)).collect();

    let mut cow_param = ParamContract::CONSERVATIVE;
    cow_param.borrowed_cow_mutated = true;
    let mut cow_contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    cow_contracts.insert(
        Name::from_raw(99),
        MemoryContract {
            params: vec![cow_param],
            ..MemoryContract::conservative(1)
        },
    );
    assert!(
        closure_member_cow_consumed_at_call(&f, &members, &cow_contracts),
        "a borrowed Invoke arg to a borrowed_cow_mutated callee param fires gate (c3)",
    );

    let mut plain_contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    plain_contracts.insert(Name::from_raw(99), MemoryContract::conservative(1));
    assert!(
        !closure_member_cow_consumed_at_call(&f, &members, &plain_contracts),
        "the same shape without the borrowed_cow_mutated fact stays admitted",
    );
}
