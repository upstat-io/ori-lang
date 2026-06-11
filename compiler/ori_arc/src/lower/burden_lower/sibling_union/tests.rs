//! Unit tests for the sibling-alias moved-field cross-check.
//!
//! Pins the structural primitives (group-building, per-field widening,
//! single-releaser pick, decline-on-cross-variant, root-untouched, no-loop
//! decline) + the Apply-owned-param transfer disposition (matrix pin 11's
//! ori_arc-level claim).

use ori_types::{Idx, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::moved_fields::populate_moved_out_fields;
use super::super::BurdenLowerCtx;
use super::{
    apply_sibling_moved_field_union, build_sibling_groups, instr_transfers_owned,
    owned_top_level_fields, root_is_multivariant_sum, var_carries_rc,
};
use crate::aims::intraprocedural::project_aliases::{
    compute_param_edge_args, compute_project_alias_table,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, ValueRepr,
};
use crate::lower::test_utils::{registered_struct_with_two_owned_str_fields, test_name};

const PAIR_IDX: Idx = Idx::from_raw(100);

/// Build a 2-heap-field loop self-rebuild `ArcFunction` mirroring the real
/// lowering shape:
///
/// ```text
/// bb0:                       %0 = Construct Pair(...)   ; Jump bb1(%0)
/// bb1: (%1: Pair)            Branch -> bb2 | bb3        ; loop header
/// bb2:                       %2 = %1  ; %3 = Project %2.0
///                            %4 = %1  ; %5 = Project %4.1
///                            %6 = Construct Pair(%3, %5) ; Jump bb1(%6)  ; back-edge
/// bb3:                       Return %1
/// ```
///
/// `%2`,`%4` are the two sibling projection-carrier aliases of loop block-param
/// `%1`; `%3`,`%5` are the projected RC fields. Var reprs: Pair = Aggregate,
/// `[int]` projections = `RcPointer`.
fn two_field_loop_self_rebuild() -> (ArcFunction, TypeRegistry) {
    let mut registry = TypeRegistry::new();
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", PAIR_IDX);

    let v = ArcVarId::new;
    let body_bb2 = vec![
        // %2 = %1 ; %3 = Project %2.0
        ArcInstr::Let {
            dst: v(2),
            ty: PAIR_IDX,
            value: ArcValue::Var(v(1)),
        },
        ArcInstr::Project {
            dst: v(3),
            ty: Idx::STR,
            value: v(2),
            field: 0,
        },
        // %4 = %1 ; %5 = Project %4.1
        ArcInstr::Let {
            dst: v(4),
            ty: PAIR_IDX,
            value: ArcValue::Var(v(1)),
        },
        ArcInstr::Project {
            dst: v(5),
            ty: Idx::STR,
            value: v(4),
            field: 1,
        },
        // %6 = Construct Pair(%3, %5)
        ArcInstr::Construct {
            dst: v(6),
            ty: PAIR_IDX,
            ctor: CtorKind::Struct(test_name("Pair")),
            args: vec![v(3), v(5)],
        },
    ];

    let func = ArcFunction {
        // 0:Pair 1:Pair(param) 2:Pair 3:str 4:Pair 5:str 6:Pair
        var_types: vec![
            PAIR_IDX,
            PAIR_IDX,
            PAIR_IDX,
            Idx::STR,
            PAIR_IDX,
            Idx::STR,
            PAIR_IDX,
        ],
        var_reprs: vec![
            ValueRepr::Aggregate,
            ValueRepr::Aggregate,
            ValueRepr::Aggregate,
            ValueRepr::RcPointer,
            ValueRepr::Aggregate,
            ValueRepr::RcPointer,
            ValueRepr::Aggregate,
        ],
        blocks: vec![
            // bb0
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Construct {
                    dst: v(0),
                    ty: PAIR_IDX,
                    ctor: CtorKind::Struct(test_name("Pair")),
                    args: vec![],
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(0)],
                },
            },
            // bb1: loop header, param %1
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(1), PAIR_IDX)],
                body: Vec::new(),
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(3),
                },
            },
            // bb2: rebuild, back-edge to bb1
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: body_bb2,
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(6)],
                },
            },
            // bb3: return
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        entry: ArcBlockId::new(0),
        name: test_name("main"),
        params: Vec::new(),
        ..ArcFunction::default()
    };
    (func, registry)
}

/// Test harness: build the §1.9 alias table + per-edge attribution from the
/// fixture (empty apply-result map — structural edges only) and group with an
/// empty dup set unless the test computes one.
fn groups_for(func: &ArcFunction) -> Vec<super::SiblingGroup> {
    let table = compute_project_alias_table(func, &FxHashMap::default());
    let edges = compute_param_edge_args(func);
    build_sibling_groups(func, &table, &edges, &FxHashSet::default())
}

