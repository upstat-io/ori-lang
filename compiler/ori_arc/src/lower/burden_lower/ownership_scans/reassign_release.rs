//! RL-2 mutable-`Ident` reassignment release scan.
//!
//! Spec: Annex E §AIMS RL-2 (`LastReadBeforeScopeExit` / `ScopeExit`
//! non-transfer death at the rebind).
//!
//! # Leak signature
//! - A rebind `x = e` orphans the binding's prior value (`xs = xs.updated(key: i,
//!   value: xs[i] + c)` self-referential shape).
//! - The walk keeps the FRESH-site `BurdenInc` (multi-read keep-alive) but
//!   suppresses the terminal scope-exit `BurdenDec` — the dup-terminal-move alias
//!   is a borrow-read that transfers nothing, yet marks the source transferred.
//! - Kept inc + suppressed dec nets +1 (`ORI_DUMP_AFTER_ARC`: `RcInc` w/o `RcDec`).
//!
//! # Fix
//! - Per `(old_var, new_var)` death from `lower_assign`, place EXACTLY ONE
//!   `BurdenDec(old_var)` after `new_var`'s defining `Let { value: Var(rhs) }` —
//!   every RHS use of `old_var` has completed there, so the release is UAF-safe.
//!
//! # 5-condition gate (all already-computed facts; never double-frees a transfer)
//! - `var_reprs[old_var] == RcPointer` — whole-value heap RC (Gate-0; an
//!   `Aggregate` field-move-out owns its own field-grain release).
//! - `old_var ∈ owned_vars_needing_rc` — owned RC value.
//! - `old_var ∈ transfer_via_move_alias` — terminal dec was suppressed.
//! - `old_var ∉ inc_suppressed_vars` — FRESH-site inc was kept (a genuine
//!   use-once move suppresses the inc symmetrically; kept-inc + suppressed-dec
//!   is exactly the +1 leak).
//! - `old_var ∉ full_move_vars` — whole owned-field set not moved.
//!
//! # Non-firing
//! - A genuine transfer (`x = [c, ...x]` Construct-spread; `x = if c then x else
//!   y` per-edge) carries no kept FRESH-site inc, so the inc-suppressed gate
//!   declines — the existing transfer / branch machinery already balances it.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, ValueRepr};

use super::super::PlacedReleaseMap;
use super::forwarder_release::ForwarderReleasePos;

/// Whether `old_var` (or any member of its forward `Let { Var }` alias closure)
/// has a LIVE BORROW co-use — a use at a non-owned position that is NOT itself a
/// transparent `Let { Var }` alias edge of the closure.
///
/// Discriminates the two self-rebind shapes that both reach
/// [`compute_reassign_rebind_releases`]'s 5-condition gate:
///
/// - `x = x.updated(key: i, value: x[i] + c)` — `x` feeds the consuming call via
///   an `[own]` alias AND a co-occurring `[borrow]` index-read alias. The borrow
///   keeps `x`'s allocation alive past the `[own]` transfer, so the post-call
///   rebind orphans a retained reference the rebind `BurdenDec` releases (the
///   leak signature this scan corrects). `true`.
/// - `x = x.push(v)` — `x` feeds the consuming COW call (`@push(recv [own])`,
///   which reallocs/relocates the buffer and returns it AS the rebind result)
///   via a SINGLE `[own]` alias, with NO surviving borrow co-use. `x`'s sole
///   reference is genuinely transferred into the call; the post-call rebind
///   `BurdenDec` double-frees the buffer the call already consumed. `false`.
///
/// `Let { Var }` alias edges of the closure are EXCLUDED (they are the transparent
/// aliasing being traversed, not borrow reads); every other non-owned-position
/// use (a `[borrow]` call arg, `Project`, `IsShared`, `Set` base) counts.
/// Spec: Annex E §AIMS RL-2 (transfer vs surviving-borrow last use).
fn old_var_alias_closure_has_borrow_couse(
    func: &ArcFunction,
    old_var: ArcVarId,
    iter_consume_transfer_args: &FxHashSet<ArcVarId>,
) -> bool {
    // Forward `Let { Var }` alias closure of `old_var` (fixpoint over alias edges).
    let mut closure: FxHashSet<ArcVarId> = FxHashSet::default();
    closure.insert(old_var);
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if closure.contains(src) && closure.insert(*dst) {
                        changed = true;
                    }
                }
            }
        }
    }

    for block in &func.blocks {
        for instr in &block.body {
            // Skip the transparent alias edges of the closure itself.
            if let ArcInstr::Let {
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if closure.contains(src) {
                    continue;
                }
            }
            // RC bookkeeping ops (`BurdenInc`/`BurdenDec`/`RcInc`/`RcDec`) name
            // their var at a non-owned position but are NOT real reads — a
            // per-path keep-alive inc/dec on a loop-carried block param is not a
            // surviving borrow co-use. Skip them.
            if matches!(
                instr,
                ArcInstr::BurdenInc { .. }
                    | ArcInstr::BurdenDec { .. }
                    | ArcInstr::RcInc { .. }
                    | ArcInstr::RcDec { .. }
            ) {
                continue;
            }
            for (pos, var) in instr.used_vars().into_iter().enumerate() {
                if closure.contains(&var)
                    && !instr.is_owned_position(pos)
                    // An iter-consumed borrowed arg is an RL-2 ownership
                    // transfer (the callee's iterator machinery releases the
                    // retained reference), not a surviving borrow.
                    && !iter_consume_transfer_args.contains(&var)
                {
                    return true;
                }
            }
        }
        // A terminator borrow co-use is ONLY an `Invoke`/`InvokeIndirect` arg at
        // a non-owned position. `Jump`/`Return` args transfer ownership (RL-4
        // Jump-arg exemption / RL-2 return transfer) — `is_owned_position`
        // returns false for them, but they are transfers, not surviving borrows.
        if matches!(
            block.terminator,
            crate::ir::ArcTerminator::Invoke { .. }
                | crate::ir::ArcTerminator::InvokeIndirect { .. }
        ) {
            for (pos, var) in block.terminator.used_vars().into_iter().enumerate() {
                if closure.contains(&var)
                    && !block.terminator.is_owned_position(pos)
                    && !iter_consume_transfer_args.contains(&var)
                {
                    return true;
                }
            }
        }
    }
    false
}

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
    iter_consume_transfer_args: &FxHashSet<ArcVarId>,
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
        // Gate (6): the leak signature requires a SURVIVING BORROW co-use of
        // old_var that outlives the `[own]` transfer (the `x[i]` index-read of
        // `x = x.updated(key: i, value: x[i])`), leaving the orphaned reference
        // this dec releases. A PURE `[own]`-transfer self-rebind (`x = x.push(v)`:
        // old_var's sole reference is consumed by the COW call that reallocs the
        // buffer and returns it AS new_var) has NO surviving borrow — the buffer
        // IS new_var's allocation, so the rebind dec double-frees what the call
        // already consumed (the rc_matrix loop-carried-list UAF under
        // `ORI_DISABLE_PREDICATE_STACK_RC=1`). Spec: Annex E §AIMS RL-2.
        if !old_var_alias_closure_has_borrow_couse(func, old_var, iter_consume_transfer_args) {
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
