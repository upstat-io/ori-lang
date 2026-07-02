//! Moved-out-fields forward-dataflow cluster for the Phase 5 burden walk.
//!
//! Computes, per owned aggregate var, the set of top-level field indices
//! transferred out via field-projection moves along every reachable CFG path.
//! Feeds the full-move / partial-move partition that gates `BurdenDec`
//! suppression and `BurdenDecPartial.skip_fields` per `Spec: Annex E §AIMS
//! RL-2`. Pure structural bookkeeping over `BurdenLowerCtx`'s per-block maps;
//! no lattice consultation.

use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

#[cfg(test)]
use super::ctx::MovedFieldsConvergence;
use super::{instr_transfer_vars, BurdenLowerCtx};

/// Populate `ctx.moved_out_fields_{block_local,block_entry,block_exit}` per the
/// Non-Drop partial-move forward-flow rule. Three-pass walk over the CFG;
/// BOUNDED structural bookkeeping (finite field set per var, monotone field-set
/// growth → bounded fixpoint).
///
/// **Pass 1**: walk every block's body; record every `ArcInstr::Project
/// { dst, value, field, .. }` as a `dst → (value, field)` entry in a local
/// map.
///
/// **Pass 2**: walk every block's body + terminator; for each transferred
/// var (per `instr_transfer_vars` which honors `is_owned_position` + the
/// Set-value carve-out per `Spec: Annex E §AIMS TF-15` + IA-5 step (1), and
/// per the precomputed `terminator_transfer_per_block` set), if the
/// transferred var matches a `project_dst`, insert `(project_src, field)` into
/// `block_local[block_idx]`. This is the per-block transfer function output
/// ("what gets moved DURING this block").
///
/// Project ALONE does NOT set the bit (per `Spec: Annex E §AIMS TF-4` —
/// Project produces `Borrowed`; `is_owned_position`'s `_ => false`
/// excludes it). Project consumed at a borrowed position (e.g.,
/// `IsShared`) also leaves the bit unset — `IsShared` falls through
/// `_ => false` in `is_owned_position` and has no Set-value-style
/// carve-out.
///
/// **Pass 3 (X.2 merge)**: forward dataflow over the CFG. For each
/// block `B` in reverse-postorder:
///   - `entry(B) := INTERSECT over P in predecessors(B): exit(P)` (or
///     empty map for entry block); only fields moved on ALL incoming
///     paths are "definitely moved" at entry.
///   - `exit(B) := entry(B) ∪ block_local(B)` (pointwise union: for
///     each `(var, fields)` pair, merge field sets via set union).
///
/// Bounded fixpoint via worklist iteration to handle CFG back edges
/// (loops) — the analysis is monotone-DESCENDING from the ⊤ (universe)
/// seed, so the termination measure `Σ_b |exit[b]|` (bounded by
/// `n_blocks * universe_pair_count`) strictly shrinks each non-fixpoint
/// round, guaranteeing termination. DERIVED iteration cap
/// `n_blocks * universe_pair_count + 2` per AIMS rule IA-MF1 (proven by
/// `AimsProof.MovedFields::MF1_no_change_at_derived_cap`); non-convergence
/// within the cap fires a RELEASE-ACTIVE fail-closed guard (Spec: Annex E
/// §AIMS).
///
/// When E2043 typeck rejection guarantees equal predecessor exit sets the
/// INTERSECT degenerates to pick-any; INTERSECT remains the correct merge —
/// correct across both rejection states and structurally simpler than
/// special-casing per typeck status.
///
/// **Union rebuild**: `moved_out_fields_union` rebuilt as the pointwise
/// union over every `block_exit[B]`. Preserves the `moved_out_fields()`
/// accessor contract; consumed by `compute_full_move_vars` /
/// `compute_partial_move_vars` per `Spec: Annex E §AIMS RL-2`
/// partial-transfer semantics.
pub(super) fn populate_moved_out_fields(
    ctx: &mut BurdenLowerCtx<'_>,
    func: &ArcFunction,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    type_registry: &TypeRegistry,
) {
    // Pass 1: collect (project_dst → (project_src, field)) tuples.
    //
    // A `Project` of a SCALAR field transfers NO RC ownership (`Spec: Annex E
    // §AIMS L-9` scalar exclusion + TF-4 Project-is-Borrowed): moving an int /
    // bool / float payload out of an aggregate does not release any heap
    // allocation, so it must NOT contribute a moved-out field. Recording a
    // scalar projection would wrongly populate `skip_fields`, suppressing the
    // surviving RC'd payload's `BurdenDec` and leaking it (the Result<int, str>
    // `?? ` shape: the scalar Ok-int projection produces a `Project %r.1`-shaped
    // moved-field that would otherwise skip variant 1's surviving heap str).
    // Gate the record on the projected dst carrying an RC burden — scalars
    // return `None` from `lookup_burden` and are skipped.
    let mut project_origins: FxHashMap<ArcVarId, (ArcVarId, u32)> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst, value, field, ..
            } = instr
            {
                let dst_ty: TypeRef = idx_to_type_ref(func.var_types[dst.index()], type_registry);
                // A scalar / non-RC dst carries no allocation to move. `int`
                // resolves a builtin burden entry that is EMPTY (no owned
                // fields, no self-heap-alloc) — `is_some()` is NOT a sufficient
                // gate; require `burden_carries_rc` (the same RC-bearing
                // predicate `owned_vars_needing_rc` uses).
                let carries_rc = lookup_burden(dst_ty, type_registry)
                    .as_ref()
                    .is_some_and(super::burden_carries_rc);
                if !carries_rc {
                    continue;
                }
                project_origins.insert(*dst, (*value, *field));
            }
        }
    }

    // Pass 2: walk instructions + terminators; check transfer-vars against
    // project_origins. instr_transfer_vars honors is_owned_position +
    // Set-value carve-out; terminator_transfer_per_block carries
    // Return / Jump-to-Owned-param / Invoke-Owned / InvokeIndirect-Owned
    // per `Spec: Annex E §AIMS RL-2`. Insertions land in
    // `block_local[block_idx]` — the per-block transfer function output
    // consumed by Pass 3's merge.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            let transfer_vars = instr_transfer_vars(instr, func);
            for var in &transfer_vars {
                if let Some(&(src, field)) = project_origins.get(var) {
                    ctx.moved_out_fields_block_local[block_idx]
                        .entry(src)
                        .or_default()
                        .insert(field);
                }
            }
        }
        if let Some(term_transfers) = terminator_transfer_per_block.get(block_idx) {
            for var in term_transfers {
                if let Some(&(src, field)) = project_origins.get(var) {
                    ctx.moved_out_fields_block_local[block_idx]
                        .entry(src)
                        .or_default()
                        .insert(field);
                }
            }
        }
    }

    // Pass 3: forward dataflow with INTERSECT-at-entry merge.
    propagate_moved_out_fields(ctx, func, None);

    // Union step: rebuild the flat view from block_exit storage. Each
    // var's union is the set of fields moved on ANY reachable CFG path
    // from definition to last use — matches the partial-transfer
    // semantics consumed by `compute_full_move_vars` /
    // `compute_partial_move_vars`. Cleared first to keep
    // `populate_moved_out_fields` idempotent on repeat invocation.
    // `moved_out_fields_union[carrier]` carries the per-carrier moved-field
    // attribution derived from `project_origins` (Pass 1) through the Pass-2
    // transfer + Pass-3 dataflow: a field counts under the carrier (the
    // `project_src` alias) that lowered its projection. The sibling-alias
    // cross-check post-process consumes THIS union (the same projection-origin
    // attribution, already folded per carrier) — no re-derivation. Spec: Annex E
    // §AIMS RL-2.
    ctx.moved_out_fields_union.clear();
    for per_block in &ctx.moved_out_fields_block_exit {
        for (&src, fields) in per_block {
            let union_entry = ctx.moved_out_fields_union.entry(src).or_default();
            for &field in fields {
                union_entry.insert(field);
            }
        }
    }
}

