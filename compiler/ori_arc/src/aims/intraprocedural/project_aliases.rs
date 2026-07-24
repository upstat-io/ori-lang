//! Project alias analysis for backward demand propagation.
//!
//! Computes a function-wide map from Project destinations (and their transitive
//! aliases through Let instructions and Jump-arg → block-param edges) to the
//! original Project source variables. At CFG merge points a block param may
//! receive projected values from *multiple* predecessor aggregates, so the map
//! value is a `SmallVec` of sources (not a single `ArcVarId`).
//!
//! This enables [`propagate_project_source_demand`] to keep *all* possible
//! parent aggregates alive when borrowed children are used cross-block.
//!
//! # Example
//!
//! ```text
//! %3 = Project %2.0
//! %4 = Let Var(%3)
//! Jump block1, args=[%4]
//! // block1 params: [%5]
//! ```
//!
//! Maps: `%3 → [%2]`, `%4 → [%2]`, `%5 → [%2]`

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::{smallvec, SmallVec};

pub(crate) use crate::ir::ParamEdgeArg;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::lattice::{AimsState, Cardinality};
use super::block_state::BlockState;
use super::state_map::{AimsStateMap, ApplyAliasSource};

/// Per-variable set of possible Project source aggregates.
///
/// `SmallVec<[ArcVarId; 1]>` avoids heap allocation for the common
/// single-predecessor case while supporting multi-predecessor merges.
pub(crate) type ProjectSources = SmallVec<[ArcVarId; 1]>;

/// The §1.9 `project_alias_sources` side table plus the over-approximation dst
/// set. The map is the unified same-allocation alias graph (R1 Project, R2
/// generalized to whole-var Let identity of non-projected roots, R3 Jump-arg,
/// R4 CFG-merge, R5 Select, R6 nested-Project transitive).
pub(crate) struct ProjectAliasTable {
    /// The UNIFIED same-allocation alias closure (R1 + R2-gen + R3 + R4 + R5 +
    /// R6). Installed on `AimsStateMap` for the post-convergence consumers
    /// (PIN-6 project-alias seeds, `cleanup_redundant`).
    pub(crate) sources: FxHashMap<ArcVarId, ProjectSources>,
    /// The ORIGINAL §1.9 backward-demand table (R1 + R3 + R4 + R6 — NO R2-gen
    /// whole-var identity, NO R5). Consumed by `propagate_project_source_demand`;
    /// byte-identical to the pre-unification table so the proven
    /// keep-the-borrow-parent-alive behavior is preserved. R2-gen is absent
    /// because a whole-var alias threaded through a CFG merge is a MOVE, not a
    /// borrow — keeping the parent alive at the merge suppresses the untaken
    /// parent's branch-edge dec (the merge-edge scoped-cleanup leak). Spec: Annex
    /// E §AIMS.
    pub(crate) demand_sources: FxHashMap<ArcVarId, ProjectSources>,
    /// R5 Select-origin dsts. Consumed by `propagate_project_source_demand` to
    /// exclude Select aliases from BACKWARD demand over `demand_sources`.
    pub(crate) select_alias_dsts: FxHashSet<ArcVarId>,
}

