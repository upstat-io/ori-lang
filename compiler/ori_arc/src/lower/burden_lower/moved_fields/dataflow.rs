//! Pass 3 forward-CFG dataflow engine for the moved-out-fields cluster.
//!
//! Owns the INTERSECT-at-entry worklist fixpoint (`propagate_moved_out_fields`)
//! as its own logical phase, separate from `moved_fields::mod`'s Pass 1
//! collection + Pass 2 transfer + the post-fixpoint union step, plus the
//! fixpoint's supporting pure helpers (`derived_convergence_cap`,
//! `compute_block_local_universe`, `intersect_predecessor_exits`,
//! `union_entry_with_local`). `moved_fields::mod` dispatches into this module
//! for Pass 3, then resumes the union step over its result.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcFunction, ArcVarId};

#[cfg(test)]
use super::super::ctx::MovedFieldsConvergence;
use super::super::BurdenLowerCtx;

/// Pass 3 — forward CFG dataflow propagating moved-field sets via
/// INTERSECT-at-entry merge.
///
/// Computes `entry(B) := INTERSECT over P in predecessors(B): exit(P)`
/// (empty for entry block) and `exit(B) := entry(B) ∪ block_local(B)` for
/// every block in reverse-postorder. Bounded fixpoint via worklist iteration
/// handles back edges; monotonicity of `∪` over a finite field-index set
/// guarantees termination.
///
/// # INTERSECT semantics
///
/// For each var-key in the intersect result, the field set is the
/// intersection of fields moved on EVERY predecessor path. Vars present in
/// only one predecessor's exit set are DROPPED from the intersect (NOT
/// definitely-moved on this path). Per `Spec: Annex E §AIMS RL-2`, this is
/// the architecturally-correct merge — emitting `BurdenDecPartial` with a
/// field skipped only because ONE of N predecessors moved it would be a
/// use-after-free if the run-time execution took a different predecessor.
///
/// # Parameters
///
/// `cap_override` forces the convergence cap in place of the derived
/// `derived_convergence_cap(n, universe_pair_count)`. Production always
/// passes `None` (the derived cap). Convergence-guard tests pass
/// `Some(forced)` to drive `changed == true` at the cap on a genuinely
/// multi-round CFG so the release-active guard fires on the real path — the
/// seam exists because the guard is otherwise unreachable on valid input
/// (rule IA-MF1).
///
/// # Returns
///
/// The set of block indices forward-reachable from `func.entry` (computed
/// once here via `compute_postorder`, the same traversal already needed for
/// the RPO worklist order) so the caller's post-fixpoint union filter reuses
/// it instead of re-traversing the CFG.
pub(super) fn propagate_moved_out_fields(
    ctx: &mut BurdenLowerCtx<'_>,
    func: &ArcFunction,
    cap_override: Option<usize>,
) -> FxHashSet<usize> {
    let n = func.blocks.len();
    if n == 0 {
        return FxHashSet::default();
    }

    let predecessors = compute_predecessors(func);

    // Optimistic-⊤ initialization for MUST-move INTERSECT fixpoint per
    // standard dataflow practice (Kildall 1973; Aho/Lam/Sethi/Ullman
    // chapter 9.3). For an INTERSECT (must-) analysis, non-entry blocks
    // are seeded with the lattice top ⊤ so that INTERSECT with ⊤ acts
    // as identity until each predecessor has been processed at least
    // once. Without ⊤ seeding, a back-edge predecessor's empty initial
    // exit would falsely "intersect away" a fact contributed by a
    // forward predecessor, yielding a strictly-weaker (incorrect)
    // fixpoint at loop-exit blocks.
    //
    // ⊤ here = the universe of all `(project_src, field)` pairs that
    // appear in ANY block_local; no block can possibly move a (var,
    // field) pair outside this universe, so this is a sound upper
    // bound. Entry block stays at ⊥ (empty) — the entry has no
    // predecessors and starts with "nothing yet moved".
    let universe = compute_block_local_universe(&ctx.moved_out_fields_block_local);
    let entry_idx = func.entry.index();
    for b in 0..n {
        if b != entry_idx {
            ctx.moved_out_fields_block_exit[b].clone_from(&universe);
        }
    }

    // Reverse-postorder traversal: visiting blocks in RPO converges on a
    // single pass for DAG IR. Back edges require the outer worklist loop.
    let mut rpo = compute_postorder(func);
    let reachable_blocks: FxHashSet<usize> = rpo.iter().copied().collect();
    rpo.reverse();

    // DERIVED convergence cap per AIMS rule IA-MF1 (implementer-internal;
    // compiled Lean is the authority, no Annex E anchor). The termination
    // measure `Σ_b |exit[b]|` is
    // bounded by `n_blocks * universe_pair_count` (n exit slots, each a subset
    // of the universe of size `universe_pair_count`) and strictly shrinks on
    // every non-fixpoint round (INTERSECT + ∪ are monotone from the ⊤ seed, so
    // every `changed` round removes ≥1 pair from some exit set), so the fixpoint
    // is reached within `n_blocks * universe_pair_count + 1` rounds; the cap
    // adds one round of margin. Proven bounded by
    // `AimsProof.MovedFields::MF1_no_change_at_derived_cap`, which makes the
    // release-active convergence guard unreachable on valid input. NOT a
    // heuristic.
    let universe_pair_count: usize = universe.values().map(FxHashSet::len).sum();
    let iteration_cap =
        cap_override.unwrap_or_else(|| derived_convergence_cap(n, universe_pair_count));
    let mut changed = true;
    let mut rounds = 0usize;
    while changed && rounds < iteration_cap {
        changed = false;
        rounds += 1;
        for &b in &rpo {
            // Compute new entry state: INTERSECT over predecessors' exits.
            // Entry block (or any unreachable block with no predecessors)
            // has empty entry.
            let new_entry =
                intersect_predecessor_exits(&predecessors[b], &ctx.moved_out_fields_block_exit);

            // Compute new exit state: entry ∪ block_local.
            let new_exit = union_entry_with_local(&new_entry, &ctx.moved_out_fields_block_local[b]);

            if ctx.moved_out_fields_block_entry[b] != new_entry {
                ctx.moved_out_fields_block_entry[b] = new_entry;
                changed = true;
            }
            if ctx.moved_out_fields_block_exit[b] != new_exit {
                ctx.moved_out_fields_block_exit[b] = new_exit;
                changed = true;
            }
        }
    }

    // Record the structured convergence outcome (round count + derived cap) for
    // convergence-guard tests before the guard fires (AIMS rule IA-MF1). The
    // production guard reads the local `changed` flag directly; this record is
    // test-observation only.
    #[cfg(test)]
    {
        ctx.moved_fields_convergence = Some(MovedFieldsConvergence {
            rounds,
            iteration_cap,
            converged: !changed,
        });
    }

    // RELEASE-ACTIVE fail-closed convergence guard (a plain `assert!`, NOT a
    // release-stripped `debug_assert!`). The moved-out-fields INTERSECT fixpoint
    // is monotone-descending over a finite lattice, proven to converge within
    // the derived cap by `AimsProof.MovedFields::MF1_no_change_at_derived_cap`
    // (rule IA-MF1), so `changed == true` here is UNREACHABLE on valid input —
    // it signals a compiler bug or a malformed `ArcFunction`, never user error.
    // Firing it is correct fail-closed behavior: a non-converged over-
    // approximated moved-set would suppress an owed `BurdenDec` / narrow a
    // `BurdenDecPartial.skip_fields` (Spec: Annex E §AIMS RL-2) and silently
    // leak. Deliberately NOT routed through the `verify`/`debug_assertions`-
    // gated `run_verify` / `run_burden_balance` surfaces — this guard must fire
    // on the real path in a release build. IA-MF1 authority is the compiled Lean.
    assert!(
        !changed,
        "AIMS moved-out-fields INTERSECT fixpoint (rule IA-MF1) failed to converge in \
         {iteration_cap} rounds (n_blocks={n}, universe_pairs={universe_pair_count}); the \
         fixpoint is proven to converge within the derived cap \
         (AimsProof.MovedFields::MF1_no_change_at_derived_cap), so this is a compiler bug \
         or a malformed ArcFunction, not user error — please report it \
         (fail-closed convergence guard, rule IA-MF1).",
    );

    reachable_blocks
}

