//! RL-2 borrowed-`Invoke`-arg terminal-read release relocation on the
//! burden-sole path. Spec: Annex E §AIMS RL-2 + RL-4.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

/// Borrow-bundle of the Phase-5 scan outputs the relocation gates consult.
pub(in crate::lower::burden_lower) struct BorrowedInvokeRelocationInputs<'a> {
    pub last_uses_at: &'a FxHashMap<(usize, usize), Vec<ArcVarId>>,
    pub terminator_transfer_per_block: &'a [FxHashSet<ArcVarId>],
    pub full_move_vars: &'a FxHashSet<ArcVarId>,
    pub transfer_via_move_alias: &'a FxHashSet<ArcVarId>,
    pub genuine_dup_call_arg_aliases: &'a FxHashSet<ArcVarId>,
    pub transfer_through_return_param_vars: &'a FxHashSet<ArcVarId>,
    pub partial_move_vars: &'a FxHashMap<ArcVarId, Vec<u32>>,
    pub live_out_per_block: &'a [FxHashSet<ArcVarId>],
}

/// Relocation verdict: block-entry releases to place, the per-block relocated
/// set (the terminator-dec suppression set), and the fresh-root vars whose
/// fresh-site keep-alive inc is surplus under the relocated placement.
pub(in crate::lower::burden_lower) struct BorrowedInvokeRelocation {
    pub relocated_by_block: FxHashMap<usize, FxHashSet<ArcVarId>>,
    pub entry_releases: Vec<(usize, ArcVarId)>,
    pub suppressed_fresh_root_incs: Vec<ArcVarId>,
}

/// The sole emitter owes the borrowed arg's terminal-read release, but a
/// pre-terminator dec frees the arg BEFORE the call reads it (PV-2 clause 2
/// demands count >= 1 at every read). Relocate each such dec to BOTH invoke
/// successors' entries (the pre-terminator dec covered both edges; a
/// normal-only dec leaks the caught-panic path). Both successors single-pred,
/// so each entry release fires only for paths through THIS invoke. Multi-pred
/// successors + partial-move vars keep the legacy placement.
///
/// A relocated FRESH ROOT (`Construct` / non-alias `Let` definer) owes exactly
/// one release — its birth ref, paid by the relocated dec. The fresh-site
/// keep-alive inc previously canceled against the same-block pre-terminator
/// dec at coalesce; cross-block relocation defeats that pairing, so the
/// surplus inc is suppressed instead. Alias vars keep their duplication incs
/// (the alias pair still nets zero per path against the relocated dec).
///
/// Empty when `ORI_DISABLE_BORROWED_INVOKE_ARG_DEC_RELOCATION=1` (restores the
/// pre-terminator placement for bisection).
pub(in crate::lower::burden_lower) fn compute_relocated_borrowed_invoke_arg_decs(
    func: &ArcFunction,
    inputs: &BorrowedInvokeRelocationInputs<'_>,
) -> BorrowedInvokeRelocation {
    let mut relocation = BorrowedInvokeRelocation {
        relocated_by_block: FxHashMap::default(),
        entry_releases: Vec::new(),
        suppressed_fresh_root_incs: Vec::new(),
    };
    if std::env::var_os("ORI_DISABLE_BORROWED_INVOKE_ARG_DEC_RELOCATION").is_some() {
        return relocation;
    }
    let preds = crate::graph::compute_predecessors(func);
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let (normal, unwind) = match &block.terminator {
            ArcTerminator::Invoke { normal, unwind, .. }
            | ArcTerminator::InvokeIndirect { normal, unwind, .. } => {
                (normal.index(), unwind.index())
            }
            _ => continue,
        };
        if preds.get(normal).is_none_or(|p| p.len() != 1)
            || preds.get(unwind).is_none_or(|p| p.len() != 1)
        {
            continue;
        }
        let borrowed = super::super::emit::invoke_terminator_borrowed_args(&block.terminator);
        let Some(last_use_vars) = inputs.last_uses_at.get(&(block_idx, block.body.len())) else {
            continue;
        };
        for &var in last_use_vars {
            if !borrowed.contains(&var)
                || inputs.terminator_transfer_per_block[block_idx].contains(&var)
                || inputs.full_move_vars.contains(&var)
                || inputs.transfer_via_move_alias.contains(&var)
                || inputs.genuine_dup_call_arg_aliases.contains(&var)
                || inputs.transfer_through_return_param_vars.contains(&var)
                || inputs.partial_move_vars.contains_key(&var)
                || inputs
                    .live_out_per_block
                    .get(block_idx)
                    .is_some_and(|s| s.contains(&var))
            {
                continue;
            }
            relocation.entry_releases.push((normal, var));
            relocation.entry_releases.push((unwind, var));
            relocation
                .relocated_by_block
                .entry(block_idx)
                .or_default()
                .insert(var);
            if is_fresh_root_definition(func, var) {
                relocation.suppressed_fresh_root_incs.push(var);
            }
        }
    }
    relocation
}

/// True iff `var` is defined by a FRESH self-allocating instruction — a
/// `Construct` or a non-alias `Let` (literal / non-Var value). An alias
/// (`Let { value: Var }`) or a call result returns false.
fn is_fresh_root_definition(func: &ArcFunction, var: ArcVarId) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                crate::ir::ArcInstr::Construct { dst, .. } if *dst == var => return true,
                crate::ir::ArcInstr::Let { dst, value, .. } if *dst == var => {
                    return !matches!(value, crate::ir::ArcValue::Var(_));
                }
                _ => {}
            }
        }
    }
    false
}
