//! Edge cleanup for inter-block RC operations.
//!
//! Handles variables that are live in a predecessor but dead in a particular
//! successor. For single-predecessor successors, prepends `RcDec` at the
//! successor's entry. For multi-predecessor successors, inserts trampoline
//! blocks.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::{AimsStateMap, ApplyAliasSource};
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::graph::{compute_predecessors, successor_block_ids};
use crate::ir::{
    ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, RcStrategy,
};

use super::trampoline::{compute_defined_at_or_before, insert_trampoline};
use super::{block_id, rc_strategy, should_suppress_apply_aliased_dec, DeferredDec, EdgeDec};

/// Whether the successor block at `succ_idx` is an unwind block (terminator
/// = Resume).
///
/// For Branch/Switch successors (no explicit unwind/normal distinction in
/// the terminator shape), this is the available signal. For Invoke
/// successors, prefer the explicit `normal`/`unwind` field distinction —
/// `is_unwind_succ_block` is a fallback when the explicit flag is unknown.
///
/// Per BUG-04-090, intermediate blocks reachable via Invoke
/// unwind-successor cluster may not have Resume terminators themselves; this
/// shallow check under-detects unwind blocks for the cluster case. For the
/// bug-pin scope this is sound because hop2/hop4/F-prj/E-mat exercise
/// non-unwind control flow exclusively. F-try (try-block, the unwind-edge
/// case) requires the deeper authoritative `ArcFunction::unwind_blocks`
/// accessor — tracked as a Hypothesis D component #1 follow-up.
#[inline]
fn is_unwind_succ_block(func: &ArcFunction, succ_idx: usize) -> bool {
    matches!(func.blocks[succ_idx].terminator, ArcTerminator::Resume)
}

/// Whether a variable should be treated as owned for RC purposes.
///
/// Same logic as `is_owned_at_entry` but works with raw state and the
/// function-level borrowed defs set. Used in edge cleanup where we don't
/// have the per-block `defined_in_block` context.
#[inline]
fn is_owned_for_rc(
    state_map: &AimsStateMap,
    var: ArcVarId,
    access: AccessClass,
    cardinality: Cardinality,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) -> bool {
    if state_map.is_excluded(var) {
        return false;
    }
    if access == AccessClass::Owned {
        return true;
    }
    // Cross-block variable with access stuck at BOTTOM (Borrowed).
    // Owned unless it was defined by Project.
    if cardinality != Cardinality::Absent {
        return !all_borrowed_defs.contains(&var);
    }
    false
}

/// Emit `RcDec` on edges where a variable is live in the predecessor but
/// dead in a particular successor.
///
/// Also handles deferred `RcDec` operations from two sources:
/// - **Phase B deferred parents** (`target: None`): parent aggregates whose
///   `RcDec` was deferred because a borrowed child (from Project) is used in
///   the block terminator. Emitted on ALL successor edges.
/// - **Merge-edge decs** (`target: Some(succ)`): branch-local variables at
///   merge blocks. Emitted ONLY on the edge to the specific merge successor.
///
/// Union-find representative over the SAME-ALLOCATION subset of the SSA-alias
/// graph: every union edge `compute_ssa_alias_classes` uses EXCEPT edge type 2
/// (Jump-arg → successor block-param). The Jump-phi edge merges DIFFERENT
/// runtime allocations into one class when a block param has predecessors
/// passing distinct values (e.g. `if c then x else y` unions x and y via the
/// merge param), so it is NOT a same-allocation relation. Edges retained:
/// Let{Var} aliases, apply-result Direct + Conditional (Project/Wrapped already
/// excluded by PIN-2). Used by the PIN-4 class-liveness suppression in
/// `collect_branch_edge_decs` so only a TRUE same-allocation alias being live
/// at a successor suppresses `var`'s edge dec — phi-merged alternatives must
/// not (RL-4 P1 + §10 under-elimination-leaks per-path-net-0 invariant).
pub(crate) fn compute_same_alloc_reps(
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
    // Edge type 4: apply-result Direct + Conditional (Project/Wrapped excluded
    // per PIN-2, matching compute_ssa_alias_classes). Edge type 2 (Jump-phi)
    // and edge type 3 (Select, already dropped) are intentionally NOT unioned.
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

/// Whether `a` and `b` denote the same runtime allocation (same
/// `compute_same_alloc_reps` rep). A var with no entry is its own rep.
pub(crate) fn same_alloc(reps: &FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) -> bool {
    reps.get(&a).copied().unwrap_or(a) == reps.get(&b).copied().unwrap_or(b)
}

/// Function-wide analysis context shared by the edge-dead-set collectors.
///
/// Bundles the `ArcFunction`, state map, type pool, borrowed-def set,
/// take-move facts, and same-allocation reps. Every field is a shared borrow
/// read together by `collect_branch_edge_decs` / `compute_branch_edge_dead_set`
/// / `collect_invoke_edge_decs` / `compute_invoke_edge_dead_set`.
#[derive(Clone, Copy)]
pub(crate) struct EdgeCleanupEnv<'a> {
    func: &'a ArcFunction,
    state_map: &'a AimsStateMap,
    pool: &'a Pool,
    all_borrowed_defs: &'a FxHashSet<ArcVarId>,
    take_move_facts: &'a super::take_project::TakeMoveFacts,
    same_alloc_reps: &'a FxHashMap<ArcVarId, ArcVarId>,
}