/// Derived convergence cap for the moved-out-fields INTERSECT fixpoint per AIMS
/// rule IA-MF1: `n_blocks * universe_pair_count + 2`. `universe_pair_count` =
/// distinct `(project_src, field)` pairs in the block-local universe (the
/// exit-lattice per-slot height). The termination measure `Σ_b |exit[b]|` is
/// bounded by `n_blocks * universe_pair_count` and strictly shrinks each
/// non-fixpoint round, so the fixpoint is reached within that bound + 1 (the
/// trailing +1 is round-margin, saturating to avoid overflow) — proven by
/// `AimsProof.MovedFields::MF1_no_change_at_derived_cap`.
pub(super) fn derived_convergence_cap(n_blocks: usize, universe_pair_count: usize) -> usize {
    n_blocks
        .saturating_mul(universe_pair_count)
        .saturating_add(2)
}

/// Pointwise-union `src`'s per-var field sets into `dest`, in place. The
/// shared merge primitive for every `FxHashMap<ArcVarId, FxHashSet<u32>>`
/// accumulation site in the moved-out-fields cluster — the ⊤-universe seed
/// (`compute_block_local_universe`), the `entry ∪ block_local` per-block
/// transfer (`union_entry_with_local`), and `moved_fields::mod`'s
/// post-fixpoint reachable-block union. Extend callers to use this when a
/// new accumulation site is added; never re-derive the merge inline.
pub(super) fn union_field_map_into(
    dest: &mut FxHashMap<ArcVarId, FxHashSet<u32>>,
    src: &FxHashMap<ArcVarId, FxHashSet<u32>>,
) {
    for (&key, fields) in src {
        let entry = dest.entry(key).or_default();
        for &field in fields {
            entry.insert(field);
        }
    }
}

