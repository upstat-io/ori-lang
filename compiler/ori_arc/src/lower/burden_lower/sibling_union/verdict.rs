//! Per-group sibling-union verdict application: moved-field union, skip-set
//! widening, single-releaser assignment, and the RL-1 keeper ledger.
//! Spec: Annex E §AIMS RL-1 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::TypeRegistry;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::groups::SiblingGroup;
use super::{owned_top_level_fields, root_is_multivariant_sum};

/// Verdict detail for one APPLIED group.
pub(super) struct GroupVerdict {
    /// The assigned keeper sibling, when the group carried a dup intermediate.
    pub(super) keeper: Option<ArcVarId>,
}

/// Apply the per-field verdict to one sibling group: compute the sibling-union
/// of moved fields, widen each sibling's skip set, and absorb fully-covered
/// siblings into `full_move_vars`.
pub(super) fn apply_group(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    group: &SiblingGroup,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) -> Option<GroupVerdict> {
    let decline = |gate: &str| {
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            root = group.root.index(),
            block = group.block_idx,
            gate,
            "sibling-moved-field verdict declined"
        );
    };

    // Sum-payload guard: decline when any sibling projects through a sum
    // (variant-tagged) source whose variant context is not statically the
    // same across siblings. v1-conservative: a sibling whose root type is a
    // sum aggregate with >1 variant declines the whole group (cross-variant
    // unification is unsound; pin 15). The same-variant sum extraction
    // (pin 10) reaches the rebuild through a Construct local root, not a
    // direct sum-field projection, so it is admitted.
    if root_is_multivariant_sum(func, group.root, type_registry) {
        decline("sum-payload-multivariant");
        return None;
    }

    // Sibling UNION of moved-out fields: a field counts as moved when ANY
    // sibling transferred it (the Pass-3 union already accumulates a field per
    // carrier through any RL-2 transfer position). Restrict to fields the
    // GROUP's siblings transferred so an unrelated alias of the same root does
    // not pollute the union.
    let mut union_fields: FxHashSet<u32> = FxHashSet::default();
    for &sib in &group.siblings {
        if let Some(fields) = moved_out_fields_union.get(&sib) {
            union_fields.extend(fields.iter().copied());
        }
    }
    if union_fields.is_empty() {
        decline("empty-union");
        return None;
    }

    // Keeper ledger (RL-1 — the multi-hop / late-use-alias chain): an
    // INTERMEDIATE alias with a kept dup inc and a transfer-suppressed own dec
    // (its last use is a sibling's Let) is balanced by EXACTLY ONE whole-var
    // release among its siblings — the KEEPER, the LAST sibling through it
    // (all reads precede the release). The keeper is left UNTOUCHED (keeps its
    // whole `burden_dec` at last use; its own dup inc is the standard DP-3
    // Once-Affine elision target). v1 conservative declines: >1 dup
    // intermediate (one whole dec cannot balance two kept incs); a keeper that
    // MOVED a field (its dec is partial — cannot balance the whole inc); an
    // intermediate used outside the group's sibling Lets (the kept inc may be
    // balanced elsewhere); union not covering all owned fields (an uncovered
    // field needs the entering reference released too — one whole dec cannot
    // do both). Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`).
    let owned_field_set_precheck = owned_top_level_fields(func, group.root, type_registry);
    let keeper: Option<ArcVarId> = match assign_keeper(
        func,
        group,
        moved_out_fields_union,
        &union_fields,
        &owned_field_set_precheck,
    ) {
        Ok(keeper) => keeper,
        Err(gate) => {
            decline(gate);
            return None;
        }
    };

    // Per-FIELD verdict: for each sibling, widen its skip set with the union,
    // then classify. A sibling whose widened skip covers ALL its owned
    // RC-carrying top-level fields becomes a full-move (dec + inc suppressed);
    // a mixed-coverage sibling keeps a `BurdenDecPartial` with the widened skip
    // (releases only its still-uncovered fields).
    //
    // Single-releaser assignment: an uncovered RC field must be released by
    // EXACTLY ONE sibling's kept dec (else widening leaves it in multiple
    // siblings' release sets — double-free; OR drops it from all — leak). v1
    // same-block: assign each uncovered field to the LAST sibling (instruction
    // order) that still owns it, widening every earlier sibling's skip to
    // include it.
    let owned_field_set = owned_field_set_precheck;

    // The releaser per uncovered field: the highest-index sibling that owns the
    // field (same-block instruction order). Compute owned-by per sibling first.
    let mut releaser: FxHashMap<u32, ArcVarId> = FxHashMap::default();
    for &field in &owned_field_set {
        if union_fields.contains(&field) {
            // Covered by some sibling's transfer — no sibling keeps a dec for
            // it (the transfer discharges it). NOT an uncovered field.
            continue;
        }
        // Uncovered: assign to the last sibling (group.siblings is sorted
        // ascending by index, so .last() is the latest).
        if let Some(&last) = group.siblings.last() {
            releaser.insert(field, last);
        }
    }

    for &sib in &group.siblings {
        // The KEEPER stays untouched: its whole `burden_dec` at last use is
        // the designated balancing release for the dup intermediate's kept
        // inc (RL-1). Never widened, never absorbed into full_move_vars.
        if Some(sib) == keeper {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = group.root.index(),
                sibling = sib.index(),
                "sibling-moved-field verdict: keeper (whole dec kept)"
            );
            continue;
        }
        // The sibling's widened skip: every owned field EXCEPT the uncovered
        // fields this sibling is the assigned releaser for.
        let mut skip: Vec<u32> = owned_field_set
            .iter()
            .copied()
            .filter(|f| releaser.get(f) != Some(&sib))
            .collect();
        skip.sort_unstable();

        // Does the widened skip cover ALL owned RC fields? (i.e. this sibling
        // releases nothing.)
        let covers_all = owned_field_set.iter().all(|f| skip.contains(f));

        if covers_all {
            // Full move: suppress the dec entirely + (via the
            // `inc_suppressed_vars = full_move_vars` coupling) the FRESH-site
            // dup-alias inc. Remove any stale partial entry.
            partial_move_vars.remove(&sib);
            full_move_vars.insert(sib);
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = group.root.index(),
                sibling = sib.index(),
                "sibling-moved-field verdict: widened (full-move)"
            );
        } else {
            // Mixed coverage: keep a partial dec with the widened skip. Only
            // emit when the sibling carries RC (in owned_vars_needing_rc); a
            // borrowed/scalar carrier never had a dec.
            if owned_vars_needing_rc.contains(&sib) {
                partial_move_vars.insert(sib, skip.clone());
                full_move_vars.remove(&sib);
            }
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = group.root.index(),
                sibling = sib.index(),
                ?skip,
                "sibling-moved-field verdict: widened (partial, kept uncovered)"
            );
        }
    }
    Some(GroupVerdict { keeper })
}

