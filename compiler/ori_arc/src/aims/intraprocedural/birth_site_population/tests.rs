//! Tests for the birth-site partition population pass.

use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::aims::intraprocedural::project_aliases::compute_genuine_same_alloc_reps;
use crate::ir::{ArcBlock, ArcBlockId, CtorKind};

use super::*;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn construct(dst: u32, args: Vec<u32>) -> ArcInstr {
    ArcInstr::Construct {
        dst: v(dst),
        ty: ty(0),
        ctor: CtorKind::Tuple,
        args: args.into_iter().map(v).collect(),
    }
}

fn block(id: u32, params: Vec<u32>, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: params.into_iter().map(|p| (v(p), ty(0))).collect(),
        body,
        terminator,
    }
}

fn jump(target: u32, args: Vec<u32>) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args: args.into_iter().map(v).collect(),
    }
}

fn func_with_blocks(num_vars: u32, blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        var_types: (0..num_vars).map(ty).collect(),
        blocks,
        ..Default::default()
    }
}

fn one_block_func(num_vars: u32, body: Vec<ArcInstr>) -> ArcFunction {
    func_with_blocks(
        num_vars,
        vec![block(
            0,
            vec![],
            body,
            ArcTerminator::Return { value: v(0) },
        )],
    )
}

fn compute(func: &ArcFunction) -> BirthSitePartition {
    let state_map = AimsStateMap::new(func);
    compute_birth_site_partition(func, &state_map)
}

fn whole(partition: &mut BirthSitePartition, var: u32) -> NodeIdx {
    partition.register_node(v(var), FieldPath::whole_var())
}

fn field(partition: &mut BirthSitePartition, var: u32, f: u32) -> NodeIdx {
    partition.register_node(v(var), FieldPath::single(f))
}

/// Construct funding + Project + Let compose into ONE class carrying the
/// stored allocation's birth site, distinct from the aggregate's class.
#[test]
fn construct_project_chain_unifies_field_path() {
    // %1 = Construct []          (the stored buffer)
    // %0 = Construct [%1]        (the aggregate; funds field 0)
    // %2 = Project %0.0          (borrow-view of the buffer)
    // %3 = Let Var(%2)           (whole-var alias of the view)
    let func = one_block_func(
        4,
        vec![
            construct(1, vec![]),
            construct(0, vec![1]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(2)),
            },
        ],
    );
    let mut partition = compute(&func);

    let buffer = whole(&mut partition, 1);
    let agg_field = field(&mut partition, 0, 0);
    let view = whole(&mut partition, 2);
    let alias = whole(&mut partition, 3);
    let aggregate = whole(&mut partition, 0);

    assert!(partition.same_rep(buffer, agg_field));
    assert!(partition.same_rep(view, buffer));
    assert!(partition.same_rep(alias, view));
    assert!(!partition.same_rep(aggregate, buffer));
    assert_eq!(partition.site(view), partition.site(buffer));
    assert!(partition.site(view).is_some());
    assert_ne!(partition.site(aggregate), partition.site(buffer));
}

/// A two-predecessor block-param merge over DISTINCT Construct sites has
/// no singleton witness: the param stays its own class with no site.
#[test]
fn two_pred_merge_over_distinct_construct_sites_refuses() {
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(2, vec![0])),
            block(1, vec![], vec![construct(1, vec![])], jump(2, vec![1])),
            block(2, vec![2], vec![], ArcTerminator::Return { value: v(2) }),
        ],
    );
    let mut partition = compute(&func);

    let entry_alloc = whole(&mut partition, 0);
    let latch_alloc = whole(&mut partition, 1);
    let param = whole(&mut partition, 2);
    assert!(!partition.same_rep(param, entry_alloc));
    assert!(!partition.same_rep(param, latch_alloc));
    assert!(!partition.same_rep(entry_alloc, latch_alloc));
    assert_eq!(partition.site(param), None);
}

/// The loop-invariant back-edge shape: a loop-header param fed the entry
/// allocation AND its own threaded-back value admits under the singleton
/// witness and joins the birth class.
#[test]
fn loop_invariant_back_edge_merge_admits() {
    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![], jump(1, vec![1])),
        ],
    );
    let mut partition = compute(&func);

    let alloc = whole(&mut partition, 0);
    let param = whole(&mut partition, 1);
    assert!(partition.same_rep(param, alloc));
    assert_eq!(partition.site(param), partition.site(alloc));
    assert!(partition.site(param).is_some());
}