#[test]
fn param_edge_args_classifies_loop_back_edge() {
    let (func, _registry) = two_field_loop_self_rebuild();
    let edges = compute_param_edge_args(&func);
    let Some(param_edges) = edges.get(&ArcVarId::new(1)) else {
        panic!("loop block-param %1 has incoming Jump edges")
    };
    assert_eq!(param_edges.len(), 2, "init edge + back-edge");
    let init = param_edges
        .iter()
        .find(|e| e.pred_block == 0)
        .unwrap_or_else(|| panic!("bb0 init edge present"));
    assert!(!init.is_back_edge, "bb0 -> bb1 init edge is FORWARD");
    assert_eq!(init.arg, ArcVarId::new(0), "init edge passes %0");
    let back = param_edges
        .iter()
        .find(|e| e.pred_block == 2)
        .unwrap_or_else(|| panic!("bb2 back-edge present"));
    assert!(back.is_back_edge, "bb2 -> bb1 closes the cycle (back-edge)");
    assert_eq!(
        back.arg,
        ArcVarId::new(6),
        "back-edge passes the new struct"
    );
}

#[test]
fn build_sibling_groups_keys_two_carriers_to_loop_param_root() {
    let (func, _registry) = two_field_loop_self_rebuild();
    let groups = groups_for(&func);
    assert_eq!(groups.len(), 1, "one rebuild group");
    let g = &groups[0];
    assert_eq!(g.root, ArcVarId::new(1), "root is the loop block-param");
    assert_eq!(g.block_idx, 2, "siblings live in the rebuild block bb2");
    assert_eq!(
        g.siblings,
        vec![ArcVarId::new(2), ArcVarId::new(4)],
        "both projection-carrier aliases are siblings; root NOT a sibling",
    );
}

#[test]
fn root_never_a_sibling() {
    let (func, _registry) = two_field_loop_self_rebuild();
    let groups = groups_for(&func);
    let g = &groups[0];
    assert!(
        !g.siblings.contains(&g.root),
        "the alias-chain root is never a suppression target",
    );
}

#[test]
fn per_field_widening_absorbs_both_siblings_into_full_move() {
    let (func, registry) = two_field_loop_self_rebuild();
    // Populate the moved-out-fields union (Pass 1-3) as the production pipeline
    // does, then run the post-process.
    let terminator_transfer: Vec<FxHashSet<ArcVarId>> =
        vec![FxHashSet::default(); func.blocks.len()];
    let mut ctx = BurdenLowerCtx::new(&func);
    populate_moved_out_fields(&mut ctx, &func, &terminator_transfer, &registry);

    // Sibling %2 moved field 0; sibling %4 moved field 1 (per the projections).
    let union = &ctx.moved_out_fields_union;
    assert_eq!(
        union.get(&ArcVarId::new(2)).map(|s| {
            let mut v: Vec<u32> = s.iter().copied().collect();
            v.sort_unstable();
            v
        }),
        Some(vec![0]),
        "%2 moved field 0",
    );
    assert_eq!(
        union.get(&ArcVarId::new(4)).map(|s| {
            let mut v: Vec<u32> = s.iter().copied().collect();
            v.sort_unstable();
            v
        }),
        Some(vec![1]),
        "%4 moved field 1",
    );

    let owned_vars_needing_rc: FxHashSet<ArcVarId> =
        [ArcVarId::new(2), ArcVarId::new(4)].into_iter().collect();
    // Pre-post-process: each sibling is a partial-move skipping only its own
    // moved field.
    let mut full_move: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut partial: FxHashMap<ArcVarId, Vec<u32>> =
        [(ArcVarId::new(2), vec![0]), (ArcVarId::new(4), vec![1])]
            .into_iter()
            .collect();

    let table = compute_project_alias_table(&func, &FxHashMap::default());
    let edges = compute_param_edge_args(&func);
    apply_sibling_moved_field_union(
        &func,
        &registry,
        union,
        &owned_vars_needing_rc,
        &table,
        &edges,
        &FxHashSet::default(),
        &mut full_move,
        &mut partial,
    );

    // Union {0,1} covers each sibling's whole owned-field set -> both become
    // full-move; both partial entries are removed.
    assert!(
        full_move.contains(&ArcVarId::new(2)) && full_move.contains(&ArcVarId::new(4)),
        "both siblings absorbed into full_move_vars",
    );
    assert!(
        !partial.contains_key(&ArcVarId::new(2)) && !partial.contains_key(&ArcVarId::new(4)),
        "no partial-dec survives for a fully-covered sibling",
    );
    assert!(
        !full_move.contains(&ArcVarId::new(1)),
        "the loop block-param root is NEVER absorbed",
    );
}