/// Compute the universe of `(project_src, field)` pairs that appear in
/// ANY block-local moved-field map. This is the lattice top ⊤ for the
/// MUST-move INTERSECT analysis: a sound upper bound on what any block
/// could possibly move.
fn compute_block_local_universe(
    block_local: &[FxHashMap<ArcVarId, FxHashSet<u32>>],
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut universe: FxHashMap<ArcVarId, FxHashSet<u32>> = FxHashMap::default();
    for per_block in block_local {
        union_field_map_into(&mut universe, per_block);
    }
    universe
}

/// INTERSECT field-sets across predecessors' exit states.
///
/// For each var-key present in ALL predecessor exit sets, take the
/// intersection of field sets. Var-keys present in only a strict subset
/// of predecessors are dropped (NOT definitely-moved at this entry).
/// Empty predecessor list (entry block) returns an empty map.
fn intersect_predecessor_exits(
    preds: &[usize],
    block_exits: &[FxHashMap<ArcVarId, FxHashSet<u32>>],
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut result: FxHashMap<ArcVarId, FxHashSet<u32>> = FxHashMap::default();
    let Some((&first, rest)) = preds.split_first() else {
        return result;
    };
    // Seed from first predecessor.
    for (&src, fields) in &block_exits[first] {
        result.insert(src, fields.clone());
    }
    // Intersect against each remaining predecessor.
    for &p in rest {
        let other = &block_exits[p];
        result.retain(|src, fields| {
            let Some(other_fields) = other.get(src) else {
                return false;
            };
            fields.retain(|f| other_fields.contains(f));
            !fields.is_empty()
        });
    }
    result
}

/// Pointwise union of `entry` and `local`. For each var present in
/// either, the result's field set is the union. Pure function — the
/// per-block transfer function `exit(B) = entry(B) ∪ block_local(B)`.
fn union_entry_with_local(
    entry: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    local: &FxHashMap<ArcVarId, FxHashSet<u32>>,
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut result = entry.clone();
    union_field_map_into(&mut result, local);
    result
}
