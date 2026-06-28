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
//! Full VF-1 (per-edge dataflow + SCC net-zero obligation) is target-only;
//! this module ships the basic function-exit form that gates burden-op
//! emission, elimination, and the coexistence handshake.
//!
//! Set `ORI_LOG=ori_arc::aims::verify::burden_balance=debug` to emit a
//! per-block burden-delta breakdown for each imbalanced var (locates the
//! surplus/deficit block for missing CFG-edge or dead-at-entry decs).

use crate::aims::verify::burden_delta::{balance_verdict, BalanceViolation, BalanceViolationKind};
use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::verify::{BurdenBalanceError, BurdenResidualOps};

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
        // Verdict extraction (merge-disagree first, suppressing the
        // terminal-net check; else first nonzero terminal) is the SSOT at
        // `burden_delta::balance_verdict` — shared with the TRMC verifier.
        let verdict = balance_verdict(func, &preds, var);
        if let Some(v) = verdict.violation {
            errors.push(BurdenBalanceError {
                var,
                expected_net: 0,
                observed_net: v.observed_net,
                exit_block: func.blocks[v.block_idx].id,
                def_kind: var_def_kind(func, var),
                exit_kind: violation_exit_kind(func, &v),
                var_repr: var_repr_kind(func, var),
                residual_ops: residual_ops_census(func, var),
            });
            trace_burden_imbalance(func, var, &verdict.delta);
        }
    }

    errors
}

/// Defining-site kind of `var` — the attribution dimension classifying
/// WHICH emission family produced the imbalanced lineage. Reused by the
/// RC-remark emitter (`aims::realize::rc_remark`) for the `def_kind` taxonomy.
pub(crate) fn var_def_kind(func: &ArcFunction, var: ArcVarId) -> &'static str {
    if func.params.iter().any(|p| p.var == var) {
        return "param";
    }
    for block in &func.blocks {
        if block.params.iter().any(|(v, _)| *v == var) {
            return "block-param";
        }
        for instr in &block.body {
            let kind = match instr {
                ArcInstr::Let { dst, value, .. } if *dst == var => match value {
                    ArcValue::Var(_) => "alias",
                    ArcValue::Literal(_) => "let-literal",
                    ArcValue::PrimOp { .. } => "let-primop",
                },
                ArcInstr::Apply { dst, .. } if *dst == var => "apply",
                ArcInstr::ApplyIndirect { dst, .. } if *dst == var => "apply-indirect",
                ArcInstr::PartialApply { dst, .. } if *dst == var => "closure",
                ArcInstr::Project { dst, .. } if *dst == var => "project",
                ArcInstr::Construct { dst, .. } if *dst == var => "construct",
                ArcInstr::IsShared { dst, .. } if *dst == var => "is-shared",
                ArcInstr::Reset { token, .. } if *token == var => "reset-token",
                ArcInstr::Reuse { dst, .. } if *dst == var => "reuse",
                ArcInstr::CollectionReuse { dst, .. } if *dst == var => "collection-reuse",
                ArcInstr::Select { dst, .. } if *dst == var => "select",
                _ => continue,
            };
            return kind;
        }
        match block.terminator {
            ArcTerminator::Invoke { dst, .. } if dst == var => return "invoke",
            ArcTerminator::InvokeIndirect { dst, .. } if dst == var => return "invoke-indirect",
            _ => {}
        }
    }
    "undef"
}

/// `var`'s machine repr label for attribution. Reused by the RC-remark emitter
/// (`aims::realize::rc_remark`) for the `var_repr` taxonomy.
pub(crate) fn var_repr_kind(func: &ArcFunction, var: ArcVarId) -> &'static str {
    match func.var_repr(var) {
        Some(crate::ir::ValueRepr::Scalar) => "scalar",
        Some(crate::ir::ValueRepr::RcPointer) => "rc-pointer",
        Some(crate::ir::ValueRepr::Aggregate) => "aggregate",
        Some(crate::ir::ValueRepr::FatValue) => "fat-value",
        None => "none",
    }
}

/// Violating path kind: `merge-disagree` for CFG-merge predecessor
/// disagreement; the terminal block's terminator kind otherwise.
fn violation_exit_kind(func: &ArcFunction, v: &BalanceViolation) -> &'static str {
    if v.kind == BalanceViolationKind::MergeDisagree {
        return "merge-disagree";
    }
    if v.kind == BalanceViolationKind::ConvergenceExhausted {
        return "convergence-exhausted";
    }
    match func.blocks[v.block_idx].terminator {
        ArcTerminator::Return { .. } => "return",
        ArcTerminator::Resume => "resume",
        ArcTerminator::Unreachable => "unreachable",
        _ => "non-terminal",
    }
}

/// Whole-function census of SURVIVING burden ops targeting `var` at
/// verification time (post-elimination, post-lowering residue).
fn residual_ops_census(func: &ArcFunction, var: ArcVarId) -> BurdenResidualOps {
    let mut census = BurdenResidualOps::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var: v } if *v == var => census.inc += 1,
                ArcInstr::BurdenDec { var: v } if *v == var => census.dec += 1,
                ArcInstr::BurdenDecPartial { var: v, .. } if *v == var => census.dec_partial += 1,
                ArcInstr::BurdenDecVariant { var: v } if *v == var => census.dec_variant += 1,
                _ => {}
            }
        }
    }
    census
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

#[cfg(test)]
mod tests;