/// The loop-VARYING back-edge shape: the latch feeds a per-iteration
/// allocation, so the merge refuses and stays distinct from both.
#[test]
fn loop_varying_back_edge_merge_refuses() {
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![construct(2, vec![])], jump(1, vec![2])),
        ],
    );
    let mut partition = compute(&func);

    let entry_alloc = whole(&mut partition, 0);
    let per_iter_alloc = whole(&mut partition, 2);
    let param = whole(&mut partition, 1);
    assert!(!partition.same_rep(param, entry_alloc));
    assert!(!partition.same_rep(param, per_iter_alloc));
    assert_eq!(partition.site(param), None);
}

/// A single-predecessor param is a pure renaming: tier-1 union.
#[test]
fn single_pred_param_forwarding_is_tier1() {
    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![], ArcTerminator::Return { value: v(1) }),
        ],
    );
    let mut partition = compute(&func);

    let alloc = whole(&mut partition, 0);
    let param = whole(&mut partition, 1);
    assert!(partition.same_rep(param, alloc));
    assert!(partition.site(param).is_some());
}

/// `Set` taints the base's whole-var class AND the touched field class
/// (through class membership: the funded arg is tainted too); `SetTag`
/// taints the whole-var class only.
#[test]
fn set_and_settag_mark_cow_boundaries() {
    let func = one_block_func(
        4,
        vec![
            construct(1, vec![]),
            construct(0, vec![1]),
            ArcInstr::Set {
                base: v(0),
                field: 0,
                value: v(2),
            },
            construct(3, vec![]),
            ArcInstr::SetTag { base: v(3), tag: 1 },
        ],
    );
    let mut partition = compute(&func);

    let base = whole(&mut partition, 0);
    let base_field = field(&mut partition, 0, 0);
    let funded_arg = whole(&mut partition, 1);
    let tagged = whole(&mut partition, 3);
    assert!(partition.is_cow_boundary(base));
    assert!(partition.is_cow_boundary(base_field));
    assert!(partition.is_cow_boundary(funded_arg));
    assert!(partition.is_cow_boundary(tagged));
}

/// Scalar-repr vars carry no allocation: excluded vars produce no nodes,
/// no birth sites, and no edges.
#[test]
fn scalar_vars_are_skipped() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![1]),
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(2));
    let partition = compute_birth_site_partition(&func, &state_map);

    // Only the aggregate's whole-var node exists: the scalar arg funds
    // no field and the scalar Let-Var pair adds no edge.
    assert_eq!(partition.len(), 1);
}

/// Contract-proven result aliases: `Direct` and `Project` union tier-1;
/// `Wrapped` adds no edge; `Conditional` admits only under the singleton
/// witness.
#[test]
fn contract_result_aliases_follow_admission_kinds() {
    // %0 / %1 = distinct Constructs; %2..%6 = call results.
    let func = one_block_func(7, vec![construct(0, vec![]), construct(1, vec![])]);
    let mut state_map = AimsStateMap::new(&func);
    let mut aliases: FxHashMap<ArcVarId, ApplyAliasSource> = FxHashMap::default();
    aliases.insert(v(2), ApplyAliasSource::Direct(v(0)));
    aliases.insert(
        v(3),
        ApplyAliasSource::Project {
            arg: v(0),
            field: 1,
        },
    );
    aliases.insert(v(4), ApplyAliasSource::Wrapped(v(0)));
    aliases.insert(
        v(5),
        ApplyAliasSource::Conditional {
            candidates: vec![v(0), v(0)],
        },
    );
    aliases.insert(
        v(6),
        ApplyAliasSource::Conditional {
            candidates: vec![v(0), v(1)],
        },
    );
    state_map.set_apply_result_aliases(aliases);
    let mut partition = compute_birth_site_partition(&func, &state_map);

    let ctor_a = whole(&mut partition, 0);
    let ctor_b = whole(&mut partition, 1);
    let direct = whole(&mut partition, 2);
    let projected = whole(&mut partition, 3);
    let ctor_a_field = field(&mut partition, 0, 1);
    let wrapped = whole(&mut partition, 4);
    let same_site_merge = whole(&mut partition, 5);
    let split_site_merge = whole(&mut partition, 6);

    assert!(partition.same_rep(direct, ctor_a));
    assert!(partition.same_rep(projected, ctor_a_field));
    assert!(!partition.same_rep(wrapped, ctor_a));
    assert!(partition.same_rep(same_site_merge, ctor_a));
    assert!(!partition.same_rep(split_site_merge, ctor_a));
    assert!(!partition.same_rep(split_site_merge, ctor_b));
}

