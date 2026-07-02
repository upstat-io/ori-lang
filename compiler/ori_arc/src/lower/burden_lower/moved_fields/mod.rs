//! Moved-out-fields forward-dataflow cluster for the Phase 5 burden walk.
//!
//! Computes, per owned aggregate var, the set of top-level field indices
//! transferred out via field-projection moves along every reachable CFG path.
//! Feeds the full-move / partial-move partition that gates `BurdenDec`
//! suppression and `BurdenDecPartial.skip_fields` per `Spec: Annex E §AIMS
//! RL-2`. Pure structural bookkeeping over `BurdenLowerCtx`'s per-block maps;
//! no lattice consultation.
//!
//! `dataflow` owns Pass 3 (the INTERSECT-at-entry worklist fixpoint) as its
//! own logical phase; this module owns Pass 1/2 collection + the
//! post-fixpoint union step + the full/partial-move post-processing.

mod dataflow;

use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use super::{instr_transfer_vars, BurdenLowerCtx};
use dataflow::{propagate_moved_out_fields, union_field_map_into};

/// Populate `ctx.moved_out_fields_{block_local,block_entry,block_exit}` per the
/// Non-Drop partial-move forward-flow rule. Three-pass walk over the CFG;
/// BOUNDED structural bookkeeping (finite field set per var, monotone field-set
/// growth → bounded fixpoint).
///
/// # Pass 1
///
/// Walk every block's body; record every `ArcInstr::Project
/// { dst, value, field, .. }` as a `dst → (value, field)` entry in a local
/// map.
///
/// # Pass 2
///
/// Walk every block's body + terminator; for each transferred var (per
/// `instr_transfer_vars` which honors `is_owned_position` + the Set-value
/// carve-out per `Spec: Annex E §AIMS TF-15` + IA-5 step (1), and per the
/// precomputed `terminator_transfer_per_block` set), if the transferred var
/// matches a `project_dst`, insert `(project_src, field)` into
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
/// # Pass 3 (X.2 merge)
///
/// Delegates to [`dataflow::propagate_moved_out_fields`] — the
/// INTERSECT-at-entry worklist fixpoint (entry/exit transfer functions,
/// bounded convergence, AIMS rule IA-MF1) is documented there; not
/// restated here to avoid a second copy drifting out of sync.
///
/// # Union rebuild
///
/// `moved_out_fields_union` rebuilt as the pointwise union over every
/// REACHABLE block's `block_exit[B]` — an unreachable block's exit stays
/// stuck at the Pass-3 ⊤ seed (see `dataflow::propagate_moved_out_fields`'s
/// return value) and is excluded. Preserves the `moved_out_fields()`
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
    let reachable_blocks = propagate_moved_out_fields(ctx, func, None);

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
    //
    // Why: an unreachable block's exit entry stays stuck at the Pass-3 ⊤
    // seed (never converged), so folding it in unfiltered would attribute
    // fields as moved purely because dead code projects them.
    ctx.moved_out_fields_union.clear();
    for (block_idx, per_block) in ctx.moved_out_fields_block_exit.iter().enumerate() {
        if !reachable_blocks.contains(&block_idx) {
            continue;
        }
        union_field_map_into(&mut ctx.moved_out_fields_union, per_block);
    }
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

#[cfg(test)]
mod tests;
