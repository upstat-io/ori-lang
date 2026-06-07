//! Per-block burden-op net delta — shared by TRMC-scope
//! `verify_trmc_burden_balance` (in `aims/normalize/verify.rs`) and
//! full-function-scope `verify_burden_balance` (in
//! `aims/verify/burden_balance.rs`).
//!
//! The same `BurdenInc(v) - BurdenDec*(v)` forward-dataflow walk over a
//! block's body would otherwise exist in multiple sites with identical
//! control-flow shape; this helper is the single canonical home consumed by
//! every burden-balance verifier (full-function + TRMC-scope).

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

/// Whole-var burden-dec variant target, or `None` for a non-dec instruction
/// or the field-grain `BurdenDecField`.
///
/// SSOT for the whole-var burden-dec variant set `{BurdenDec, BurdenDecPartial,
/// BurdenDecVariant}`. `BurdenDecField` (field-grain) is excluded — it targets
/// a field of the base, not the whole-var balance. Every site reasoning about
/// whole-var burden decs consumes this predicate (`compute_var_block_deltas`).
pub(crate) fn whole_var_dec_target(instr: &ArcInstr) -> Option<ArcVarId> {
    match instr {
        ArcInstr::BurdenDec { var }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::BurdenDecVariant { var } => Some(*var),
        _ => None,
    }
}

/// Forward burden-balance dataflow result for one variable.
///
/// `entry_net[b]` is the agreed net `Σ BurdenInc − Σ BurdenDec*` along every
/// path from `func.entry` to block `b`'s entry (`None` = unreachable).
/// `disagree_blocks` are merge points where predecessors exit with divergent
/// nets — a per-path imbalance the consumer resolves (the verifier reports it;
/// the reconciler skips it).
pub(crate) struct BurdenEntryNets {
    pub entry_net: Vec<Option<i64>>,
    /// Merge points where predecessors exit with divergent nets, paired with
    /// one observed divergent exit value (for imbalance reporting).
    pub disagree_blocks: Vec<(usize, i64)>,
}

/// Worklist forward dataflow over per-block burden `delta` (from
/// [`compute_var_block_deltas`]). At a merge, the first defined predecessor
/// seeds; a divergent predecessor records `b` in `disagree_blocks` (the first
/// agreed value is retained so downstream blocks still receive a defined net).
/// Shared SSOT for `verify_burden_balance` (full-function) and the
/// post-realize burden-ledger reconciliation.
pub(crate) fn compute_burden_entry_nets(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    delta: &[i64],
) -> BurdenEntryNets {
    let n = func.blocks.len();
    let mut entry_net: Vec<Option<i64>> = vec![None; n];
    let mut disagree_blocks: Vec<(usize, i64)> = Vec::new();
    let entry_idx = func.entry.index();
    if entry_idx < n {
        entry_net[entry_idx] = Some(0);
    }

    let iter_cap = n.saturating_mul(4).max(16);
    let mut changed = true;
    let mut iterations: usize = 0;
    while changed && iterations < iter_cap {
        changed = false;
        iterations += 1;
        for b in 0..n {
            if b == entry_idx || preds[b].is_empty() {
                continue;
            }
            let mut chosen: Option<i64> = None;
            for &p in &preds[b] {
                let Some(pe) = entry_net[p] else {
                    continue;
                };
                let p_exit = pe + delta[p];
                match chosen {
                    None => chosen = Some(p_exit),
                    Some(c) if c == p_exit => {}
                    Some(_) => {
                        if !disagree_blocks.iter().any(|(db, _)| *db == b) {
                            disagree_blocks.push((b, p_exit));
                        }
                    }
                }
            }
            if let Some(c) = chosen {
                if entry_net[b] != Some(c) {
                    entry_net[b] = Some(c);
                    changed = true;
                }
            }
        }
    }

    BurdenEntryNets {
        entry_net,
        disagree_blocks,
    }
}

/// Per-block burden delta for `var`: `Σ BurdenInc(var) - Σ BurdenDec*(var)`.
///
/// `BurdenDecPartial` and `BurdenDecVariant` mirror whole-var burden
/// tracking (like `BurdenDec`) and contribute `-1`. `BurdenDecField`
/// targets a field of the base — not the whole-var balance — so it is
/// excluded.
///
/// Returned vector is indexed by block index (`func.blocks[i]`), length
/// `func.blocks.len()`.
pub(crate) fn compute_var_block_deltas(func: &ArcFunction, var: ArcVarId) -> Vec<i64> {
    let mut delta: Vec<i64> = vec![0; func.blocks.len()];
    for (idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if matches!(instr, ArcInstr::BurdenInc { var: v } if *v == var) {
                delta[idx] += 1;
            } else if whole_var_dec_target(instr) == Some(var) {
                delta[idx] -= 1;
            }
        }
    }
    delta
}

/// The kind of per-var burden-balance violation a verdict carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BalanceViolationKind {
    /// CFG-merge predecessors exit with divergent nets — per-path imbalance.
    MergeDisagree,
    /// A reachable terminal block (`Return` / `Resume` / `Unreachable`)
    /// nets nonzero — function-exit imbalance on a straight-line path.
    TerminalNonZero,
}

/// First per-var burden-balance violation, with the per-block delta the
/// verdict was derived from (consumers reuse it for diagnostics).
pub(crate) struct BalanceVerdict {
    pub violation: Option<BalanceViolation>,
    pub delta: Vec<i64>,
}

/// One burden-balance violation: the block index where it surfaced and the
/// observed net (`expected_net` is always 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BalanceViolation {
    pub kind: BalanceViolationKind,
    pub block_idx: usize,
    pub observed_net: i64,
}

/// Per-var burden-balance verdict — SSOT for the verdict-extraction layer
/// shared by `verify_burden_balance` (full-function VF-1) and
/// `verify_trmc_burden_balance` (TRMC PL-10). Both map the verdict to their
/// error type; neither re-derives the merge-suppression semantics.
///
/// Merge disagreement is reported FIRST and suppresses the terminal-net
/// check (the function-exit net is undefined when a merge point already
/// diverges). Otherwise the first reachable terminal block whose
/// `entry_net + delta != 0` is the violation. At most one violation per var.
pub(crate) fn balance_verdict(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    var: ArcVarId,
) -> BalanceVerdict {
    let delta = compute_var_block_deltas(func, var);
    let nets = compute_burden_entry_nets(func, preds, &delta);

    if let Some(&(block_idx, observed_net)) = nets.disagree_blocks.first() {
        return BalanceVerdict {
            violation: Some(BalanceViolation {
                kind: BalanceViolationKind::MergeDisagree,
                block_idx,
                observed_net,
            }),
            delta,
        };
    }

    for (b, block) in func.blocks.iter().enumerate() {
        let Some(eb) = nets.entry_net[b] else {
            continue;
        };
        let is_terminal = matches!(
            block.terminator,
            crate::ir::ArcTerminator::Return { .. }
                | crate::ir::ArcTerminator::Resume
                | crate::ir::ArcTerminator::Unreachable
        );
        if !is_terminal {
            continue;
        }
        let observed = eb + delta[b];
        if observed != 0 {
            return BalanceVerdict {
                violation: Some(BalanceViolation {
                    kind: BalanceViolationKind::TerminalNonZero,
                    block_idx: b,
                    observed_net: observed,
                }),
                delta,
            };
        }
    }

    BalanceVerdict {
        violation: None,
        delta,
    }
}