pub(crate) fn emit_edge_cleanup(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    take_move_facts: &super::take_project::TakeMoveFacts,
    deferred_parent_decs: &FxHashMap<usize, Vec<DeferredDec>>,
) {
    let predecessors = compute_predecessors(func);
    let same_alloc_reps = compute_same_alloc_reps(func, state_map.apply_result_aliases());

    // Collect edge cleanup operations: (pred_block, succ_block, var, strategy).
    let mut edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)> = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);
        let env = EdgeCleanupEnv {
            func,
            state_map,
            pool,
            all_borrowed_defs,
            take_move_facts,
            same_alloc_reps: &same_alloc_reps,
        };

        // Handle Invoke/InvokeIndirect separately — use InvokeEdgeState.
        if matches!(
            block.terminator,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            collect_invoke_edge_decs(env, block_idx, &mut edge_decs);
            // Add deferred decs to Invoke edges. target=None → both edges,
            // target=Some(succ) → only the matching edge.
            if let Some(decs) = deferred_parent_decs.get(&block_idx) {
                if let ArcTerminator::Invoke { normal, unwind, .. }
                | ArcTerminator::InvokeIndirect { normal, unwind, .. } = &block.terminator
                {
                    for &(target, var, strategy) in decs {
                        match target {
                            None => {
                                edge_decs.push((block_idx, normal.index(), var, strategy));
                                edge_decs.push((block_idx, unwind.index(), var, strategy));
                            }
                            Some(succ) if succ == normal.index() => {
                                edge_decs.push((block_idx, normal.index(), var, strategy));
                            }
                            Some(succ) if succ == unwind.index() => {
                                edge_decs.push((block_idx, unwind.index(), var, strategy));
                            }
                            Some(succ) => {
                                debug_assert!(
                                    false,
                                    "merge-edge dec targets block {succ} which is neither \
                                     normal ({}) nor unwind ({}) of Invoke in block {block_idx}",
                                    normal.index(),
                                    unwind.index(),
                                );
                            }
                        }
                    }
                }
            }
            continue;
        }

        let successors = successor_block_ids(&block.terminator);

        // Add deferred decs to successor edges. target=None → all edges,
        // target=Some(succ) → only the matching edge.
        if let Some(decs) = deferred_parent_decs.get(&block_idx) {
            for &(target, var, strategy) in decs {
                match target {
                    None => {
                        for succ_id in &successors {
                            edge_decs.push((block_idx, succ_id.index(), var, strategy));
                        }
                    }
                    Some(succ) => {
                        debug_assert!(
                            successors.iter().any(|s| s.index() == succ),
                            "merge-edge dec targets block {succ} which is not a successor of block {block_idx}",
                        );
                        if successors.iter().any(|s| s.index() == succ) {
                            edge_decs.push((block_idx, succ, var, strategy));
                        }
                    }
                }
            }
        }

        if successors.len() <= 1 {
            continue; // Single successor: no edge-specific cleanup needed.
        }

        // Multiple successors: check each edge for dead variables.
        collect_branch_edge_decs(env, block_idx, blk, &successors, &mut edge_decs);
    }

    apply_edge_decs(func, &predecessors, edge_decs);
}