/// Keeper-ledger assignment for one group (RL-1): pick the ONE sibling whose
/// whole `burden_dec` balances the dup INTERMEDIATE's kept inc, or name the
/// declining gate. `Ok(None)` = no dup intermediate (no keeper needed).
/// v1 conservative declines: >1 dup intermediate (one whole dec cannot balance
/// two kept incs); a keeper that MOVED a field (its dec is partial — cannot
/// balance the whole inc); an intermediate used outside the group's sibling
/// `Let` aliases (the kept inc may be balanced elsewhere); union not covering all owned
/// fields (an uncovered field needs the entering reference released too — one
/// whole dec cannot do both). Spec: Annex E §AIMS RL-1
/// (`RL1_duplication_balanced`).
fn assign_keeper(
    func: &ArcFunction,
    group: &SiblingGroup,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    union_fields: &FxHashSet<u32>,
    owned_field_set: &FxHashSet<u32>,
) -> Result<Option<ArcVarId>, &'static str> {
    let (&dup_int, through_sibs) = match group.dup_intermediates.len() {
        0 => return Ok(None),
        1 => match group.dup_intermediates.iter().next() {
            Some(kv) => kv,
            None => return Err("dup-intermediate-missing"),
        },
        _ => return Err("multi-dup-intermediate"),
    };
    let Some(&last) = through_sibs.last() else {
        return Err("dup-intermediate-no-sibling");
    };
    if moved_out_fields_union
        .get(&last)
        .is_some_and(|f| !f.is_empty())
    {
        return Err("dup-intermediate-keeper-moves");
    }
    if !owned_field_set.iter().all(|f| union_fields.contains(f)) {
        return Err("dup-intermediate-uncovered-field");
    }
    // Every use of the intermediate must be one of the group's sibling Lets —
    // else the kept inc may already be balanced by a release outside the
    // group (keeping the keeper's dec would double-release).
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().contains(&dup_int) {
                continue;
            }
            let is_group_let = matches!(
                instr,
                ArcInstr::Let { dst, value: ArcValue::Var(src), .. }
                    if *src == dup_int && group.siblings.contains(dst)
            );
            if !is_group_let {
                return Err("dup-intermediate-escapes-group");
            }
        }
        if block.terminator.used_vars().contains(&dup_int) {
            return Err("dup-intermediate-escapes-group");
        }
    }
    tracing::trace!(
        target: "ori_arc::aims::realize",
        fn_name = ?func.name,
        root = group.root.index(),
        dup_intermediate = dup_int.index(),
        keeper = last.index(),
        "sibling-moved-field keeper assigned (balances intermediate dup inc)"
    );
    Ok(Some(last))
}