/// Compute a function-wide map from (Project destination + transitive Let
/// aliases + block param aliases + Apply-result alias roots) to the set of
/// possible Project / Apply-aliased source variables.
///
/// For `%3 = Project %2.0` followed by `%4 = Let Var(%3)`, maps both
/// `%3 → [%2]` and `%4 → [%2]`. For `Jump block1, args=[%3]` where block1
/// has param `%5`, maps `%5 → [%2]`. At CFG merges where multiple
/// predecessors jump to the same block with projections from different
/// aggregates, the param maps to the union of all sources.
///
/// Also folds in [`ApplyAliasSource`] entries as Step 1b roots so
/// shipped transitivity Rules 2/3/4/6 (§1.9 `project_alias_sources` preamble —
/// Rules 5/7 are unshipped, folding through them is dead code) carry the
/// Apply-result allocation-identity through Let/Jump/CFG-merge/nested-Project
/// alias chains. When `%4 = Apply...` and `apply_result_aliases[%4] =
/// Direct(%3)`, Step 1b seeds `alias_sources[%4] = [%3]`; downstream `%5 = Let
/// Var(%4)` then transitively maps `%5 → [%3]` via the existing Step 2
/// worklist without any per-rule code change.
///
/// This enables `propagate_project_source_demand` to detect demand on any
/// alias of a projected or Apply-aliased value — including block params that
/// receive projected values from multiple predecessors via Jump arguments.
///
/// Precomputed once before the worklist loop (the alias structure is static).
///
/// Map-only projection of [`compute_project_alias_table`] retained for tests
/// that consume only the `var → sources` map. The R5 Select-origin dst set is
/// discarded here.
#[cfg(test)]
pub(crate) fn compute_project_alias_sources(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> FxHashMap<ArcVarId, ProjectSources> {
    compute_project_alias_table(func, apply_result_aliases).sources
}

/// Compute production alias facts using the frozen representation classes.
/// Scalar projections copy bits and therefore carry no allocation identity.
pub(crate) fn compute_project_alias_table_for_state(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
    state_map: &AimsStateMap,
) -> ProjectAliasTable {
    compute_project_alias_table_filtered(func, apply_result_aliases, |dst| {
        !state_map.is_scalar(dst)
    })
}

/// Compute the unified same-allocation alias table: the §1.9
/// `project_alias_sources` map (R1 Project, R3 Jump-arg, R4 CFG-merge, R6
/// nested-Project transitive) WIDENED with R2 generalized to whole-var Let
/// identity of non-projected roots + R5 Select aliases. The whole-var Let
/// identity is the same-allocation relation `compute_same_alloc_reps` carried
/// independently (Spec: Annex E §AIMS — DP-5 alias closure); unifying it here
/// retires the strictly-weaker parallel tracker. R5 Select dsts are recorded in
/// `select_alias_dsts` so the backward-demand consumer excludes them (R5 unions
/// mutually-exclusive operands of DIFFERENT allocations — a DP-5 safety-check
/// over-approximation that must NOT keep both alive via
/// `propagate_project_source_demand`).
#[cfg(test)]
pub(crate) fn compute_project_alias_table(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> ProjectAliasTable {
    compute_project_alias_table_filtered(func, apply_result_aliases, |_| true)
}

fn compute_project_alias_table_filtered(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
    include_project: impl Fn(ArcVarId) -> bool,
) -> ProjectAliasTable {
    // Why: Borrow demand starts from projections; take-project ownership is resolved by the birth-site partition.
    let mut alias_sources: FxHashMap<ArcVarId, ProjectSources> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if include_project(*dst) {
                    alias_sources.insert(*dst, smallvec![*value]);
                }
            }
        }
    }
    let borrow_alias_sources = alias_sources.clone();

    // INVARIANT: Apply-result roots extend safety aliases without implying borrow demand.
    for (apply_dst, alias_source) in apply_result_aliases {
        let roots: ProjectSources = match alias_source {
            ApplyAliasSource::Direct(arg) | ApplyAliasSource::Project { arg, .. } => {
                smallvec![*arg]
            }
            ApplyAliasSource::Wrapped(_) => {
                // Why: Containment is not projection aliasing; inheritance would suppress the inner value's decrement.
                continue;
            }
            ApplyAliasSource::Conditional { candidates } => SmallVec::from_slice(candidates),
        };
        // INVARIANT: SSA destinations cannot be defined by both `Apply` and `Project`.
        merge_sources(&mut alias_sources, *apply_dst, &roots);
    }

    // INVARIANT: Borrow demand excludes whole-value and `Select` aliases to preserve branch-local releases.
    let demand_sources = run_alias_fixpoint(func, borrow_alias_sources, false).0;
    // Spec: Annex E §AIMS — DP-5 alias closure.
    let (sources, select_alias_dsts) = run_alias_fixpoint(func, alias_sources, true);

    ProjectAliasTable {
        sources,
        demand_sources,
        select_alias_dsts,
    }
}

/// GENUINE same-allocation union-find (member -> rep) over the unconditional
/// same-allocation subset of the alias graph: Let{Var} whole-var aliases (edge
/// type 1) + apply-result Direct/Conditional edges (edge type 4;
/// Project/Wrapped excluded). Edge type 2 (Jump-arg -> block-param) and edge
/// type 3 (Select) are intentionally NOT unioned — a Jump-phi / Select merges
/// DIFFERENT runtime allocations into one name when predecessors / operands
/// pass distinct values, so neither is an unconditional same-allocation
/// relation; per-edge attribution ([`compute_param_edge_args`]) resolves a
/// block-param to ONE predecessor's arg instead. Spec: Annex E §AIMS.
#[cfg(test)]
pub(crate) fn compute_genuine_same_alloc_reps(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    fn find(parent: &mut FxHashMap<ArcVarId, ArcVarId>, v: ArcVarId) -> ArcVarId {
        let p = *parent.get(&v).unwrap_or(&v);
        if p == v {
            return v;
        }
        let r = find(parent, p);
        parent.insert(v, r);
        r
    }
    fn union(parent: &mut FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    // Edge type 1: Let{Var} aliases.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                union(&mut parent, *dst, *src);
            }
        }
    }
    // Edge type 4: apply-result Direct + Conditional (Project/Wrapped excluded).
    for (&dst, source) in apply_result_aliases {
        match source {
            ApplyAliasSource::Direct(arg) => union(&mut parent, dst, *arg),
            ApplyAliasSource::Conditional { candidates } => {
                for &cand in candidates {
                    union(&mut parent, dst, cand);
                }
            }
            ApplyAliasSource::Project { .. } | ApplyAliasSource::Wrapped(_) => {}
        }
    }
    let keys: Vec<ArcVarId> = parent.keys().copied().collect();
    let mut reps = FxHashMap::default();
    for v in keys {
        let r = find(&mut parent, v);
        reps.insert(v, r);
    }
    reps
}

