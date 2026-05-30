//! Per-block burden-op net delta — shared by TRMC-scope
//! `verify_trmc_burden_balance` (in `aims/normalize/verify.rs`) and
//! full-function-scope `verify_burden_balance` (in
//! `aims/verify/burden_balance.rs`).
//!
//! Per `impl-hygiene.md §LEAK:algorithmic-duplication`, the same
//! `BurdenInc(v) - BurdenDec*(v)` walk over a block's body would otherwise
//! exist in two sites with identical control-flow shape; the helper here
//! is the canonical home.

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

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
            match instr {
                ArcInstr::BurdenInc { var: v } if *v == var => delta[idx] += 1,
                ArcInstr::BurdenDec { var: v } if *v == var => delta[idx] -= 1,
                ArcInstr::BurdenDecPartial { var: v, .. } if *v == var => delta[idx] -= 1,
                ArcInstr::BurdenDecVariant { var: v } if *v == var => delta[idx] -= 1,
                _ => {}
            }
        }
    }
    delta
}
