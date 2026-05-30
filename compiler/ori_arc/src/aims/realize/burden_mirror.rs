//! Coexistence-mirror reconciliation of the per-value burden ledger against
//! the realized predicate-stack RC ledger, for the cross-block gap classes the
//! per-instruction burden walk does not balance.
//!
//! Two passes (post-realize, after unwind cleanup, before the burden-balance
//! check):
//!
//! - **dec-mirror** (inc-surplus: a `BurdenInc` with no whole-var `BurdenDec`):
//!   the scope-exit release class — the value's last use is a borrow and its
//!   release lands at a later merge / exit / unwind block. Mirror each
//!   predicate-stack `RcDec` with a whole-var `BurdenDec`. The `RcDec` set is
//!   RL-2-faithful, so the burden ledger nets 0 along every path transitively.
//!
//! - **inc-injection** (per-path dec coverage): a forward dataflow per var
//!   keeps the running burden net `≥ 0` along every path — whenever a whole-var
//!   `BurdenDec` would drive the running net below 0 (an uncovered release, e.g.
//!   a spurious dup-alias dec whose FRESH-site inc the Step-4b walk could not
//!   place because the `RcInc` proving duplication does not exist until
//!   realization), inject a balancing `BurdenInc` immediately before it. The
//!   adjacent inc+dec pair nets 0 locally, so block exit nets — tracked live so
//!   successors see the post-injection value — stay correct without staleness.
//!
//! Whole-var dec counting matches `aims/verify/burden_delta.rs`: `BurdenDec` /
//! `BurdenDecPartial` / `BurdenDecVariant` count; `BurdenDecField` (field-grain)
//! is excluded. Eligibility consults the CURRENT (post-elimination) ledger, not
//! the Step-4b `burden_emitted` flag (Phase 2.5 elimination may have removed a
//! var's inc).

use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

/// Whole-var burden-dec variant target, or `None` for non-dec / field-grain
/// `BurdenDecField`.
fn whole_var_dec_target(instr: &ArcInstr) -> Option<usize> {
    match instr {
        ArcInstr::BurdenDec { var }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::BurdenDecVariant { var } => Some(var.index()),
        _ => None,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "ArcFunction var counts fit in u32"
)]
fn var_id(raw: usize) -> ArcVarId {
    ArcVarId::new(raw as u32)
}

/// Reconcile the per-value burden ledger against the realized RC ledger. No-op
/// when nothing is burden-tracked.
pub(crate) fn reconcile_burden_ledger(func: &mut ArcFunction) {
    let num_vars = func.var_types.len();
    if num_vars == 0 {
        return;
    }

    let (has_inc, mut has_dec) = compute_inc_dec_presence(func, num_vars);

    // Pass 1 — inc-surplus dec-mirror: mirror each RcDec with a BurdenDec for a
    // var that carries an inc but no whole-var dec (the scope-exit class).
    let inc_surplus: Vec<bool> = (0..num_vars).map(|i| has_inc[i] && !has_dec[i]).collect();
    let mirrored_decs = mirror_scope_exit_decs(func, num_vars, &inc_surplus);
    // Pass 1 added decs; refresh the dec-presence view for pass 2.
    for i in 0..num_vars {
        if inc_surplus[i] {
            has_dec[i] = true;
        }
    }

    // Pass 2 — per-path balancer: a var carrying a whole-var inc OR dec can be
    // per-path-imbalanced (an inc uncovered on a branch lacking the dec, or a
    // dec uncovered on a branch lacking the inc). The forward pass balances both.
    let preds = compute_predecessors(func);
    let entry_idx = func.entry.index();
    // Reverse-postorder so every forward predecessor of a merge is finalized
    // before the merge — index order is not topological once unwind blocks
    // interleave. The CFG is stable (only body burden ops change), so the order
    // is computed once. Back-edge predecessors stay after their header.
    let rpo: Vec<usize> = {
        let mut po = compute_postorder(func);
        po.reverse();
        po
    };
    let mut injected_incs = 0usize;
    let mut injected_decs = 0usize;
    for raw in 0..num_vars {
        if !(has_inc[raw] || has_dec[raw]) {
            continue;
        }
        let (incs, decs) = balance_var_per_path(func, raw, &rpo, &preds, entry_idx);
        injected_incs += incs;
        injected_decs += decs;
    }

    if tracing::enabled!(tracing::Level::DEBUG)
        && (mirrored_decs > 0 || injected_incs > 0 || injected_decs > 0)
    {
        tracing::debug!(
            target: "ori_arc::aims::realize::burden_mirror",
            function = ?func.name,
            mirrored_decs,
            injected_incs,
            injected_decs,
            "burden ledger reconciliation",
        );
    }
}

/// CURRENT (post-elimination) whole-var burden inc/dec presence per var.
fn compute_inc_dec_presence(func: &ArcFunction, num_vars: usize) -> (Vec<bool>, Vec<bool>) {
    let mut has_inc = vec![false; num_vars];
    let mut has_dec = vec![false; num_vars];
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::BurdenInc { var } = instr {
                if var.index() < num_vars {
                    has_inc[var.index()] = true;
                }
            } else if let Some(vi) = whole_var_dec_target(instr) {
                if vi < num_vars {
                    has_dec[vi] = true;
                }
            }
        }
    }
    (has_inc, has_dec)
}

