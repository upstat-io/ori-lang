//! RL-DROP §8.1.1 copy-out treatment for the class-ledger plan.
//!
//! A user-drop value RUNTIME-COPIED into a map/set at a borrowed `insert`
//! arg is not released at its local site: the stored copy carries the single
//! `@drop` at the container's teardown, so the local site owes a fields-only
//! release. This module detects the copy-out store args and rewrites the
//! affected classes' placed whole-var releases accordingly.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::intraprocedural::birth_site_partition::BirthSitePartition;
use crate::ir::{ArcFunction, ArcVarId};

use super::emit::{self, ClassOutcome};
use super::ClassLedgerAnalysis;

/// Borrowed store args whose value is RUNTIME-COPIED into a map/set: a
/// borrowed arg (position > 0) of an `insert` terminator-`Invoke` whose
/// receiver's definer chain resolves to a `Construct` with a
/// `MapLiteral`/`SetLiteral` ctor. The stored copy carries the value's
/// `@drop` at the container's teardown (RL-DROP §8.1.1 copy-out); the local
/// site owes a fields-only release. Receivers without a local
/// map/set-literal `Construct` definer stay unmarked (conservative).
fn copy_out_store_args(
    func: &ArcFunction,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    use crate::ir::{ArcInstr, ArcValue, CtorKind};

    let copy_in_names = crate::borrow::copy_in_builtin_names(interner);
    let definer_is_map_or_set = |mut var: ArcVarId| loop {
        let mut definer = None;
        for arc_block in &func.blocks {
            for instr in &arc_block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if *dst == var => definer = Some(Err(*src)),
                    ArcInstr::Construct { dst, ctor, .. } if *dst == var => {
                        definer = Some(Ok(matches!(
                            ctor,
                            CtorKind::MapLiteral | CtorKind::SetLiteral
                        )));
                    }
                    _ => {}
                }
            }
        }
        match definer {
            Some(Ok(verdict)) => return verdict,
            Some(Err(src)) => var = src,
            None => return false,
        }
    };
    let mut out = FxHashSet::default();
    for arc_block in &func.blocks {
        let crate::ir::ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &arc_block.terminator
        else {
            continue;
        };
        if !copy_in_names.contains(callee) || args.is_empty() {
            continue;
        }
        if !definer_is_map_or_set(args[0]) {
            continue;
        }
        for (position, &arg) in args.iter().enumerate().skip(1) {
            let borrowed = arg_ownership
                .get(position)
                .is_some_and(|o| matches!(o, crate::ir::ArgOwnership::Borrowed));
            let user_drop = func.var_types.get(arg.index()).is_some_and(|&ty| {
                crate::lower::burden_lookup::type_has_user_drop(ty, type_registry)
            });
            if borrowed && user_drop {
                out.insert(arg);
            }
        }
    }
    out
}

/// RL-DROP §8.1.1 copy-out: rewrite the placed WHOLE-VAR releases of a
/// copy-out class to fields-only (`DecPartial` with an empty skip set) —
/// the local site is not the value's death point (the stored copy's
/// teardown glue carries the single `@drop`); the local's funded reference
/// still releases its fields (`RLDROP_copyout_balanced`).
pub(super) fn apply_copy_out_rewrite(
    func: &ArcFunction,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    partition: &mut BirthSitePartition,
    analysis: &mut ClassLedgerAnalysis,
) {
    let store_args = copy_out_store_args(func, type_registry, interner);
    if store_args.is_empty() {
        return;
    }
    let nodes = partition.nodes_snapshot();
    let copy_out_reps: FxHashSet<_> = nodes
        .iter()
        .filter(|(var, path, _)| path.is_whole_var() && store_args.contains(var))
        .map(|(_, _, node)| partition.rep_of(*node))
        .collect();
    if copy_out_reps.is_empty() {
        return;
    }
    let var_rep: FxHashMap<ArcVarId, _> = nodes
        .iter()
        .filter(|(_, path, _)| path.is_whole_var())
        .map(|(var, _, node)| (*var, partition.rep_of(*node)))
        .collect();
    for entry in &mut analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &mut entry.outcome else {
            continue;
        };
        for op in ops.iter_mut() {
            let rewrite = matches!(op.kind, emit::PlannedOpKind::Dec)
                && var_rep
                    .get(&op.var)
                    .is_some_and(|rep| copy_out_reps.contains(rep));
            if rewrite {
                op.kind = emit::PlannedOpKind::DecPartial {
                    skip_fields: Vec::new(),
                };
            }
        }
    }
    analysis.copy_out_covered = var_rep
        .iter()
        .filter(|(_, rep)| copy_out_reps.contains(*rep))
        .map(|(var, _)| *var)
        .collect();
}