#[test]
fn toggle_skips_the_post_process() {
    let (func, registry) = two_field_loop_self_rebuild();
    let terminator_transfer: Vec<FxHashSet<ArcVarId>> =
        vec![FxHashSet::default(); func.blocks.len()];
    let mut ctx = BurdenLowerCtx::new(&func);
    populate_moved_out_fields(&mut ctx, &func, &terminator_transfer, &registry);
    let union = ctx.moved_out_fields_union.clone();
    let owned_vars_needing_rc: FxHashSet<ArcVarId> =
        [ArcVarId::new(2), ArcVarId::new(4)].into_iter().collect();
    let mut full_move: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut partial: FxHashMap<ArcVarId, Vec<u32>> =
        [(ArcVarId::new(2), vec![0]), (ArcVarId::new(4), vec![1])]
            .into_iter()
            .collect();

    // Injected disabled-verdict — no process-global env mutation (env writes
    // race parallel test threads reading the same toggle).
    let table = compute_project_alias_table(&func, &FxHashMap::default());
    let edges = compute_param_edge_args(&func);
    super::apply_sibling_moved_field_union_with(
        true,
        &func,
        &registry,
        &union,
        &owned_vars_needing_rc,
        &table,
        &edges,
        &FxHashSet::default(),
        &mut full_move,
        &mut partial,
    );

    assert!(
        full_move.is_empty(),
        "toggle ON -> no sibling absorbed (per-alias attribution restored)",
    );
    assert_eq!(
        partial.get(&ArcVarId::new(2)),
        Some(&vec![0]),
        "toggle ON -> partial skip unchanged",
    );
}

#[test]
fn owned_top_level_fields_reads_burden() {
    let (func, registry) = two_field_loop_self_rebuild();
    let mut fields: Vec<u32> = owned_top_level_fields(&func, ArcVarId::new(1), &registry)
        .into_iter()
        .collect();
    fields.sort_unstable();
    assert_eq!(fields, vec![0, 1], "Pair's two heap fields are 0 and 1");
}

#[test]
fn struct_root_is_not_multivariant_sum() {
    let (func, registry) = two_field_loop_self_rebuild();
    assert!(
        !root_is_multivariant_sum(&func, ArcVarId::new(1), &registry),
        "a struct (no variants) is not a multivariant sum -> unification admitted",
    );
}

#[test]
fn apply_owned_param_is_a_transfer() {
    // Matrix pin 11 (ori_arc-level disposition): an Apply to an Owned param
    // is an owned-position transfer. A sibling whose projected field is
    // forwarded through such a call has that field counted in the union (an
    // uncovered Apply-transferred field would leave a double-freeing dec).
    let v = ArcVarId::new;
    let instr = ArcInstr::Apply {
        dst: v(10),
        ty: Idx::STR,
        func: test_name("forward"),
        args: vec![v(5)],
        arg_ownership: vec![ArgOwnership::Owned],
        mono_instance_id: None,
    };
    assert!(
        instr_transfers_owned(&instr, v(5)),
        "an Owned Apply arg is an owned-position transfer (RL-2)",
    );
}