/// Pass 3 — forward CFG dataflow propagating moved-field sets via
/// INTERSECT-at-entry merge.
///
/// Computes `entry(B) := INTERSECT over P in predecessors(B): exit(P)`
/// (empty for entry block) and `exit(B) := entry(B) ∪ block_local(B)`
/// for every block in reverse-postorder. Bounded fixpoint via worklist
/// iteration handles back edges; monotonicity of `∪` over a finite
/// field-index set guarantees termination.
///
/// INTERSECT semantics: for each var-key in the intersect result, the
/// field set is the intersection of fields moved on EVERY predecessor
/// path. Vars present in only one predecessor's exit set are DROPPED
/// from the intersect (NOT definitely-moved on this path). Per `Spec:
/// Annex E §AIMS RL-2`, this is the architecturally-correct merge —
/// emitting `BurdenDecPartial` with a field skipped only because ONE
/// of N predecessors moved it would be a use-after-free if the run-time
/// execution took a different predecessor.
/// `cap_override` forces the convergence cap in place of the derived
/// `derived_convergence_cap(n, universe_pair_count)`. Production always passes
/// `None` (the derived cap). Convergence-guard tests pass `Some(forced)` to
/// drive `changed == true` at the cap on a genuinely multi-round CFG so the
/// release-active guard fires on the real path (DESIGN-08 seam; the guard is
/// otherwise theorem-unreachable per IA-MF1).
fn propagate_moved_out_fields(
    ctx: &mut BurdenLowerCtx<'_>,
    func: &ArcFunction,
    cap_override: Option<usize>,
) {
    let n = func.blocks.len();
    if n == 0 {
        return;
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
    // single pass for DAG IR. Back edges require the outer worklist loop
    // below.
    let mut rpo = compute_postorder(func);
    rpo.reverse();

    // DERIVED convergence cap per AIMS rule IA-MF1 (Spec: Annex E §AIMS
    // fail-closed convergence). The termination measure `Σ_b |exit[b]|` is
    // bounded by `n_blocks * universe_pair_count` (n exit slots, each a subset
    // of the universe of size `universe_pair_count`) and strictly shrinks on
    // every non-fixpoint round (INTERSECT + ∪ are monotone from the ⊤ seed, so
    // every `changed` round removes ≥1 pair from some exit set), so the fixpoint
    // is reached within `n_blocks * universe_pair_count + 1` rounds; the cap
    // adds one round of margin. Proven bounded by
    // `AimsProof.MovedFields::MF1_no_change_at_derived_cap`, which makes the
    // release-active guard below unreachable on valid input. NOT a heuristic.
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
    // on the real path in a release build. Spec: Annex E §AIMS.
    assert!(
        !changed,
        "AIMS moved-out-fields INTERSECT fixpoint (rule IA-MF1) failed to converge in \
         {iteration_cap} rounds (n_blocks={n}, universe_pairs={universe_pair_count}); the \
         fixpoint is proven to converge within the derived cap \
         (AimsProof.MovedFields::MF1_no_change_at_derived_cap), so this is a compiler bug \
         or a malformed ArcFunction, not user error — please report it. \
         Spec: Annex E §AIMS (fail-closed convergence).",
    );
}

/// Derived convergence cap for the moved-out-fields INTERSECT fixpoint per AIMS
/// rule IA-MF1: `n_blocks * universe_pair_count + 2`. `universe_pair_count` =
/// distinct `(project_src, field)` pairs in the block-local universe (the
/// exit-lattice per-slot height). The termination measure `Σ_b |exit[b]|` is
/// bounded by `n_blocks * universe_pair_count` and strictly shrinks each
/// non-fixpoint round, so the fixpoint is reached within that bound + 1; the
/// trailing +1 is round-margin. Proven by
/// `AimsProof.MovedFields::MF1_no_change_at_derived_cap`. Saturating to avoid
/// overflow on pathological inputs.
fn derived_convergence_cap(n_blocks: usize, universe_pair_count: usize) -> usize {
    n_blocks
        .saturating_mul(universe_pair_count)
        .saturating_add(2)
}

#[cfg(test)]
mod tests;

/// Compute the universe of `(project_src, field)` pairs that appear in
/// ANY block-local moved-field map. This is the lattice top ⊤ for the
/// MUST-move INTERSECT analysis: a sound upper bound on what any block
/// could possibly move.
fn compute_block_local_universe(
    block_local: &[FxHashMap<ArcVarId, FxHashSet<u32>>],
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut universe: FxHashMap<ArcVarId, FxHashSet<u32>> = FxHashMap::default();
    for per_block in block_local {
        for (&src, fields) in per_block {
            let dest = universe.entry(src).or_default();
            for &f in fields {
                dest.insert(f);
            }
        }
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
    for (&src, fields) in local {
        let dest = result.entry(src).or_default();
        for &f in fields {
            dest.insert(f);
        }
    }
    result
}

/// Derive the full-move var set. For each `var` in `owned_vars_needing_rc`,
/// the full-move criterion holds when every `Burden::owned_fields()` entry's
/// `field_path[0]` (top-level field index) is contained in
/// `moved_out_fields[var]`. Vacuously true for vars with empty
/// `owned_fields()` (treated as not-full-move because such a var would not be
/// in `owned_vars_needing_rc` per the `burden_carries_rc` filter — the vacuous
/// case is unreachable in practice).
///
/// Returns a set of vars whose `BurdenDec` emission is SUPPRESSED at last-use
/// sites + terminator-positions per AIMS RL-2 (`BurdenDec` SHALL be emitted at
/// last use of an owned value UNLESS the last use is ownership-transferring;
/// full-move == complete field-projection transfer).
///
/// Partial-move (some-but-not-all fields covered by `moved_out_fields`) is NOT
/// in the full-move set — those vars still emit a conservative FULL `BurdenDec`
/// at last-use; field-aware partial-drop emission is handled by the
/// `BurdenDecPartial` IR variant.
pub(super) fn compute_full_move_vars(
    func: &ArcFunction,
    moved_out_fields: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut full_move_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for &var in owned_vars_needing_rc {
        let Some(moved_fields) = moved_out_fields.get(&var) else {
            continue;
        };
        let var_type = func.var_types[var.index()];
        let ty: TypeRef = idx_to_type_ref(var_type, type_registry);
        let Some(burden) = lookup_burden(ty, type_registry) else {
            continue;
        };
        // Empty owned_fields → vacuous all() returns true; guard against
        // false-positive by requiring at least one owned field. Vars in
        // owned_vars_needing_rc pass burden_carries_rc which excludes EMPTY
        // burdens, so this guard is defensive (catches future edge cases).
        let mut has_owned_field = false;
        let all_top_level_moved = burden.owned_fields().all(|of| {
            has_owned_field = true;
            of.field_path
                .first()
                .is_some_and(|f| moved_fields.contains(f))
        });
        if has_owned_field && all_top_level_moved {
            full_move_vars.insert(var);
        }
    }
    full_move_vars
}

/// Derive the partial-move var map. For each `var` in `owned_vars_needing_rc`
/// whose `moved_out_fields[var]` is non-empty AND `var` is NOT in
/// `full_move_vars`, collect a sorted `Vec<u32>` of the moved-out top-level
/// field indices. This is the `skip_fields` payload for the
/// `BurdenDecPartial { var, skip_fields }` IR variant.
///
/// Sorted-Vec encoding makes pass output deterministic: `moved_out_fields[var]`
/// is a `FxHashSet<u32>` whose iteration order is non-deterministic, so sorting
/// at emission time yields byte-identical IR across runs.
///
/// Returns a map from `ArcVarId` to its sorted `skip_fields`. Vars in
/// `full_move_vars` are excluded (suppression branch handles them); vars
/// with empty `moved_out_fields` are excluded (no skip required → emit full
/// `BurdenDec`). The result feeds the three-way branch in
/// `emit_instr_burdens` and `emit_terminator_burden_decs` at last-use sites.
pub(super) fn compute_partial_move_vars(
    moved_out_fields: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    full_move_vars: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, Vec<u32>> {
    let mut partial: FxHashMap<ArcVarId, Vec<u32>> = FxHashMap::default();
    for (&var, fields) in moved_out_fields {
        if fields.is_empty() {
            continue;
        }
        if !owned_vars_needing_rc.contains(&var) {
            continue;
        }
        if full_move_vars.contains(&var) {
            continue;
        }
        let mut sorted: Vec<u32> = fields.iter().copied().collect();
        sorted.sort_unstable();
        partial.insert(var, sorted);
    }
    partial
}
