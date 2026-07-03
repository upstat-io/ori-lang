//! Population pass for the per-`(ArcVarId, FieldPath)` birth-site partition.
//!
//! Walks the ARC IR once, admitting edges per the T1 partition calculus
//! (`AimsProof.Partition`). Tier-1 unconditional same-allocation edges union
//! directly: Let-Var whole-var aliases, Project field-path composition,
//! Construct field funding, and contract-proven Apply/Invoke result aliases
//! (`ApplyAliasSource::Direct` / `::Project`). Block-param merges and
//! `ApplyAliasSource::Conditional` aliases are admitted ONLY under the
//! singleton birth-site witness, iterated to a fixpoint so chained merges
//! compose. Birth sites are minted per `Construct` site; every other fresh
//! definition stays UNKNOWN, so a merge over it refuses conservatively.
//! COW-mutating uses (`Set` / `SetTag`) taint their class as a boundary.
//! Scalar / immortal vars carry no allocation and are skipped.

use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::birth_site_partition::{BirthSiteId, BirthSitePartition, FieldPath, NodeIdx};
use super::state_map::{AimsStateMap, ApplyAliasSource};

/// Build the birth-site partition for `func` from its converged state map.
///
/// Reads `state_map` for the scalar/immortal exclusion set and the pre-walk
/// `apply_result_aliases` side table; writes nothing. The caller installs
/// the result via [`AimsStateMap::set_birth_site_partition`]; the table is
/// read-only thereafter (PL-5).
pub(crate) fn compute_birth_site_partition(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> BirthSitePartition {
    let mut partition = BirthSitePartition::new();
    let mut next_site: u32 = 0;

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, args, .. } => {
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    let dst_node = whole_node(&mut partition, *dst);
                    partition.set_site(dst_node, BirthSiteId::new(next_site));
                    next_site += 1;
                    // Field funding: constructor arg positions ARE the field
                    // indices `Project { field }` reads back.
                    for (position, &arg) in args.iter().enumerate() {
                        if state_map.is_excluded(arg) {
                            continue;
                        }
                        let Ok(field) = u32::try_from(position) else {
                            unreachable!("constructor arg position exceeds u32::MAX");
                        };
                        let field_node = partition.register_node(*dst, FieldPath::single(field));
                        let arg_node = whole_node(&mut partition, arg);
                        partition.union_tier1(field_node, arg_node);
                    }
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    if state_map.is_excluded(*dst) || state_map.is_excluded(*src) {
                        continue;
                    }
                    let dst_node = whole_node(&mut partition, *dst);
                    let src_node = whole_node(&mut partition, *src);
                    partition.union_tier1(dst_node, src_node);
                }
                ArcInstr::Project {
                    dst, value, field, ..
                } => {
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    // Single-hop composition: multi-hop chains compose
                    // transitively through the union-find, never through
                    // hand-built multi-hop paths.
                    let dst_node = whole_node(&mut partition, *dst);
                    let field_node = partition.register_node(*value, FieldPath::single(*field));
                    partition.union_tier1(dst_node, field_node);
                }
                ArcInstr::Set { base, field, .. } => {
                    if state_map.is_excluded(*base) {
                        continue;
                    }
                    let base_node = whole_node(&mut partition, *base);
                    partition.mark_cow_boundary(base_node);
                    let field_node = partition.register_node(*base, FieldPath::single(*field));
                    partition.mark_cow_boundary(field_node);
                }
                ArcInstr::SetTag { base, .. } => {
                    if state_map.is_excluded(*base) {
                        continue;
                    }
                    let base_node = whole_node(&mut partition, *base);
                    partition.mark_cow_boundary(base_node);
                }
                _ => {}
            }
        }
    }

    let mut merges = collect_contract_alias_edges(&mut partition, state_map);
    collect_block_param_edges(&mut partition, func, state_map, &mut merges);
    admit_witnessed_merges(&mut partition, &merges);

    partition
}

/// Intern the whole-variable node for `var`.
fn whole_node(partition: &mut BirthSitePartition, var: ArcVarId) -> NodeIdx {
    partition.register_node(var, FieldPath::whole_var())
}