/// `Select` is an EXCLUDED edge: a select over two DISTINCT birth sites
/// mints no union, so the dst stays its own class with no birth site.
#[test]
fn distinct_birth_select_stays_distinct() {
    let func = one_block_func(
        4,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Select {
                dst: v(2),
                ty: ty(0),
                cond: v(3),
                true_val: v(0),
                false_val: v(1),
            },
        ],
    );
    let mut partition = compute(&func);

    let site_a = whole(&mut partition, 0);
    let site_b = whole(&mut partition, 1);
    let selected = whole(&mut partition, 2);
    assert!(!partition.same_rep(selected, site_a));
    assert!(!partition.same_rep(selected, site_b));
    assert_eq!(partition.site(selected), None);
}

fn genuine_rep(reps: &FxHashMap<ArcVarId, ArcVarId>, var: u32) -> ArcVarId {
    reps.get(&v(var)).copied().unwrap_or(v(var))
}

/// Tier-1 parity, unconditional subset: on a shape with ONLY Let{Var} +
/// Direct + same-site Conditional edges, the whole-var partition classes
/// agree EXACTLY with `compute_genuine_same_alloc_reps` — no missed
/// tier-1 union, no false unification.
#[test]
fn tier1_parity_with_genuine_same_alloc_reps() {
    let func = one_block_func(
        6,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(2)),
            },
        ],
    );
    let mut aliases: FxHashMap<ArcVarId, ApplyAliasSource> = FxHashMap::default();
    aliases.insert(v(4), ApplyAliasSource::Direct(v(3)));
    aliases.insert(
        v(5),
        ApplyAliasSource::Conditional {
            candidates: vec![v(0), v(2)],
        },
    );
    let genuine = compute_genuine_same_alloc_reps(&func, &aliases);
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_apply_result_aliases(aliases);
    let mut partition = compute_birth_site_partition(&func, &state_map);

    let nodes: Vec<NodeIdx> = (0..6).map(|n| whole(&mut partition, n)).collect();
    for a in 0..6u32 {
        for b in (a + 1)..6u32 {
            let genuine_equal = genuine_rep(&genuine, a) == genuine_rep(&genuine, b);
            assert_eq!(
                partition.same_rep(nodes[a as usize], nodes[b as usize]),
                genuine_equal,
                "tier-1 parity diverged on (%{a}, %{b})"
            );
        }
    }
}

/// The split-site Conditional is where the two structures DIVERGE by
/// design: `compute_genuine_same_alloc_reps` over-approximates (unions
/// the dst with every candidate, bridging distinct births), while the
/// birth-site partition refuses without the singleton witness.
#[test]
fn split_site_conditional_partition_refuses_genuine_over_approx() {
    let func = one_block_func(3, vec![construct(0, vec![]), construct(1, vec![])]);
    let mut aliases: FxHashMap<ArcVarId, ApplyAliasSource> = FxHashMap::default();
    aliases.insert(
        v(2),
        ApplyAliasSource::Conditional {
            candidates: vec![v(0), v(1)],
        },
    );
    let genuine = compute_genuine_same_alloc_reps(&func, &aliases);
    assert_eq!(genuine_rep(&genuine, 2), genuine_rep(&genuine, 0));
    assert_eq!(genuine_rep(&genuine, 0), genuine_rep(&genuine, 1));

    let mut state_map = AimsStateMap::new(&func);
    state_map.set_apply_result_aliases(aliases);
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let site_a = whole(&mut partition, 0);
    let site_b = whole(&mut partition, 1);
    let merged = whole(&mut partition, 2);
    assert!(!partition.same_rep(merged, site_a));
    assert!(!partition.same_rep(merged, site_b));
    assert!(!partition.same_rep(site_a, site_b));
}

