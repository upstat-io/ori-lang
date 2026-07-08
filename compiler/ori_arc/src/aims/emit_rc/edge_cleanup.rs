//! Edge cleanup for inter-block RC operations — dispatch hub.
//!
//! Handles variables that are live in a predecessor but dead in a particular
//! successor. For single-predecessor successors, appends `RcDec` at the
//! successor's END (the RL-2 scope-exit position — a release may transitively
//! run a user `@drop`). For multi-predecessor successors, inserts trampoline
//! blocks. The per-terminator dead-set analysis lives in two sibling category
//! modules: `branch` (Branch/Switch/Jump edges) and `invoke`
//! (Invoke/InvokeIndirect edges + the Phase-6.98 unwind pair-net release). This
//! file owns the shared `EdgeCleanupEnv` + same-alloc reps, the top-level
//! `emit_edge_cleanup` dispatch, and the `apply_edge_decs` trampoline/end-append
//! emission.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::{AimsStateMap, ApplyAliasSource};
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::graph::{compute_predecessors, successor_block_ids};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::trampoline::insert_trampoline;
use super::{block_id, DeferredDec, EdgeDec};

mod branch;
mod invoke;
mod invoke_unwind;

#[cfg(test)]
mod tests;

use branch::collect_branch_edge_decs;
use invoke::collect_invoke_edge_decs;

// Re-export the Invoke Phase-6.98 unwind pair-net release so consumers continue
// to resolve it through `edge_cleanup::` (consumed by `emit_rc::mod`).
pub(crate) use invoke_unwind::emit_invoke_unwind_pair_net_releases;

/// Whether the successor block at `succ_idx` is an unwind block (terminator
/// = Resume).
///
/// For Branch/Switch successors this is the available signal; for Invoke
/// successors, prefer the explicit `normal`/`unwind` field — this is a
/// fallback when that flag is unknown.
///
/// Intermediate blocks in the Invoke unwind-successor cluster may lack a
/// Resume terminator, so this shallow check under-detects there; sound for
/// non-unwind control flow, but the unwind-edge (try-block) case needs the
/// deeper `ArcFunction::unwind_blocks` accessor.
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