/// Collect branch/switch/jump edge `RcDec`s by mapping the shared edge-dead-set
/// to `(pred, succ, var, RcStrategy)`. The dead-set itself is the SSOT consumed
/// by both this predicate-stack emitter and burden-op emission.
fn collect_branch_edge_decs(
    env: EdgeCleanupEnv<'_>,
    block_idx: usize,
    blk: ArcBlockId,
    successors: &[ArcBlockId],
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    for (pred, succ, var) in compute_branch_edge_dead_set(env, block_idx, blk, successors) {
        if let Some(strategy) = rc_strategy(env.func, var, env.pool) {
            edge_decs.push((pred, succ, var, strategy));
        }
    }
}

/// Pure branch/switch/jump edge-dead-set: which `(pred_block, succ_block, var)`
/// triples have `var` owned-non-scalar live at `pred` exit but dead (Absent) at
/// `succ` entry, after take-move-class / apply-aliased / same-alloc-member-live
/// / per-edge-class-dedup suppression. SSOT consumed by both `edge_cleanup`
/// (`RcDec`) and burden-op emission (`BurdenDec`); strategy re-derived per var.
#[expect(
    clippy::too_many_lines,
    reason = "single edge-dead-set analysis pass — extracting would fragment the per-edge suppression logic"
)]
pub(crate) fn compute_branch_edge_dead_set(
    env: EdgeCleanupEnv<'_>,
    block_idx: usize,
    blk: ArcBlockId,
    successors: &[ArcBlockId],
) -> Vec<(usize, usize, ArcVarId)> {
    let EdgeCleanupEnv {
        func,
        state_map,
        pool,
        all_borrowed_defs,
        take_move_facts,
        same_alloc_reps,
    } = env;
    let mut dead_set: Vec<(usize, usize, ArcVarId)> = Vec::new();
    let Some(exit_states) = state_map.block_exit_states(blk) else {
        return dead_set;
    };

    // Filter out variables defined downstream (from project-source demand
    // propagation at merge points).
    let defined_at_or_before = compute_defined_at_or_before(func, block_idx);

    // BUG-04-104 PIN-5: per-edge class-id tracking for same-edge batching.
    let mut classes_inserted_per_edge: FxHashMap<(usize, usize), FxHashSet<u32>> =
        FxHashMap::default();

    for (&var, &state) in exit_states {
        if state.is_scalar() {
            continue;
        }
        if !is_owned_for_rc(
            state_map,
            var,
            state.access,
            state.cardinality,
            all_borrowed_defs,
        ) {
            continue;
        }
        // skip vars that participate in a
        // take-project alias class. Their scope-exit drops are
        // emitted by `dead_cleanup` source 1's in-class branch on
        // bypass-safe blocks (per-class), with class-deduped
        // semantics. Letting edge cleanup also emit a dec for an
        // alias sibling (e.g., `%19 = %5` Let alias) would
        // double-free the shared underlying value.
        if take_move_facts.is_in_class(var) {
            tracing::debug!(
                target: "ori_arc::aims::realize::edge_cleanup",
                func = ?func.name, var = var.raw(), block = block_idx,
                reason = "in-take-move-class",
                "RL-4 branch-edge-dec SUPPRESSED"
            );
            continue;
        }
        // Skip variables that are only defined downstream (in a successor
        // block). These come from project-source demand propagation at
        // merge points and don't actually exist at this block's exit.
        if !defined_at_or_before.contains(&var) {
            tracing::debug!(
                target: "ori_arc::aims::realize::edge_cleanup",
                func = ?func.name, var = var.raw(), block = block_idx,
                reason = "not-defined-at-or-before",
                "RL-4 branch-edge-dec SUPPRESSED"
            );
            continue;
        }
        for succ_id in successors {
            let succ_entry = state_map.var_state_at_block_entry(*succ_id, var);
            if (succ_entry.cardinality == Cardinality::Absent
                || !is_owned_for_rc(
                    state_map,
                    var,
                    succ_entry.access,
                    succ_entry.cardinality,
                    all_borrowed_defs,
                ))
                && rc_strategy(func, var, pool).is_some()
            {
                // BUG-04-090: suppress
                // edge dec when `var` was consumed by an Apply/Invoke
                // whose dst aliases it.
                let is_unwind_succ = is_unwind_succ_block(func, succ_id.index());
                if should_suppress_apply_aliased_dec(state_map, var, is_unwind_succ) {
                    tracing::debug!(
                        target: "ori_arc::aims::realize::edge_cleanup",
                        func = ?func.name, var = var.raw(),
                        block = block_idx, succ = succ_id.index(),
                        reason = "apply-aliased-dst",
                        "RL-4 branch-edge-dec SUPPRESSED"
                    );
                    continue;
                }
                // BUG-04-104 PIN-4 + PIN-5: class-aware skip + per-edge
                // batching. Skip when any class member is live at the
                // successor's entry (PIN-4), or when the same class
                // already emitted a dec for this (pred, succ) edge in
                // this collection pass (PIN-5).
                if let Some(class_id) = state_map.ssa_alias_class_of(var) {
                    if let Some(members) = state_map.class_members(class_id) {
                        // Only a TRUE same-allocation alias being live at the
                        // successor may suppress var's edge dec. Phi-merged
                        // alternatives (distinct allocations unioned via a
                        // Jump-arg→block-param merge param, e.g. `if c then x
                        // else y`) must NOT suppress — each alternative needs
                        // its own edge dec on the branch where it dies
                        // (RL-4 P1 + §10 under-elimination per-path-net-0).
                        // BUG-04-123 sibling (project-merge): a same-alloc
                        // member `m` may only carry `var`'s drop across the
                        // THIS-block→succ edge if it actually EXISTS at this
                        // block (defined-at-or-before `block_idx`, mirroring
                        // the `var` guard above). A member defined only on a
                        // SIBLING branch (e.g. `%15 = %p2` in the else arm)
                        // is reported `Once`-live at the then-successor entry
                        // by `var_state_at_block_entry` (backward alias-demand
                        // bleed) but is NOT reachable on this edge — counting
                        // it phantom-suppresses the not-taken parent's edge
                        // dec → leak (`04B.2-under-elim` per-path-net-0).
                        if let Some(&m_live) = members.iter().find(|&&m| {
                            defined_at_or_before.contains(&m)
                                && same_alloc(same_alloc_reps, m, var)
                                && state_map.var_state_at_block_entry(*succ_id, m).cardinality
                                    != Cardinality::Absent
                        }) {
                            tracing::debug!(
                                target: "ori_arc::aims::realize::edge_cleanup",
                                func = ?func.name, var = var.raw(),
                                block = block_idx, succ = succ_id.index(),
                                member = m_live.raw(),
                                member_card = ?state_map
                                    .var_state_at_block_entry(*succ_id, m_live)
                                    .cardinality,
                                reason = "pin4-same-alloc-member-live",
                                "RL-4 branch-edge-dec SUPPRESSED"
                            );
                            continue;
                        }
                    }
                    let edge_key = (block_idx, succ_id.index());
                    let edge_classes = classes_inserted_per_edge.entry(edge_key).or_default();
                    if !edge_classes.insert(class_id) {
                        continue;
                    }
                }
                dead_set.push((block_idx, succ_id.index(), var));
            }
        }
    }
    dead_set
}