/// Admit contract-proven Apply/Invoke result aliases.
///
/// `Direct` (whole-var passthrough) and `Project` (single-field-of-param
/// return) are tier-1: the contract proves the result names the SAME
/// allocation. `Wrapped` is containment — the result is a SEPARATE
/// allocation — so no edge. `Conditional` is a merge over candidates
/// (the result aliases ONE of N at runtime) and joins the returned
/// witnessed-merge set. Sorted by destination for deterministic unions.
fn collect_contract_alias_edges(
    partition: &mut BirthSitePartition,
    state_map: &AimsStateMap,
) -> Vec<(ArcVarId, Vec<ArcVarId>)> {
    let mut aliases: Vec<(ArcVarId, &ApplyAliasSource)> = state_map
        .apply_result_aliases()
        .iter()
        .map(|(&dst, source)| (dst, source))
        .collect();
    aliases.sort_by_key(|&(dst, _)| dst.raw());

    let mut merges: Vec<(ArcVarId, Vec<ArcVarId>)> = Vec::new();
    for (dst, source) in aliases {
        if state_map.is_excluded(dst) {
            continue;
        }
        match source {
            ApplyAliasSource::Direct(arg) => {
                if state_map.is_excluded(*arg) {
                    continue;
                }
                let dst_node = whole_node(partition, dst);
                let arg_node = whole_node(partition, *arg);
                partition.union_tier1(dst_node, arg_node);
            }
            ApplyAliasSource::Project { arg, field } => {
                if state_map.is_excluded(*arg) {
                    continue;
                }
                let dst_node = whole_node(partition, dst);
                let field_node = partition.register_node(*arg, FieldPath::single(*field));
                partition.union_tier1(dst_node, field_node);
            }
            ApplyAliasSource::Wrapped(_) => {}
            ApplyAliasSource::Conditional { candidates } => {
                merges.push((dst, candidates.clone()));
            }
        }
    }
    merges
}

/// Route Jump-arg -> block-param edges.
///
/// A single-predecessor param is a pure renaming of its one arg (tier-1);
/// a multi-predecessor param is a phi merge and joins the witnessed set.
fn collect_block_param_edges(
    partition: &mut BirthSitePartition,
    func: &ArcFunction,
    state_map: &AimsStateMap,
    merges: &mut Vec<(ArcVarId, Vec<ArcVarId>)>,
) {
    let mut incoming: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut param_order: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for &(param, _) in &block.params {
            param_order.push(param);
        }
    }
    for block in &func.blocks {
        let ArcTerminator::Jump { target, args } = &block.terminator else {
            continue;
        };
        let Some(target_block) = func.blocks.get(target.index()) else {
            continue;
        };
        for (&arg, &(param, _)) in args.iter().zip(target_block.params.iter()) {
            incoming.entry(param).or_default().push(arg);
        }
    }

    for param in param_order {
        if state_map.is_excluded(param) {
            continue;
        }
        let Some(args) = incoming.remove(&param) else {
            continue;
        };
        if let [only_arg] = args.as_slice() {
            if state_map.is_excluded(*only_arg) {
                continue;
            }
            let param_node = whole_node(partition, param);
            let arg_node = whole_node(partition, *only_arg);
            partition.union_tier1(param_node, arg_node);
        } else {
            merges.push((param, args));
        }
    }
}

/// Admit the witnessed merges to a fixpoint.
///
/// Per merge, predecessors already in the merge node's class are dropped
/// first: a threaded-back self value holds the merge's own prior-iteration
/// value, so (by induction over iterations) its birth site equals the
/// admitted class's — the loop-invariant back-edge shape
/// (`T1_items_header_unified_across_backedge`). The singleton witness runs
/// over the remaining predecessors; an admission can enable a chained
/// merge, so the loop repeats until no admission fires.
fn admit_witnessed_merges(
    partition: &mut BirthSitePartition,
    merges: &[(ArcVarId, Vec<ArcVarId>)],
) {
    let mut changed = true;
    while changed {
        changed = false;
        for (merge_var, args) in merges {
            let merge_node = whole_node(partition, *merge_var);
            let mut preds: Vec<NodeIdx> = Vec::with_capacity(args.len());
            for &arg in args {
                let arg_node = whole_node(partition, arg);
                if partition.same_rep(arg_node, merge_node) {
                    continue;
                }
                preds.push(arg_node);
            }
            if preds.is_empty() {
                continue;
            }
            if partition.union_phi_witnessed(merge_node, &preds) {
                changed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn block(
        id: u32,
        params: Vec<u32>,
        body: Vec<ArcInstr>,
        terminator: ArcTerminator,
    ) -> ArcBlock {
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
        assert_eq!(partition.class_site(view), partition.class_site(buffer));
        assert!(partition.class_site(view).is_some());
        assert_ne!(
            partition.class_site(aggregate),
            partition.class_site(buffer)
        );
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
        assert_eq!(partition.class_site(param), None);
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
        assert_eq!(partition.class_site(param), partition.class_site(alloc));
        assert!(partition.class_site(param).is_some());
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
        assert_eq!(partition.class_site(param), None);
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
        assert!(partition.class_site(param).is_some());
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
        assert_eq!(partition.class_site(selected), None);
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
}
