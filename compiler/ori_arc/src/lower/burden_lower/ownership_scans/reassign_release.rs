//! RL-2 mutable-`Ident` reassignment release scan.
//!
//! A mutable rebind `x = e` orphans the binding's prior value at the rebind.
//! When the prior value is a multi-used heap allocation (a fresh `Construct`
//! or call result read by >= 2 `Let { Var }` aliases — e.g. the index-assign
//! desugar `xs = xs.updated(key: i, value: xs[i] + c)`), the burden walk emits
//! the value's FRESH-site `BurdenInc` (the keep-alive across the multiple
//! reads) but SUPPRESSES its terminal scope-exit `BurdenDec`: the last
//! `Let { Var }` use is a dup-terminal-move whose alias is a borrow-read that
//! transfers nothing, so `compute_transfer_via_move_alias` marks the source
//! transferred even though no consumer discharges the binding's own reference.
//! The kept inc with the suppressed dec nets +1 — the binding's prior value
//! leaks (`ORI_DUMP_AFTER_ARC` shows the `Construct`'s `RcInc` with no matching
//! `RcDec`).
//!
//! This scan re-instates the release the suppression dropped: for each
//! `(old_var, new_var)` reassignment death recorded by `lower_assign`, place
//! EXACTLY ONE `BurdenDec(old_var)` immediately after `new_var`'s defining
//! `Let { dst: new_var, value: Var(rhs) }` (the rebind point — every RHS use of
//! `old_var` has completed there, so the release is UAF-safe). Per `Spec:
//! Annex E §AIMS RL-2` (the binding's prior value reaches its
//! `LastReadBeforeScopeExit` / `ScopeExit` non-transfer death at the rebind).
//!
//! Gate (the leak signature, all from already-computed facts; never a
//! double-free on a genuinely-transferred value):
//!   1. `old_var ∈ owned_vars_needing_rc` — a heap RC value.
//!   2. `old_var ∈ transfer_via_move_alias` — its terminal scope-exit dec was
//!      SUPPRESSED (the move-alias transfer claim that did not discharge).
//!   3. `old_var ∉ inc_suppressed_vars` — its FRESH-site inc was KEPT (a
//!      genuinely-moved value (use-once) has its inc suppressed symmetrically,
//!      so kept-inc + suppressed-dec is exactly the net +1 leak).
//!   4. `old_var ∉ full_move_vars` — its whole owned-field set was not moved
//!      (a full field-grain move owns its own releases).
//!
//! A value whose ownership genuinely transferred out of the binding (the
//! `x = [c, ...x]` Construct-spread move, the `x = if c then x else y` per-edge
//! transfer) carries NO kept FRESH-site inc for the binding's own reference
//! (it is use-once / branch-consumed), so gate (3) declines and no release is
//! emitted — the existing transfer / branch machinery already balances it.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, ValueRepr};

use super::super::PlacedReleaseMap;
use super::forwarder_release::ForwarderReleasePos;

/// `ORI_DISABLE_REASSIGN_REBIND_RELEASE=1` skips the Phase-5 RL-2 mutable-rebind
/// release ([`compute_reassign_rebind_releases`]). Default (unset): the gated
/// `BurdenDec(old_var)` is emitted at the rebind. Bisection surface: isolates a
/// self-referential-reassignment leak / double-free to this scan vs the rest of
/// the Phase-5 walk. Spec: Annex E §AIMS RL-2.
pub(in crate::lower::burden_lower) fn reassign_rebind_release_disabled() -> bool {
    std::env::var("ORI_DISABLE_REASSIGN_REBIND_RELEASE").as_deref() == Ok("1")
}

/// Build the per-(block, pos) placed-release map for mutable-rebind deaths.
///
/// Iterates `func.reassign_deaths` (`(old_var, new_var)` pairs from
/// `lower_assign`). For each pair passing the leak-signature gate, locate
/// `new_var`'s defining `Let { dst: new_var, value: Var(_) }` and place
/// `BurdenDec(old_var)` at `AfterInstr(new_var_let_idx)` in that block.
pub(in crate::lower::burden_lower) fn compute_reassign_rebind_releases(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    transfer_via_move_alias: &FxHashSet<ArcVarId>,
    inc_suppressed_vars: &FxHashSet<ArcVarId>,
    full_move_vars: &FxHashSet<ArcVarId>,
) -> PlacedReleaseMap {
    let mut releases: PlacedReleaseMap = PlacedReleaseMap::default();
    if reassign_rebind_release_disabled() {
        return releases;
    }

    for &(old_var, new_var) in &func.reassign_deaths {
        // Gate (0): whole-value heap RC only. The leak is the list/map/set/
        // str-buffer `.updated()` shape where the binding holds a single
        // `RcPointer` allocation. An `Aggregate` binding (struct / enum / tuple
        // — e.g. `r = { ...r, players: r.players.updated(..) }`) moves its
        // RC-bearing FIELD out via a `Project` consumed `[own]`, so its
        // field-grain release is owned by the partial/full-move machinery; a
        // whole-var rebind dec here double-frees the already-moved field. Scalar
        // / FatValue reprs carry no whole-value `RcPointer` lineage to leak.
        if old_var.index() >= func.var_reprs.len()
            || func.var_reprs[old_var.index()] != ValueRepr::RcPointer
        {
            continue;
        }
        // Gate: the leak signature — a kept FRESH-site inc (gates 1 + 3 + 4)
        // whose terminal scope-exit dec was suppressed (gate 2). A genuinely
        // transferred / branch-consumed binding value fails gate 3 (its inc was
        // suppressed symmetrically) and is left to the existing machinery.
        if !owned_vars_needing_rc.contains(&old_var)
            || !transfer_via_move_alias.contains(&old_var)
            || inc_suppressed_vars.contains(&old_var)
            || full_move_vars.contains(&old_var)
        {
            continue;
        }

        let Some((block_idx, let_idx)) = find_rebind_let(func, new_var) else {
            continue;
        };

        let entry = releases
            .entry((block_idx, ForwarderReleasePos::AfterInstr(let_idx)))
            .or_default();
        // Idempotent: never two releases of one binding value at one rebind.
        if !entry.contains(&old_var) {
            entry.push(old_var);
        }
    }

    releases
}

/// Locate `new_var`'s defining `Let { dst: new_var, value: Var(_) }` — the
/// rebind site `lower_assign` emitted (`new_var = ArcValue::Var(rhs)`). Returns
/// `(block_idx, instr_idx)`; `None` if `new_var` has no such def (defensive —
/// e.g. CFG rewrites that re-shaped the rebind).
fn find_rebind_let(func: &ArcFunction, new_var: ArcVarId) -> Option<(usize, usize)> {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(_),
                ..
            } = instr
            {
                if *dst == new_var {
                    return Some((block_idx, instr_idx));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
