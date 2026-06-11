//! Loop-carried pure-borrow-view alias admission — Pass 4 of
//! [`collect_pair_atomic_alias_dsts`](super::collect_pair_atomic_alias_dsts):
//! a `Let { Var }` alias of a back-edge-carrying block-param whose EVERY use
//! is a `Project` read feeding only borrowed positions joins the pair-atomic
//! set (RL-1 atomicity for the loop's same-allocation views).
//! Spec: Annex E §AIMS RL-1 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::lower::burden_lower::loop_carried_pair_coupling_disabled;

/// Pass 4: loop-carried PURE-BORROW-VIEW aliases — a `Let { Var }` alias of
/// a loop-carried BLOCK-param (a back-edge-carrying param per the
/// per-predecessor-edge attribution) whose EVERY use is a `Project` read
/// feeding only borrowed positions. Such an alias is a same-allocation
/// view of the loop's threaded value — never a birth site — so its inc/dec
/// pair is RL-1-atomic exactly like the function-param case (the
/// branch-exclusive read-arm borrow aliases). Aliases that MOVE a
/// projected field (Construct-feeding carriers) stay DECOUPLED: their
/// splits are the load-bearing per-iteration release arrangement (the
/// whole-inc / partial-dec carrier split). Gated by
/// `ORI_DISABLE_LOOP_CARRIED_PAIR_COUPLING=1` independently.
pub(super) fn extend_with_loop_carried_borrow_view_aliases(
    func: &ArcFunction,
    root: &FxHashMap<ArcVarId, ArcVarId>,
    pair_atomic: &mut FxHashSet<ArcVarId>,
) {
    if loop_carried_pair_coupling_disabled() {
        return;
    }
    let root_of = |v: ArcVarId| root.get(&v).copied().unwrap_or(v);
    let edges = crate::aims::intraprocedural::project_aliases::compute_param_edge_args(func);
    let loop_params: FxHashSet<ArcVarId> = edges
        .iter()
        .filter(|(_, es)| es.iter().any(|e| e.is_back_edge))
        .map(|(&p, _)| p)
        .collect();
    if !loop_params.is_empty() {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    let rooted = loop_params.contains(&root_of(*src)) || loop_params.contains(src);
                    if rooted && is_pure_borrow_view_alias(func, *dst) {
                        pair_atomic.insert(*dst);
                    }
                }
            }
        }
    }
}

/// True iff EVERY use of `alias` is a `Project` read whose projection dst
/// feeds only BORROWED positions — no owned-position instr transfer
/// (`is_owned_position` / `Set.value`), no terminator use of the alias itself,
/// and no projection dst at an Owned call arg / `Return` / `Jump` / owned
/// terminator-arg position. A pure borrow-view never carries a birth `+1`, so
/// its whole-var inc/dec pair is bookkeeping that must move WHOLE (RL-1
/// atomicity); any move-out or onward-threading use disqualifies (those
/// lineages rely on the decoupled split as their release arrangement). Burden
/// ops on the alias itself are RC bookkeeping, not uses (TF-11).
fn is_pure_borrow_view_alias(func: &ArcFunction, alias: ArcVarId) -> bool {
    let mut projection_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    // Walk 1: every use of `alias` must be a Project read (or its own burden
    // bookkeeping); no terminator may use `alias`.
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } if *var == alias => {}
                ArcInstr::Project { dst, value, .. } if *value == alias => {
                    projection_dsts.insert(*dst);
                }
                _ => {
                    if instr.used_vars().contains(&alias) {
                        return false;
                    }
                }
            }
        }
        if block.terminator.used_vars().contains(&alias) {
            return false;
        }
    }
    if projection_dsts.is_empty() {
        return false;
    }
    // Walk 2: every projection dst feeds only borrowed positions.
    for block in &func.blocks {
        for instr in &block.body {
            let used = instr.used_vars();
            if !projection_dsts.iter().any(|d| used.contains(d)) {
                continue;
            }
            // Owned-position instr transfer (Construct / Reuse / owned call
            // arg / Set.value) disqualifies.
            for (pos, &arg) in used.iter().enumerate() {
                if projection_dsts.contains(&arg) && instr.is_owned_position(pos) {
                    return false;
                }
            }
            if let ArcInstr::Set { value, .. } = instr {
                if projection_dsts.contains(value) {
                    return false;
                }
            }
        }
        match &block.terminator {
            ArcTerminator::Return { value } => {
                if projection_dsts.contains(value) {
                    return false;
                }
            }
            ArcTerminator::Jump { args, .. } => {
                if args.iter().any(|a| projection_dsts.contains(a)) {
                    return false;
                }
            }
            ArcTerminator::Invoke {
                args,
                arg_ownership,
                ..
            }
            | ArcTerminator::InvokeIndirect {
                args,
                arg_ownership,
                ..
            } => {
                for (i, a) in args.iter().enumerate() {
                    if !projection_dsts.contains(a) {
                        continue;
                    }
                    // Owned (or unknown-ownership) terminator arg = transfer.
                    if arg_ownership.get(i) != Some(&crate::ir::ArgOwnership::Borrowed) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}