/// Apply collected edge decs: prepend for single-pred successors, trampoline
/// for multi-pred successors.
fn apply_edge_decs(
    func: &mut ArcFunction,
    predecessors: &[Vec<usize>],
    edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    if tracing::enabled!(tracing::Level::DEBUG) {
        let burden_true = func.burden_emitted.iter().filter(|b| **b).count();
        let edge_vars: Vec<String> = edge_decs
            .iter()
            .map(|(p, s, v, _)| {
                let be = func.burden_emitted.get(v.index()).copied().unwrap_or(false);
                format!("bb{p}->bb{s}:%{}(burden={be})", v.index())
            })
            .collect();
        tracing::debug!(
            target: "ori_arc::aims::realize::edge_cleanup",
            func = ?func.name,
            burden_emitted_true = burden_true,
            burden_emitted_len = func.burden_emitted.len(),
            edge_decs = %edge_vars.join(" "),
            "apply_edge_decs entry",
        );
    }
    let mut edge_groups: FxHashMap<(usize, usize), Vec<EdgeDec>> = FxHashMap::default();
    for (pred, succ, var, strategy) in edge_decs {
        edge_groups
            .entry((pred, succ))
            .or_default()
            .push((var, strategy));
    }

    let mut trampolines: Vec<(usize, usize, Vec<EdgeDec>)> = Vec::new();

    for ((pred, succ), decs) in &edge_groups {
        if predecessors[*succ].len() == 1 {
            // Faithful release: `BurdenDec` paired adjacent to each edge
            // `RcDec` whose var carries burden ops — per-value burden ledger
            // nets 0 across this CFG edge (RL-4). The edge variant suppresses
            // the burden dec for an owned-transfer arg of `pred`'s terminator
            // (already balanced at the transfer point).
            let dec_instrs: Vec<ArcInstr> = decs
                .iter()
                .flat_map(|&(var, strategy)| {
                    super::release_with_burden_edge(func, *pred, var, strategy)
                })
                .collect();
            let body = &mut func.blocks[*succ].body;
            let mut new_body = dec_instrs;
            new_body.append(body);
            *body = new_body;
        } else {
            trampolines.push((*pred, *succ, decs.clone()));
        }
    }

    for (pred, succ, decs) in trampolines {
        insert_trampoline(func, pred, succ, &decs);
    }
}

