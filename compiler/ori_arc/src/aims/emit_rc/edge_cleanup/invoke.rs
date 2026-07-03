//! Invoke/InvokeIndirect edge cleanup — the three-category dead-set
//! (`compute_invoke_edge_dead_set`) plus the Phase-6.98 unwind pair-net release.
//!
//! Category 1: `exit_states` vars dying on specific normal/unwind edges.
//! Category 2: borrowed `Invoke` args absent from `exit_states`.
//! Category 3: unwind cleanup for ownership-transferred (Owned) args.
//! The `same_alloc_member_live_at` probe is the shared `deadAtSucc` SSOT for the
//! Category-1 normal-edge gate and the Category-2 borrowed-arg gate. Spec: Annex
//! E §AIMS RL-4.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::Cardinality;
use crate::ir::{ArcBlockId, ArcTerminator, ArcVarId, ArgOwnership, RcStrategy};

use super::super::trampoline::compute_defined_at_or_before;
use super::super::{block_id, rc_strategy, should_suppress_apply_aliased_dec};
use super::{is_owned_for_rc, same_alloc, EdgeCleanupEnv};

/// Collect edge-specific `RcDec` for Invoke terminators using
/// `InvokeEdgeState` to determine normal vs unwind cleanup.
///
/// Three cleanup categories: (1) `exit_states` vars dying on specific edges,
/// (2) borrowed `Invoke` args absent from `exit_states`, (3) unwind cleanup for
/// ownership-transferred args. Categories 1-2 exclude `Owned` normal-path
/// transfers; category 3 handles the unwind path for those same variables.
pub(crate) fn collect_invoke_edge_decs(
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
    reason = "per-category apply_result_aliases gates; splitting Cat 1/2/3 into \
              helpers would obscure the unwind-vs-normal contract"
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
        // `same_alloc_reps` reaches the Category-1 / Category-2 same-alloc gates
        // through `env` (passed to `same_alloc_member_live_at`), not a local.
        ..
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

    // PIN-5: per-edge class-id tracking for same-edge batching
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
            // PIN-5: per-edge class batching across Cat 3 and Cat 1/2. Marking
            // `arg`'s class on the unwind edge makes subsequent Cat 1 / Cat 2
            // emissions of the same class skip.
            if !emit_class_once(
                state_map,
                &mut classes_inserted_per_edge,
                (block_idx, unwind.index()),
                arg,
            ) {
                continue;
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
            // Take-project alias-class members are dec'd by `dead_cleanup`
            // source 1's in-class branch; skip here to avoid an
            // alias-sibling double-free (e.g. `%5` and Let-alias `%19`).
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
                        // PIN-5: per-edge class batching for the unwind edge.
                        // If the class already emitted (e.g., via Cat 3), skip.
                        if emit_class_once(
                            state_map,
                            &mut classes_inserted_per_edge,
                            (block_idx, unwind.index()),
                            var,
                        ) {
                            edge_decs.push((block_idx, unwind.index(), var, strategy));
                        }
                    }
                }
            }

            // Check normal path: gate via `apply_result_aliases`. The unwind
            // edge above is unaffected per RL-4 (always emits cleanup decs);
            // the gate cannot match a Let-alias var.
            let normal_state = edge_state
                .normal
                .get(&var)
                .copied()
                .unwrap_or(crate::aims::lattice::AimsState::BOTTOM);
            if normal_state.cardinality == Cardinality::Absent {
                if let Some(strategy) = rc_strategy(func, var, pool) {
                    if !should_suppress_apply_aliased_dec(state_map, var, false) {
                        // PIN-4 + PIN-5 class-aware skip via the shared
                        // `same_alloc_member_live_at` SSOT, ghost-inclusive +
                        // no self-short-circuit mode — see that fn's doc.
                        let suppressed = same_alloc_member_live_at(
                            env,
                            var,
                            normal,
                            &defined_at_or_before,
                            false,
                            false,
                        );
                        // PIN-5 per-edge class batching stays inline — it is the
                        // class-dedup axis, orthogonal to the same-alloc liveness
                        // probe above; only class-bearing vars are batched.
                        let mut emit = !suppressed;
                        if emit
                            && state_map.ssa_alias_class_of(var).is_some()
                            && !emit_class_once(
                                state_map,
                                &mut classes_inserted_per_edge,
                                (block_idx, normal.index()),
                                var,
                            )
                        {
                            emit = false;
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
            // Why: RL-4 deadAtSucc conjunct (axes true, true — see that fn's
            // doc). No per-edge class batching: Cat 2's ARG positions are
            // distinct allocations even under one Jump-phi-merged alias class.
            if !same_alloc_member_live_at(env, arg, normal, &defined_at_or_before, true, true)
                && !should_suppress_apply_aliased_dec(state_map, arg, false)
            {
                edge_decs.push((block_idx, normal.index(), arg, strategy));
            }
            if !same_alloc_member_live_at(env, arg, unwind, &defined_at_or_before, true, true) {
                edge_decs.push((block_idx, unwind.index(), arg, strategy));
            }
        }
    }
    edge_decs
        .into_iter()
        .map(|(p, s, v, _)| (p, s, v))
        .collect()
}

/// RL-4 `deadAtSucc` same-alloc liveness probe — the SSOT shared by the Invoke
/// Category-1 normal-edge gate and the Category-2 borrowed-arg gate: true iff a
/// same-allocation (`compute_same_alloc_reps` rep) member of `var`'s SSA-alias
/// class is non-`Absent` (live) at `succ`'s entry (and, when `include_var_self`,
/// `var` itself is live there). Only a TRUE same-allocation alias being live may
/// suppress `var`'s edge dec; phi-merged alternatives (distinct allocations
/// unioned via a Jump-arg block-param merge) are NOT same-alloc and must not
/// suppress (RL-4 P1 plus per-path-net-0).
///
/// # `include_var_self`
///
/// Whether `var` itself being live at `succ` suppresses:
/// - `true` (Category 2 borrowed-arg): `var` live past the call means the
///   per-edge dec would free a still-live value → double-free.
/// - `false` (Category 1 Invoke normal-edge): `InvokeEdgeState.normal` already
///   proved `var` `Absent` on THIS edge; a merge-level self short-circuit
///   would over-suppress a genuine release.
///
/// # `require_defined_at_or_before`
///
/// Selects the ghost-member discipline:
/// - `true` (Category 2 borrowed-arg): a same-alloc member suppresses only
///   when it actually EXISTS at this block. A member defined only on a
///   sibling branch (a ghost member) reads live via backward alias-demand
///   bleed but is unreachable on the borrowed-arg edge — counting it would
///   phantom-suppress a genuine release → leak.
/// - `false` (Category 1 Invoke normal-edge): a member live on a sibling arm
///   at the merge successor genuinely keeps the allocation reachable, so
///   counting it is correct; a ghost-exclusive guard would double-free.
///
/// Spec: Annex E §AIMS RL-4.
pub(crate) fn same_alloc_member_live_at(
    env: EdgeCleanupEnv<'_>,
    var: ArcVarId,
    succ: ArcBlockId,
    defined_at_or_before: &FxHashSet<ArcVarId>,
    require_defined_at_or_before: bool,
    include_var_self: bool,
) -> bool {
    let EdgeCleanupEnv {
        state_map,
        same_alloc_reps,
        ..
    } = env;
    if include_var_self
        && state_map.var_state_at_block_entry(succ, var).cardinality != Cardinality::Absent
    {
        return true;
    }
    let Some(class_id) = state_map.ssa_alias_class_of(var) else {
        return false;
    };
    let Some(members) = state_map.class_members(class_id) else {
        return false;
    };
    members.iter().any(|&m| {
        (!require_defined_at_or_before || defined_at_or_before.contains(&m))
            && same_alloc(same_alloc_reps, m, var)
            && state_map.var_state_at_block_entry(succ, m).cardinality != Cardinality::Absent
    })
}

/// PIN-5 per-edge class batching: record `var`'s SSA-alias class on the
/// `(pred, succ)` edge and return whether this is the FIRST member of that class
/// to claim the edge. Returns `true` (emit) when `var` has no class, or when its
/// class was not yet inserted for this edge; `false` (skip) on a subsequent
/// member of an already-claimed class. One dec per alias-class per edge — two
/// aliasing members of one allocation never double-release on the same edge.
fn emit_class_once(
    state_map: &AimsStateMap,
    classes_inserted_per_edge: &mut FxHashMap<(usize, usize), FxHashSet<u32>>,
    edge: (usize, usize),
    var: ArcVarId,
) -> bool {
    match state_map.ssa_alias_class_of(var) {
        Some(class_id) => classes_inserted_per_edge
            .entry(edge)
            .or_default()
            .insert(class_id),
        None => true,
    }
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
