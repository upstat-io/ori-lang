//! VF-1 burden-balance check (basic — function-exit net).
//!
//! For every `v ∈ func.burden_emitted`, verify
//! `Σ BurdenInc(v) - Σ BurdenDec*(v) == 0` along every reachable path from
//! `func.entry` to a terminal block (`Return` / `Resume` / `Unreachable`).
//!
//! `BurdenDecField` targets a SUB-FIELD of the base; it contributes to a
//! separate field-grain accumulator (not modeled here) and is excluded from
//! the whole-var balance, per `aims/verify/burden_delta.rs`.
//!
//! Algorithm — forward dataflow on per-block deltas:
//! 1. `entry_net[entry] = 0`.
//! 2. Worklist over predecessor exits: for each non-entry block `b`,
//!    `entry_net[b] = pred.entry_net + pred.delta` agreed across all
//!    defined predecessors.
//! 3. CFG-merge predecessor disagreement = imbalance (covers diamond
//!    predecessor-disagreement case per §04A.4 ITEM-6).
//! 4. Terminal block `t` (`Return`/`Resume`/`Unreachable`) MUST have
//!    `entry_net[t] + delta[t] == 0`.
//!
//! Full VF-1 (per-edge dataflow + SCC net-zero obligation per
//! `aims-rules.md §9 VF-1`) is deferred to §10 of the
//! `aims-burden-tracking` umbrella; this module ships the basic
//! function-exit form that gates the §04A wiring + §04A.2 elim +
//! §04A.3 coexistence cluster.

use crate::aims::verify::burden_delta::compute_var_block_deltas;
use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};
use crate::verify::BurdenBalanceError;

/// Verify per-variable burden balance for every `v ∈ func.burden_emitted`.
///
/// Returns an empty `Vec` when every reachable function-exit terminates
/// with net `Σ BurdenInc(v) - Σ BurdenDec*(v) == 0`. Each detected
/// violation produces one [`BurdenBalanceError`] entry.
///
/// CFG-merge predecessor disagreement produces an entry whose
/// `exit_block` is the merge block (with `expected_net = 0` and
/// `observed_net = disagreed_value`); function-exit-net violations
/// produce an entry whose `exit_block` is the terminal block.
pub(crate) fn verify_burden_balance(func: &ArcFunction) -> Vec<BurdenBalanceError> {
    let mut errors: Vec<BurdenBalanceError> = Vec::new();

    if func.burden_emitted.is_empty() {
        return errors;
    }

    let preds = compute_predecessors(func);
    let num_vars = func.burden_emitted.len();

    for raw in 0..num_vars {
        if !func.burden_emitted[raw] {
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ArcFunction var counts fit in u32"
        )]
        let var = ArcVarId::new(raw as u32);
        let delta = compute_var_block_deltas(func, var);
        let Some(entry_net) = compute_entry_nets(func, &preds, var, &delta, &mut errors) else {
            continue;
        };
        check_terminal_zero(func, var, &delta, &entry_net, &mut errors);
    }

    errors
}

/// Forward dataflow on per-block net burden count for `var`. Mirrors the
/// TRMC-scope counterpart `compute_burden_entry_nets` in
/// `aims/normalize/verify.rs`, but reports imbalance via the shared
/// [`BurdenBalanceError`] shape, scoped over the FULL function (not a
/// TRMC region).
fn compute_entry_nets(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    var: ArcVarId,
    delta: &[i64],
    errors: &mut Vec<BurdenBalanceError>,
) -> Option<Vec<Option<i64>>> {
    let n = func.blocks.len();
    let mut entry_net: Vec<Option<i64>> = vec![None; n];
    let entry_idx = func.entry.index();
    if entry_idx < n {
        entry_net[entry_idx] = Some(0);
    }

    let iter_cap = n.saturating_mul(4).max(16);
    let mut changed = true;
    let mut iterations: usize = 0;
    let mut imbalance_recorded = false;

    while changed && iterations < iter_cap {
        changed = false;
        iterations += 1;
        for b in 0..n {
            if b == entry_idx || preds[b].is_empty() {
                continue;
            }
            match merge_pred_exits(preds, b, delta, &entry_net) {
                MergeResult::Agreed(c) => {
                    if entry_net[b] != Some(c) {
                        entry_net[b] = Some(c);
                        changed = true;
                    }
                }
                MergeResult::Disagreed { observed } => {
                    errors.push(BurdenBalanceError {
                        var,
                        expected_net: 0,
                        observed_net: observed,
                        exit_block: func.blocks[b].id,
                    });
                    imbalance_recorded = true;
                    break;
                }
                MergeResult::Unreachable => {}
            }
        }
        if imbalance_recorded {
            break;
        }
    }

    if imbalance_recorded {
        None
    } else {
        Some(entry_net)
    }
}

enum MergeResult {
    Agreed(i64),
    Disagreed { observed: i64 },
    Unreachable,
}

/// Fold predecessor exit nets at merge point `b`. First defined predecessor
/// seeds; every subsequent pred MUST match. Returns the agreed exit value,
/// the divergent observed value, or `Unreachable` when no predecessor has a
/// defined entry net yet.
fn merge_pred_exits(
    preds: &[Vec<usize>],
    b: usize,
    delta: &[i64],
    entry_net: &[Option<i64>],
) -> MergeResult {
    let mut chosen: Option<i64> = None;
    for &p in &preds[b] {
        let Some(pe) = entry_net[p] else {
            continue;
        };
        let p_exit = pe + delta[p];
        match chosen {
            None => chosen = Some(p_exit),
            Some(c) if c == p_exit => {}
            Some(_) => return MergeResult::Disagreed { observed: p_exit },
        }
    }
    match chosen {
        Some(c) => MergeResult::Agreed(c),
        None => MergeResult::Unreachable,
    }
}

/// Every reachable terminal block (`Return` / `Resume` / `Unreachable`)
/// MUST have `entry_net + delta == 0`. Straight-line CFGs with uniform
/// non-zero net are caught here (no merge point fires the predecessor
/// check). The first violation per `var` is recorded.
fn check_terminal_zero(
    func: &ArcFunction,
    var: ArcVarId,
    delta: &[i64],
    entry_net: &[Option<i64>],
    errors: &mut Vec<BurdenBalanceError>,
) {
    for (b, block) in func.blocks.iter().enumerate() {
        let Some(eb) = entry_net[b] else {
            continue;
        };
        let is_terminal = matches!(
            block.terminator,
            ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
        );
        if !is_terminal {
            continue;
        }
        let observed = eb + delta[b];
        if observed != 0 {
            errors.push(BurdenBalanceError {
                var,
                expected_net: 0,
                observed_net: observed,
                exit_block: block.id,
            });
            break;
        }
    }
}

#[cfg(test)]
mod tests;