#[test]
fn borrowed_apply_arg_is_not_a_transfer() {
    let v = ArcVarId::new;
    let instr = ArcInstr::Apply {
        dst: v(10),
        ty: Idx::INT,
        func: test_name("len"),
        args: vec![v(5)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    };
    assert!(
        !instr_transfers_owned(&instr, v(5)),
        "a Borrowed Apply arg is a read, not an owned-position transfer",
    );
}

#[test]
fn var_carries_rc_distinguishes_heap_from_scalar() {
    // Extend the fixture with one scalar (`Idx::INT`) var to pin the negative
    // side of `burden_carries_rc` (a scalar carries no RC).
    let (mut func, registry) = two_field_loop_self_rebuild();
    func.var_types.push(Idx::INT);
    func.var_reprs.push(ValueRepr::Scalar);
    let scalar_var = ArcVarId::new(7);
    assert!(
        var_carries_rc(&func, ArcVarId::new(1), &registry),
        "the Pair aggregate carries RC (two owned str fields)",
    );
    assert!(
        !var_carries_rc(&func, scalar_var, &registry),
        "a scalar int var carries no RC -> contributes nothing to the union",
    );
}

/// RL-5 rebuild-lineage dead-param release: the loop-exit Jump edge feeding a
/// DEAD successor block-param with the union-fired root's value gets exactly
/// one release at that param; the back-edge re-entry never does.
#[test]
fn rebuild_lineage_dead_exit_param_gets_one_release() {
    let (mut func, _registry) = two_field_loop_self_rebuild();
    let v = ArcVarId::new;
    // Rewire bb3: loop exit now THREADS the loop param to a dead param in bb4.
    func.var_types.push(PAIR_IDX); // v7
    func.var_reprs.push(ValueRepr::Aggregate);
    func.blocks[3].terminator = ArcTerminator::Jump {
        target: ArcBlockId::new(4),
        args: vec![v(1)],
    };
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(4),
        params: vec![(v(7), PAIR_IDX)],
        body: Vec::new(),
        terminator: ArcTerminator::Unreachable,
    });

    let table = compute_project_alias_table(&func, &FxHashMap::default());
    let edges = compute_param_edge_args(&func);
    let fired: FxHashSet<ArcVarId> = [v(1)].into_iter().collect();
    let releases = super::super::ownership_scans::compute_rebuild_lineage_dead_param_releases(
        &func,
        &fired,
        &table.genuine_same_alloc_reps,
        &edges,
    );
    assert_eq!(
        releases.get(&4).map(Vec::as_slice),
        Some(&[v(7)][..]),
        "dead loop-exit param receives exactly one RL-5 release"
    );
    assert_eq!(releases.len(), 1, "no release on any other block");
    assert!(
        !releases.contains_key(&1),
        "the back-edge re-entry param is never a release target"
    );
}

/// The RL-5 release scan declines a LIVE exit param (its own last-use dec is
/// the release) and an empty fired-root set.
#[test]
fn rebuild_lineage_release_declines_live_param_and_unfired_root() {
    let (mut func, _registry) = two_field_loop_self_rebuild();
    let v = ArcVarId::new;
    func.var_types.push(PAIR_IDX); // v7
    func.var_reprs.push(ValueRepr::Aggregate);
    func.blocks[3].terminator = ArcTerminator::Jump {
        target: ArcBlockId::new(4),
        args: vec![v(1)],
    };
    // v7 is LIVE (returned).
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(4),
        params: vec![(v(7), PAIR_IDX)],
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: v(7) },
    });

    let table = compute_project_alias_table(&func, &FxHashMap::default());
    let edges = compute_param_edge_args(&func);
    let fired: FxHashSet<ArcVarId> = [v(1)].into_iter().collect();
    let releases = super::super::ownership_scans::compute_rebuild_lineage_dead_param_releases(
        &func,
        &fired,
        &table.genuine_same_alloc_reps,
        &edges,
    );
    assert!(
        releases.is_empty(),
        "live exit param declines the RL-5 release"
    );

    let unfired: FxHashSet<ArcVarId> = FxHashSet::default();
    let releases = super::super::ownership_scans::compute_rebuild_lineage_dead_param_releases(
        &func,
        &unfired,
        &table.genuine_same_alloc_reps,
        &edges,
    );
    assert!(releases.is_empty(), "no fired roots -> no releases");
}

/// The union outcome reports fired roots; a fired two-carrier group names the
/// loop block-param root.
#[test]
fn union_outcome_reports_fired_back_edge_root() {
    let (func, registry) = two_field_loop_self_rebuild();
    let terminator_transfer: Vec<FxHashSet<ArcVarId>> =
        vec![FxHashSet::default(); func.blocks.len()];
    let mut ctx = BurdenLowerCtx::new(&func);
    populate_moved_out_fields(&mut ctx, &func, &terminator_transfer, &registry);
    let union = ctx.moved_out_fields_union.clone();
    let owned_vars_needing_rc: FxHashSet<ArcVarId> =
        [ArcVarId::new(2), ArcVarId::new(4)].into_iter().collect();
    let mut full_move: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut partial: FxHashMap<ArcVarId, Vec<u32>> =
        [(ArcVarId::new(2), vec![0]), (ArcVarId::new(4), vec![1])]
            .into_iter()
            .collect();
    let table = compute_project_alias_table(&func, &FxHashMap::default());
    let edges = compute_param_edge_args(&func);
    let outcome = apply_sibling_moved_field_union(
        &func,
        &registry,
        &union,
        &owned_vars_needing_rc,
        &table,
        &edges,
        &FxHashSet::default(),
        &mut full_move,
        &mut partial,
    );
    assert!(
        outcome.fired_roots.contains(&ArcVarId::new(1)),
        "applied two-carrier group reports the loop block-param root as fired"
    );
    assert!(
        outcome.keepers.is_empty(),
        "direct one-hop group assigns no keeper"
    );
}