/// Chained merges compose through the fixpoint: a merge over a
/// NOT-YET-admitted merge param refuses on the first sweep and admits on
/// the next, after the inner merge's admission lands. Block order puts
/// the dependent merge FIRST so a single sweep provably cannot admit it.
#[test]
fn chained_merges_admit_to_fixpoint() {
    // b0(%2): Return %2            <- dependent merge: preds %1 (b3), %0 (b4)
    // b1: %0 = Construct; Jump b3(%0)
    // b2: Jump b3(%0)
    // b3(%1): Jump b0(%1)          <- inner merge: preds %0, %0
    // b4: Jump b0(%0)
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![2], vec![], ArcTerminator::Return { value: v(2) }),
            block(1, vec![], vec![construct(0, vec![])], jump(3, vec![0])),
            block(2, vec![], vec![], jump(3, vec![0])),
            block(3, vec![1], vec![], jump(0, vec![1])),
            block(4, vec![], vec![], jump(0, vec![0])),
        ],
    );
    let mut partition = compute(&func);

    let alloc = whole(&mut partition, 0);
    let inner_merge = whole(&mut partition, 1);
    let dependent_merge = whole(&mut partition, 2);
    assert!(partition.same_rep(inner_merge, alloc));
    assert!(partition.same_rep(dependent_merge, alloc));
}

/// A non-excluded heap literal (non-empty string) is a fresh allocation
/// site: its class carries a KNOWN birth site, distinct per `Let` site, so
/// a phi merge over two distinct literals refuses (no singleton witness).
#[test]
fn str_literal_lets_mint_distinct_known_birth_sites() {
    let func = one_block_func(
        2,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::String(ori_ir::Name::from_raw(3))),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::String(ori_ir::Name::from_raw(4))),
            },
        ],
    );
    let mut partition = compute(&func);

    let first = whole(&mut partition, 0);
    let second = whole(&mut partition, 1);
    assert!(partition.site(first).is_some());
    assert!(partition.site(second).is_some());
    assert_ne!(partition.site(first), partition.site(second));
    assert!(!partition.same_rep(first, second));
}

/// An immortal literal stays excluded: no node facts, no birth site.
#[test]
fn immortal_literal_let_mints_no_birth_site() {
    let func = one_block_func(
        1,
        vec![ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(crate::ir::LitValue::String(ori_ir::Name::from_raw(3))),
        }],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_immortals(vec![true]);
    let mut partition = compute_birth_site_partition(&func, &state_map);

    let node = whole(&mut partition, 0);
    assert_eq!(partition.site(node), None);
}

/// A non-excluded heap-producing `PrimOp` dst (string concat) is a fresh
/// allocation site with its own known birth site.
#[test]
fn heap_primop_dst_mints_birth_site() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            },
        ],
    );
    let mut partition = compute(&func);

    let result = whole(&mut partition, 2);
    let lhs = whole(&mut partition, 0);
    assert!(partition.site(result).is_some());
    assert_ne!(partition.site(result), partition.site(lhs));
}

/// Mutually-dependent phi merges (a loop param and an invoke-split merge
/// param feeding each other): neither admits node-locally — each waits on
/// the other's unknown site — but the family's ONLY external predecessor is
/// one known Construct site, so the SCC flow witness assigns it and both
/// admit into the construct's class (PV-1 P6).
#[test]
fn mutually_dependent_merges_admit_via_scc_flow_witness() {
    // bb0: %0 = Construct; Jump bb1(%0)
    // bb1(%1): Branch %3 ? bb2 : bb6
    // bb2: Branch %3 ? bb3 : bb4
    // bb3: Jump bb5(%1)
    // bb4: Jump bb5(%1)
    // bb5(%2): Jump bb1(%2)
    // bb6: Return %1
    let func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(
                1,
                vec![1],
                vec![],
                ArcTerminator::Branch {
                    cond: v(3),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(6),
                },
            ),
            block(
                2,
                vec![],
                vec![],
                ArcTerminator::Branch {
                    cond: v(3),
                    then_block: ArcBlockId::new(3),
                    else_block: ArcBlockId::new(4),
                },
            ),
            block(3, vec![], vec![], jump(5, vec![1])),
            block(4, vec![], vec![], jump(5, vec![1])),
            block(5, vec![2], vec![], jump(1, vec![2])),
            block(6, vec![], vec![], ArcTerminator::Return { value: v(1) }),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let mut partition = compute_birth_site_partition(&func, &state_map);

    let alloc = whole(&mut partition, 0);
    let outer = whole(&mut partition, 1);
    let inner = whole(&mut partition, 2);
    assert!(
        partition.same_rep(outer, alloc),
        "outer param joins the construct class"
    );
    assert!(
        partition.same_rep(inner, alloc),
        "inner merge joins the construct class"
    );
    assert!(partition.site(outer).is_some());
}