/// Collect edge-specific `RcDec` for Invoke terminators using
/// `InvokeEdgeState` to determine normal vs unwind cleanup.
///
/// Three cleanup categories: (1) `exit_states` vars dying on specific edges,
/// (2) borrowed `Invoke` args absent from `exit_states`, (3) unwind cleanup for
/// ownership-transferred args. Categories 1-2 exclude `Owned` normal-path
/// transfers; category 3 handles the unwind path for those same variables.
fn collect_invoke_edge_decs(
    env: EdgeCleanupEnv<'_>,
    block_idx: usize,
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    for (pred, succ, var) in compute_invoke_edge_dead_set(env, block_idx) {
        if let Some(strategy) = rc_strategy(env.func, var, env.pool) {
            edge_decs.push((pred, succ, var, strategy));
        }
    }
}

/// Pure Invoke/InvokeIndirect edge-dead-set (3 cleanup categories: exit-state
/// vars dying per-edge, borrowed args absent from `exit_states`, unwind cleanup
/// for owned args). SSOT consumed by both `edge_cleanup` (`RcDec`) and burden-op
/// emission (`BurdenDec`); strategy re-derived per var by the caller.
#[expect(
    clippy::too_many_lines,
    reason = "3 cleanup categories in one function — extracting would fragment edge logic"
)]
#[expect(
    clippy::cognitive_complexity,
    reason = "BUG-04-090 added per-category apply_result_aliases gates; \
              splitting Cat 1/2/3 into helpers would obscure the unwind-vs-normal contract"
)]
pub(crate) fn compute_invoke_edge_dead_set(
    env: EdgeCleanupEnv<'_>,
    block_idx: usize,
) -> Vec<(usize, usize, ArcVarId)> {
    let EdgeCleanupEnv {
        func,
        state_map,
        pool,
        all_borrowed_defs,
        take_move_facts,
        same_alloc_reps,
    } = env;
    let mut edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)> = Vec::new();
    let blk = block_id(block_idx);
    let (ArcTerminator::Invoke {
        normal,
        unwind,
        args,
        arg_ownership,
        ..
    }
    | ArcTerminator::InvokeIndirect {
        normal,
        unwind,
        args,
        arg_ownership,
        ..
    }) = &func.blocks[block_idx].terminator
    else {
        return Vec::new();
    };
    let (normal, unwind) = (*normal, *unwind);
    // InvokeIndirect uses conservative default (Borrowed) for unannotated args,
    // unlike Invoke which defaults to Owned. This affects ownership checks below.
    let is_indirect = matches!(
        func.blocks[block_idx].terminator,
        ArcTerminator::InvokeIndirect { .. }
    );

    let edge_state = state_map.invoke_edge_state(blk);
    let exit_states = state_map.block_exit_states(blk);

    // Cat 3 vars — so Cat 1 can skip to avoid double-dec.
    let mut cat3_unwind_vars: FxHashSet<ArcVarId> = FxHashSet::default();

    // BUG-04-104 PIN-5: per-edge class-id tracking for same-edge batching
    // across Categories 1 + 2.
    let mut classes_inserted_per_edge: FxHashMap<(usize, usize), FxHashSet<u32>> =
        FxHashMap::default();

    // Category 3: unwind cleanup for Owned args (callee may not have consumed).
    for (i, &arg) in args.iter().enumerate() {
        let is_owned = is_arg_owned(arg_ownership, i, is_indirect);
        if !is_owned {
            continue;
        }
        if state_map.is_excluded(arg) {
            continue;
        }
        if all_borrowed_defs.contains(&arg) {
            continue;
        }
        if let Some(strategy) = rc_strategy(func, arg, pool) {
            // BUG-04-104 PIN-5: per-edge class batching across Cat 3 and
            // Cat 1/2. If `arg` is in a class, mark the class as inserted on
            // the unwind edge so subsequent Cat 1 / Cat 2 emissions skip.
            if let Some(class_id) = state_map.ssa_alias_class_of(arg) {
                let edge_classes = classes_inserted_per_edge
                    .entry((block_idx, unwind.index()))
                    .or_default();
                if !edge_classes.insert(class_id) {
                    continue;
                }
            }
            edge_decs.push((block_idx, unwind.index(), arg, strategy));
            cat3_unwind_vars.insert(arg);
        }
    }

    // Precompute which variables are defined at or before this block.
    let defined_at_or_before = compute_defined_at_or_before(func, block_idx);

    // Category 1: variables in exit_states that die on specific edges.
    if let (Some(edge_state), Some(exit_states)) = (edge_state, exit_states) {
        for (&var, &state) in exit_states {
            if state.is_scalar() {
                continue;
            }
            if !is_owned_for_rc(
                state_map,
                var,
                state.access,
                state.cardinality,
                all_borrowed_defs,
            ) {
                continue;
            }
            // take-project alias-class
            // members are dec'd by `dead_cleanup` source 1's
            // in-class branch on per-class bypass-safe blocks.
            // Skipping here prevents an alias-sibling double-free
            // (e.g., `%5` and its Let alias `%19` resolve to the
            // same memory).
            if take_move_facts.is_in_class(var) {
                continue;
            }
            // Skip variables defined downstream (from project-source
            // demand propagation). Same filter as collect_branch_edge_decs.
            if !defined_at_or_before.contains(&var) {
                continue;
            }
            // Ownership transferred → callee handles normal cleanup.
            if invoke_transfers_ownership(var, args, arg_ownership, is_indirect) {
                continue;
            }

            // Check unwind path — skip if already handled by Category 3.
            if !cat3_unwind_vars.contains(&var) {
                let unwind_state = edge_state
                    .unwind
                    .get(&var)
                    .copied()
                    .unwrap_or(crate::aims::lattice::AimsState::BOTTOM);
                if unwind_state.cardinality == Cardinality::Absent {
                    if let Some(strategy) = rc_strategy(func, var, pool) {
                        // BUG-04-104 PIN-5: per-edge class batching for the
                        // unwind edge. If the class already emitted on the
                        // unwind edge (e.g., via Cat 3), skip.
                        let mut emit = true;
                        if let Some(class_id) = state_map.ssa_alias_class_of(var) {
                            let edge_classes = classes_inserted_per_edge
                                .entry((block_idx, unwind.index()))
                                .or_default();
                            if !edge_classes.insert(class_id) {
                                emit = false;
                            }
                        }
                        if emit {
                            edge_decs.push((block_idx, unwind.index(), var, strategy));
                        }
                    }
                }
            }

            // Check normal path.
            // BUG-04-090: gate the normal
            // edge dec via `apply_result_aliases`. The unwind edge above
            // is unaffected per `RL-4` (unwind paths
            // always emit cleanup decs). Per component #2, the gate
            // cannot match a Let-alias var.
            let normal_state = edge_state
                .normal
                .get(&var)
                .copied()
                .unwrap_or(crate::aims::lattice::AimsState::BOTTOM);
            if normal_state.cardinality == Cardinality::Absent {
                if let Some(strategy) = rc_strategy(func, var, pool) {
                    if !should_suppress_apply_aliased_dec(state_map, var, false) {
                        // BUG-04-104 PIN-4 + PIN-5: class-aware skip + batch.
                        let mut emit = true;
                        if let Some(class_id) = state_map.ssa_alias_class_of(var) {
                            if let Some(members) = state_map.class_members(class_id) {
                                // Same-allocation-only suppression (RL-4 P1):
                                // phi-merged alternatives must not suppress.
                                if members.iter().any(|&m| {
                                    same_alloc(same_alloc_reps, m, var)
                                        && state_map.var_state_at_block_entry(normal, m).cardinality
                                            != Cardinality::Absent
                                }) {
                                    emit = false;
                                }
                            }
                            if emit {
                                let edge_classes = classes_inserted_per_edge
                                    .entry((block_idx, normal.index()))
                                    .or_default();
                                if !edge_classes.insert(class_id) {
                                    emit = false;
                                }
                            }
                        }
                        if emit {
                            edge_decs.push((block_idx, normal.index(), var, strategy));
                        }
                    }
                }
            }
        }
    }

    // Category 2: borrowed Invoke/InvokeIndirect args absent from exit_states —
    // caller must still RcDec. Emit on both edges. Owned args excluded (transferred).
    for (i, &arg) in args.iter().enumerate() {
        if is_arg_owned(arg_ownership, i, is_indirect) {
            continue;
        }
        if state_map.is_excluded(arg) {
            continue;
        }
        // Skip Project-defined (borrowed) vars — source aggregate handles RC.
        if all_borrowed_defs.contains(&arg) {
            continue;
        }
        let in_exit_states = exit_states.is_some_and(|states| {
            states.get(&arg).is_some_and(|s| {
                is_owned_for_rc(state_map, arg, s.access, s.cardinality, all_borrowed_defs)
            })
        });
        if in_exit_states {
            continue;
        }
        if let Some(strategy) = rc_strategy(func, arg, pool) {
            // BUG-04-090: gate normal edge
            // dec via `apply_result_aliases`. Unwind edge always fires per
            // `RL-4`.
            if !should_suppress_apply_aliased_dec(state_map, arg, false) {
                edge_decs.push((block_idx, normal.index(), arg, strategy));
            }
            edge_decs.push((block_idx, unwind.index(), arg, strategy));
        }
    }
    edge_decs
        .into_iter()
        .map(|(p, s, v, _)| (p, s, v))
        .collect()
}