/// Pass 1: mirror each `RcDec` with a whole-var `BurdenDec` for an inc-surplus
/// var (carries a `BurdenInc`, no whole-var `BurdenDec` — the scope-exit release
/// class). Returns the count of mirrored decs.
fn mirror_scope_exit_decs(func: &mut ArcFunction, num_vars: usize, inc_surplus: &[bool]) -> usize {
    if !inc_surplus.iter().any(|e| *e) {
        return 0;
    }
    let mut mirrored = 0usize;
    for block in &mut func.blocks {
        let mut new_body = Vec::with_capacity(block.body.len());
        for instr in std::mem::take(&mut block.body) {
            if let ArcInstr::RcDec { var, .. } = &instr {
                let vi = var.index();
                if vi < num_vars && inc_surplus[vi] {
                    new_body.push(ArcInstr::BurdenDec { var: *var });
                    mirrored += 1;
                }
            }
            new_body.push(instr);
        }
        block.body = new_body;
    }
    mirrored
}

/// Pass 2 inner: forward per-path balancer for one var `raw` over `rpo`. Keeps
/// the running net ≥ 0 (inject `BurdenInc` before an under-covered dec) and
/// nets 0 at terminals + merge-equalizes divergent predecessors (inject
/// `BurdenDec`). Returns `(injected_incs, injected_decs)`.
fn balance_var_per_path(
    func: &mut ArcFunction,
    raw: usize,
    rpo: &[usize],
    preds: &[Vec<usize>],
    entry_idx: usize,
) -> (usize, usize) {
    let var = var_id(raw);
    let n = func.blocks.len();
    let mut exit_net: Vec<Option<i64>> = vec![None; n];
    let mut injected_incs = 0usize;
    let mut injected_decs = 0usize;
    for &b in rpo {
        let entry = if b == entry_idx {
            0
        } else {
            let (target, decs) = merge_equalize_preds(func, var, b, preds, &mut exit_net);
            injected_decs += decs;
            target
        };
        let (running, body, incs) = walk_block_balanced(func, raw, var, b, entry);
        injected_incs += incs.0;
        injected_decs += incs.1;
        func.blocks[b].body = body;
        exit_net[b] = Some(running);
    }
    (injected_incs, injected_decs)
}

/// Merge-equalize at block `b`: reduce higher-net predecessors to the agreed
/// minimum by appending a balancing `BurdenDec` to each (before its
/// terminator). Returns `(entry_net_for_b, injected_decs)`. Back-edge / unvisited
/// predecessors (`exit_net == None`) contribute nothing.
fn merge_equalize_preds(
    func: &mut ArcFunction,
    var: ArcVarId,
    b: usize,
    preds: &[Vec<usize>],
    exit_net: &mut [Option<i64>],
) -> (i64, usize) {
    let defined: Vec<(usize, i64)> = preds[b]
        .iter()
        .filter_map(|&p| exit_net[p].map(|e| (p, e)))
        .collect();
    if defined.is_empty() {
        return (0, 0);
    }
    let target = defined.iter().map(|(_, e)| *e).min().unwrap_or(0);
    let mut decs = 0usize;
    for &(p, e) in &defined {
        for _ in 0..(e - target) {
            func.blocks[p].body.push(ArcInstr::BurdenDec { var });
            decs += 1;
        }
        if e > target {
            exit_net[p] = Some(target);
        }
    }
    (target, decs)
}

/// Walk block `b`'s body for `raw`, keeping `running` ≥ 0 (inject `BurdenInc`
/// before an under-covered dec) and netting 0 at a terminal block (inject
/// `BurdenDec`). Returns `(running_exit_net, new_body, (injected_incs, injected_decs))`.
fn walk_block_balanced(
    func: &mut ArcFunction,
    raw: usize,
    var: ArcVarId,
    b: usize,
    entry: i64,
) -> (i64, Vec<ArcInstr>, (usize, usize)) {
    let mut running = entry;
    let mut injected_incs = 0usize;
    let mut injected_decs = 0usize;
    let mut new_body = Vec::with_capacity(func.blocks[b].body.len());
    for instr in std::mem::take(&mut func.blocks[b].body) {
        if matches!(&instr, ArcInstr::BurdenInc { var: v } if v.index() == raw) {
            running += 1;
        }
        if whole_var_dec_target(&instr) == Some(raw) {
            if running <= 0 {
                new_body.push(ArcInstr::BurdenInc { var });
                running += 1;
                injected_incs += 1;
            }
            running -= 1;
        }
        new_body.push(instr);
    }
    let is_terminal = matches!(
        func.blocks[b].terminator,
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
    );
    if is_terminal {
        while running > 0 {
            new_body.push(ArcInstr::BurdenDec { var });
            running -= 1;
            injected_decs += 1;
        }
    }
    (running, new_body, (injected_incs, injected_decs))
}

#[cfg(test)]
mod tests;
