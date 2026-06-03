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
//! 3. CFG-merge predecessor disagreement = imbalance (covers the diamond
//!    predecessor-disagreement case).
//! 4. Terminal block `t` (`Return`/`Resume`/`Unreachable`) MUST have
//!    `entry_net[t] + delta[t] == 0`.
//!
//! Full VF-1 (per-edge dataflow + SCC net-zero obligation) is deferred to
//! later verification work; this module ships the basic function-exit form
//! that gates burden-op emission, elimination, and the coexistence handshake.
//!
//! Set `ORI_LOG=ori_arc::aims::verify::burden_balance=debug` to emit a
//! per-block burden-delta breakdown for each imbalanced var (locates the
//! surplus/deficit block for missing CFG-edge or dead-at-entry decs).

use crate::aims::verify::burden_delta::{compute_burden_entry_nets, compute_var_block_deltas};
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
        let errs_before = errors.len();
        let nets = compute_burden_entry_nets(func, &preds, &delta);
        // Merge disagreement is reported FIRST and suppresses the terminal-net
        // check for this var (the function-exit net is undefined when a merge
        // point already diverges) — matching the prior stop-at-first semantics.
        if let Some(&(b, observed)) = nets.disagree_blocks.first() {
            errors.push(BurdenBalanceError {
                var,
                expected_net: 0,
                observed_net: observed,
                exit_block: func.blocks[b].id,
            });
        } else {
            check_terminal_zero(func, var, &delta, &nets.entry_net, &mut errors);
        }
        if errors.len() > errs_before {
            trace_burden_imbalance(func, var, &delta);
        }
    }

    errors
}

/// Emit a per-block burden-delta breakdown for a `var` whose balance check
/// failed. Surfaces WHICH block carries the surplus/deficit so the missing
/// CFG-edge dec (RL-4) or dead-at-entry dec (RL-5) is locatable without manual
/// IR-dump correlation. Gated behind `ORI_LOG=ori_arc::aims::verify::burden_balance=debug`; zero-overhead when disabled.
fn trace_burden_imbalance(func: &ArcFunction, var: ArcVarId, delta: &[i64]) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let per_block: Vec<String> = delta
        .iter()
        .enumerate()
        .filter(|(_, &d)| d != 0)
        .map(|(b, &d)| format!("bb{b}:{d:+}"))
        .collect();
    // Locate every surviving burden op targeting `var`, with its `(block,
    // instr)` site and instruction kind. Pairs with the per-block delta to name
    // the exact orphan inc/dec instruction the RL-2/RL-4/RL-5 ledger failed to
    // balance — removes the dump-correlation step for VF-1 residual fixes.
    let mut sites: Vec<String> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            let kind = match instr {
                crate::ir::ArcInstr::BurdenInc { var: v } if *v == var => "burden_inc",
                crate::ir::ArcInstr::BurdenDec { var: v } if *v == var => "burden_dec",
                crate::ir::ArcInstr::BurdenDecPartial { var: v, .. } if *v == var => {
                    "burden_dec_partial"
                }
                crate::ir::ArcInstr::BurdenDecVariant { var: v } if *v == var => {
                    "burden_dec_variant"
                }
                _ => continue,
            };
            sites.push(format!("bb{b}.{i}:{kind}"));
        }
    }
    tracing::debug!(
        function = ?func.name,
        var = var.index(),
        per_block_delta = %per_block.join(" "),
        burden_sites = %sites.join(" "),
        "burden imbalance per-block delta + burden-op sites (nonzero blocks)",
    );
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