/// Union-find representative over the SAME-ALLOCATION subset of the SSA-alias
/// graph: every union edge `compute_ssa_alias_classes` uses EXCEPT edge type 2
/// (Jump-arg → successor block-param) — that edge merges DIFFERENT runtime
/// allocations into one class (e.g. `if c then x else y` unions x and y via
/// the merge param), so it is NOT a same-allocation relation. Edges retained:
/// Let{Var} aliases, apply-result Direct + Conditional (Project/Wrapped
/// already excluded by PIN-2).
///
/// # Consumer
///
/// Used by the PIN-4 class-liveness suppression in `collect_branch_edge_decs`
/// so only a TRUE same-allocation alias being live at a successor suppresses
/// `var`'s edge dec — phi-merged alternatives must not (RL-4 P1 + §10
/// under-elimination-leaks per-path-net-0 invariant).
///
/// Thin projection over the §1.9 unified alias-table construction
/// (`project_aliases::compute_genuine_same_alloc_reps`) — ONE builder behind
/// both this query and the table's over-approximation classification; no
/// parallel tracker. Spec: Annex E §AIMS.
pub(crate) fn compute_same_alloc_reps(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    crate::aims::intraprocedural::project_aliases::compute_genuine_same_alloc_reps(
        func,
        apply_result_aliases,
    )
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

/// Emit `RcDec` on edges where a variable is live in the predecessor but
/// dead in a particular successor.
///
/// Also handles deferred `RcDec` operations from two sources:
/// - **Phase B deferred parents** (`target: None`): parent aggregates whose
///   `RcDec` was deferred because a borrowed child (from Project) is used in
///   the block terminator. Emitted on ALL successor edges.
/// - **Merge-edge decs** (`target: Some(succ)`): branch-local variables at
///   merge blocks. Emitted ONLY on the edge to the specific merge successor.
pub(crate) fn emit_edge_cleanup(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    take_move_facts: &super::take_project::TakeMoveFacts,
    deferred_parent_decs: &FxHashMap<usize, Vec<DeferredDec>>,
    // Probe path (`ORI_DISABLE_PREDICATE_STACK_RC=1`): `burden_only=true`
    // emits `BurdenDec` only (lowered to `RcDec` in Phase 7); default emits
    // predicate-stack `RcDec` plus an adjacent `BurdenDec` marker. Spec: RL-4.
    burden_only: bool,
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
                                unreachable!(
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
                        assert!(
                            successors.iter().any(|s| s.index() == succ),
                            "merge-edge dec targets block {succ} which is not a successor of block {block_idx}",
                        );
                        edge_decs.push((block_idx, succ, var, strategy));
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

    apply_edge_decs(func, &predecessors, edge_decs, burden_only);
}

/// Apply collected edge decs: append at the successor's END for single-pred
/// successors (scope-exit position), trampoline for multi-pred successors.
fn apply_edge_decs(
    func: &mut ArcFunction,
    predecessors: &[Vec<usize>],
    edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)>,
    burden_only: bool,
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
    // Deterministic iteration order (PHASE-29): `insert_trampoline` assigns each
    // new trampoline block's id from `func.blocks.len()` at call time, so the
    // ORDER trampolines are inserted in determines the emitted ArcFunction's
    // block numbering. Iterating the hash map directly would key that ordering
    // on FxHashMap bucket layout instead of the edge identity; sort by
    // `(pred, succ)` first so the same input always produces the same block
    // layout regardless of hash-map internals.
    let mut sorted_groups: Vec<((usize, usize), Vec<EdgeDec>)> = edge_groups.into_iter().collect();
    sorted_groups.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut trampolines: Vec<(usize, usize, Vec<EdgeDec>)> = Vec::new();

    for ((pred, succ), decs) in &sorted_groups {
        if predecessors[*succ].len() == 1 {
            // Faithful release: `BurdenDec` paired per edge `RcDec` nets 0
            // across this CFG edge (RL-4); suppressed for an owned-transfer
            // arg of `pred`'s terminator (already balanced at the transfer).
            // ANY strategy's release may transitively run a user `@drop` —
            // not just `AggregateFields`/`InlineEnum` (whose own field walk
            // is inline): `FatPointer` (`[T]`/`{K:V}`/`Set<T>`) frees its
            // buffer via `elem_dec_fn`, which invokes each Drop-typed
            // element's `@drop`; `HeapPointer` calls the type's generated
            // `_ori_drop$<type>` function, which walks fields exactly like
            // `AggregateFields` when the payload is boxed; `Closure` drops a
            // capture environment that may itself hold Drop-typed captures.
            // Drop glue is a trait-impl fact `ori_arc` cannot see (codegen's
            // `user_drop_method` is that SSOT), so no `RcStrategy` shape is a
            // sound "never observable" proof. Every edge release therefore
            // lands at the END of the successor body — the RL-2 scope-exit
            // position; the var is dead at entry, so the delay is
            // unobservable when no drop fires and correct when one does.
            // Spec: Annex E §AIMS RL-2 + RL-4 + RL-DROP.
            let release = |func: &ArcFunction, var: ArcVarId, strategy: RcStrategy| {
                if burden_only {
                    super::release_burden_only_edge(func, *pred, *succ, var)
                } else {
                    super::release_with_burden_edge(func, *pred, var, strategy)
                }
            };
            let end_instrs: Vec<ArcInstr> = decs
                .iter()
                .flat_map(|&(var, strategy)| release(func, var, strategy))
                .collect();
            let end_len = end_instrs.len();
            func.blocks[*succ].body.extend(end_instrs);
            // Keep `func.spans[*succ]` (indexed `[block_index][instr_index]`
            // in lockstep with `body`, per every other body-mutating pass in
            // this crate) aligned with the appended synthetic releases —
            // `None` for the inserted decs (they have no source span,
            // matching the field doc + every other synthetic-instr span
            // site in this crate).
            if let Some(spans) = func.spans.get_mut(*succ) {
                spans.extend(std::iter::repeat_n(None, end_len));
            }
        } else {
            trampolines.push((*pred, *succ, decs.clone()));
        }
    }

    for (pred, succ, decs) in trampolines {
        insert_trampoline(func, pred, succ, &decs, burden_only);
    }
}