/// Check whether arg at position `i` has Owned ownership, respecting the
/// indirect-call default (empty = Borrowed for indirect, Owned for direct).
#[inline]
fn is_arg_owned(arg_ownership: &[ArgOwnership], i: usize, is_indirect: bool) -> bool {
    if is_indirect {
        arg_ownership
            .get(i)
            .is_some_and(|o| *o == ArgOwnership::Owned)
    } else {
        arg_ownership
            .get(i)
            .is_none_or(|o| *o == ArgOwnership::Owned)
    }
}

/// Check whether an Invoke transfers ownership of `var` at any argument position.
///
/// If the variable appears at any `Owned` position, the callee takes ownership
/// and the caller must not emit cleanup for that variable. Conservative: if the
/// same variable appears at multiple positions and ANY is Owned, treat as
/// transferred (the callee received at least one owned reference).
///
/// `is_indirect` controls the default for unannotated (empty) `arg_ownership`:
/// - `false` (direct call): missing entries default to Owned (`is_none_or`)
/// - `true` (indirect call): missing entries default to Borrowed (`is_some_and`)
#[inline]
fn invoke_transfers_ownership(
    var: ArcVarId,
    args: &[ArcVarId],
    arg_ownership: &[ArgOwnership],
    is_indirect: bool,
) -> bool {
    args.iter()
        .enumerate()
        .any(|(i, &arg)| arg == var && is_arg_owned(arg_ownership, i, is_indirect))
}