/// Per-predecessor-edge attribution for every block-param (§1.9 merge-point
/// filtering): map each block-param var to the (`pred_block`, Jump-arg,
/// back-edge?) triple per incoming `Jump` edge. A block-param resolves to THAT
/// predecessor edge's arg — NEVER to the merged R4 union (which
/// over-approximates across paths). Consumers gate on `is_back_edge` to
/// decline loop-header unification of a forward-init value with a back-edge
/// iteration value. Spec: Annex E §AIMS.
pub(crate) fn compute_param_edge_args(
    func: &ArcFunction,
) -> FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>> {
    use crate::graph::successor_block_ids;
    // Forward reachability per block (successor closure), for back-edge
    // classification: edge (pred -> target) is a back-edge iff pred is
    // reachable FROM target.
    let n = func.blocks.len();
    let mut reach: Vec<FxHashSet<usize>> = vec![FxHashSet::default(); n];
    for (start, set) in reach.iter_mut().enumerate() {
        let mut stack: Vec<usize> = successor_block_ids(&func.blocks[start].terminator)
            .into_iter()
            .map(crate::ir::ArcBlockId::index)
            .collect();
        while let Some(b) = stack.pop() {
            if b >= n || !set.insert(b) {
                continue;
            }
            for s in successor_block_ids(&func.blocks[b].terminator) {
                stack.push(s.index());
            }
        }
    }
    let mut edges: FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>> = FxHashMap::default();
    for (pred_block, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Jump { target, args } = &block.terminator else {
            continue;
        };
        let target_idx = target.index();
        if target_idx >= n {
            continue;
        }
        let is_back_edge = reach[target_idx].contains(&pred_block) || target_idx == pred_block;
        for (i, &arg) in args.iter().enumerate() {
            if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                edges.entry(param_var).or_default().push(ParamEdgeArg {
                    pred_block,
                    arg,
                    is_back_edge,
                });
            }
        }
    }
    edges
}

/// Run the transitive alias fixed point (R6 nested-Project + Jump-arg/CFG-merge
/// renaming) over a seeded `alias_sources`. When `seed_r2gen` is true, ALSO seed
/// R2-generalized whole-var Let identity (`dst → [src]` for a `Let { Var }` whose
/// src has no sources) + R5 Select aliases, returning the Select-origin dst set.
/// When false, the R2-gen seed + R5 are skipped (the original §1.9
/// backward-demand table). Spec: Annex E §AIMS.
fn run_alias_fixpoint(
    func: &ArcFunction,
    mut alias_sources: FxHashMap<ArcVarId, ProjectSources>,
    seed_r2gen: bool,
) -> (FxHashMap<ArcVarId, ProjectSources>, FxHashSet<ArcVarId>) {
    let mut select_alias_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            // Let aliases within the block body. When `src` carries project /
            // apply / merge sources, extend `dst` with them transitively (the
            // original §1.9 R2). When `seed_r2gen` and `src` is a NON-PROJECTED
            // root with no sources of its own (a loop block-param, fresh
            // `Construct`, bare param), seed the whole-var identity `dst → [src]`
            // (R2 generalized): `dst` IS `src`'s allocation.
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if let Some(sources) = alias_sources.get(src).cloned() {
                        changed |= merge_sources(&mut alias_sources, *dst, &sources);
                    } else if seed_r2gen {
                        changed |= merge_sources(&mut alias_sources, *dst, &[*src]);
                    }
                }
            }

            // Rule 6 (nested Project transitivity): `%dst = Project %src.field`,
            // where %src has sources S → `%dst → [%src] ∪ S`. Step 1 only seeded
            // `%dst → [%src]`; the transitive ∪ S extension happens here so
            // multi-hop Project chains carry the original aggregate's source
            // through.
            for instr in &block.body {
                if let ArcInstr::Project {
                    dst, value: src, ..
                } = instr
                {
                    if let Some(src_sources) = alias_sources.get(src).cloned() {
                        changed |= merge_sources(&mut alias_sources, *dst, &src_sources);
                    }
                }
            }

            // Rule 5 (Select alias propagation; Spec: Annex E §AIMS — proven
            // sound at DP-5): `%dst = Select(cond, %t, %f)` → `%dst → [%t, %f] ∪
            // sources(%t) ∪ sources(%f)`. Recorded ONLY in the unified table
            // (`seed_r2gen`). `%dst` joins `select_alias_dsts`.
            if seed_r2gen {
                for instr in &block.body {
                    if let ArcInstr::Select {
                        dst,
                        true_val,
                        false_val,
                        ..
                    } = instr
                    {
                        select_alias_dsts.insert(*dst);
                        let mut operand_sources: ProjectSources = smallvec![*true_val, *false_val];
                        if let Some(t_sources) = alias_sources.get(true_val) {
                            operand_sources.extend(t_sources.iter().copied());
                        }
                        if let Some(f_sources) = alias_sources.get(false_val) {
                            operand_sources.extend(f_sources.iter().copied());
                        }
                        changed |= merge_sources(&mut alias_sources, *dst, &operand_sources);
                    }
                }
            }

            // Jump arg → target block param edges (phi-like renaming).
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                if target_idx < func.blocks.len() {
                    for (i, &arg) in args.iter().enumerate() {
                        if let Some(sources) = alias_sources.get(&arg).cloned() {
                            if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                                changed |= merge_sources(&mut alias_sources, param_var, &sources);
                            }
                        }
                    }
                }
            }
        }
    }
    (alias_sources, select_alias_dsts)
}

/// Merge `new_sources` into the source set for `var`, returning true if
/// any new source was added. Deduplicates via linear scan — source sets
/// are almost always 1–2 elements, so this is cheaper than a `HashSet`.
fn merge_sources(
    alias_sources: &mut FxHashMap<ArcVarId, ProjectSources>,
    var: ArcVarId,
    new_sources: &[ArcVarId],
) -> bool {
    let entry = alias_sources.entry(var);
    match entry {
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(SmallVec::from_slice(new_sources));
            true
        }
        std::collections::hash_map::Entry::Occupied(mut e) => {
            let existing = e.get_mut();
            let mut added = false;
            for &src in new_sources {
                if !existing.contains(&src) {
                    existing.push(src);
                    added = true;
                }
            }
            added
        }
    }
}

/// Propagate Project source demand: when a Project result (or its alias)
/// has demand, ALL possible Project sources must also have demand to prevent
/// premature `RcDec`.
///
/// Handles four cases:
/// 1. Direct: `%3 = Project %2.0` in block A, `%3` used in block B
/// 2. Aliased: `%3 = Project %2.0`, `%4 = Let %3`, `%4` used in block B
/// 3. Transitive: chains of `Let` aliases from a Project destination
/// 4. Block params (single predecessor): `Jump block1, args=[%3]`
///
/// Without this, parent aggregates would have no demand at block B's entry,
/// causing edge cleanup to emit premature `RcDec` — use-after-free when
/// the borrowed child is still alive.
///
/// Note: for block params at multi-predecessor merge points, this propagates
/// demand for ALL possible sources (from different predecessors). This is
/// correct for the backward analysis (keeps parent aggregates alive on each
/// predecessor's path) but the emission side must filter: variables that
/// exist only on one predecessor path must NOT get block-level `RcDec` at the
/// merge point.
pub(super) fn propagate_project_source_demand(
    current: &mut BlockState,
    state_map: &AimsStateMap,
    project_alias_sources: &FxHashMap<ArcVarId, ProjectSources>,
    select_alias_dsts: &FxHashSet<ArcVarId>,
    _block_params: &[(ArcVarId, ori_types::Idx)],
) {
    let mut extra_demand: Vec<(ArcVarId, AimsState)> = Vec::new();
    for (var, state) in current.observed_entries() {
        if state.cardinality == Cardinality::Absent {
            continue;
        }
        // R5 Select dsts feed the DP-5 safety check + unified same-allocation
        // closure ONLY, never backward demand: a Select's operands are
        // mutually-exclusive allocations on a SINGLE path (the condition picks
        // one), so propagating demand to BOTH over-extends every path
        // (`generics::test_path_sensitive_select_*`). R4 CFG-merge dsts are NOT
        // excluded here: at a multi-predecessor merge the backward analysis MUST
        // keep every parent alive on ITS OWN predecessor path (§1.9
        // multi-predecessor demand) — the EMISSION side filters per-edge so the
        // untaken parent still gets its branch-edge dec. Spec: Annex E §AIMS.
        if select_alias_dsts.contains(&var) {
            continue;
        }
        if let Some(sources) = project_alias_sources.get(&var) {
            for &source in sources {
                if !state_map.is_excluded(source) {
                    extra_demand.push((source, state));
                }
            }
        }
    }
    for (source, destination) in extra_demand {
        current.transfer_project(source, destination);
    }
}
