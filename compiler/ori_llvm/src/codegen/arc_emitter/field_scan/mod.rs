//! Field usage scanning for surgical struct loading.
//!
//! Scans an ARC function to determine which struct fields are actually
//! accessed via `Project` instructions. This enables loading only the
//! fields that are needed, replacing unused fields with `undef`.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(test)]
mod tests;

/// Scan an ARC function to determine which fields of each variable are accessed.
///
/// Returns a map from variable ID to the set of field indices accessed via
/// `Project`. Variables used in any other context (Apply args, Return,
/// Construct, etc.) are marked as needing ALL fields loaded — represented
/// by `None` in the map. Variables not in the map at all have no field
/// accesses (they may be scalars or unused).
///
/// This enables surgical struct loading: only fields that are actually
/// projected are loaded from memory. Unused fields get `undef`.
pub(super) fn scan_used_fields(func: &ArcFunction) -> FxHashMap<ArcVarId, Option<FxHashSet<u32>>> {
    /// Resolve a variable through alias chains to its root.
    fn resolve(aliases: &FxHashMap<ArcVarId, ArcVarId>, mut var: ArcVarId) -> ArcVarId {
        let mut depth = 0;
        while let Some(&src) = aliases.get(&var) {
            var = src;
            depth += 1;
            if depth > 100 {
                debug_assert!(false, "alias cycle detected for var {var:?}");
                break;
            }
        }
        var
    }

    /// Mark a variable (resolved through aliases) as needing all fields.
    fn mark_all(
        aliases: &FxHashMap<ArcVarId, ArcVarId>,
        usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
        var: ArcVarId,
    ) {
        usage.insert(resolve(aliases, var), None);
    }

    /// Mark each variable in a slice as needing all fields.
    fn mark_all_slice(
        aliases: &FxHashMap<ArcVarId, ArcVarId>,
        usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
        vars: &[ArcVarId],
    ) {
        for v in vars {
            mark_all(aliases, usage, *v);
        }
    }

    // Phase 1: Build alias map (Let { Var } chains).
    let mut aliases: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                aliases.insert(*dst, *src);
            }
        }
    }

    // Phase 2: Collect field usage, resolving through aliases.
    let mut usage: FxHashMap<ArcVarId, Option<FxHashSet<u32>>> = FxHashMap::default();

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Project { value, field, .. } => {
                    let root = resolve(&aliases, *value);
                    if !matches!(usage.get(&root), Some(None)) {
                        if let Some(s) = usage
                            .entry(root)
                            .or_insert_with(|| Some(FxHashSet::default()))
                            .as_mut()
                        {
                            s.insert(*field);
                        }
                    }
                }

                // PrimOp uses its args as whole values (e.g., string concat).
                ArcInstr::Let {
                    value: ori_arc::ir::ArcValue::PrimOp { args, .. },
                    ..
                } => mark_all_slice(&aliases, &mut usage, args),

                // Let { Var } is an alias (phase 1), Let { Literal } has no var refs.
                ArcInstr::Let { .. } => {}

                ArcInstr::Apply { args, .. }
                | ArcInstr::PartialApply { args, .. }
                | ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. } => {
                    mark_all_slice(&aliases, &mut usage, args);
                }
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    mark_all(&aliases, &mut usage, *closure);
                    mark_all_slice(&aliases, &mut usage, args);
                }
                ArcInstr::CollectionReuse { old_var, args, .. } => {
                    mark_all(&aliases, &mut usage, *old_var);
                    mark_all_slice(&aliases, &mut usage, args);
                }
                ArcInstr::Set { base, value, .. } => {
                    mark_all(&aliases, &mut usage, *base);
                    mark_all(&aliases, &mut usage, *value);
                }
                ArcInstr::SetTag { base, .. } => mark_all(&aliases, &mut usage, *base),
                ArcInstr::Select {
                    cond,
                    true_val,
                    false_val,
                    ..
                } => {
                    mark_all(&aliases, &mut usage, *cond);
                    mark_all(&aliases, &mut usage, *true_val);
                    mark_all(&aliases, &mut usage, *false_val);
                }
                ArcInstr::RcInc { var, .. }
                | ArcInstr::RcDec { var, .. }
                | ArcInstr::IsShared { var, .. }
                | ArcInstr::Reset { var, .. } => mark_all(&aliases, &mut usage, *var),
            }
        }

        // Terminators that use whole variables.
        match &block.terminator {
            ArcTerminator::Return { value } => mark_all(&aliases, &mut usage, *value),
            ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => {
                mark_all_slice(&aliases, &mut usage, args);
            }
            ArcTerminator::Branch { cond, .. } => mark_all(&aliases, &mut usage, *cond),
            ArcTerminator::Switch { scrutinee, .. } => {
                mark_all(&aliases, &mut usage, *scrutinee);
            }
            ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        }
    }

    // Phase 3: Propagate usage from alias targets back to sources.
    //
    // When Let bindings create aliases (e.g., `%0 = Var(%13)` from TCO
    // loop-header Let bindings), usage of `%0` gets resolved to `%13`.
    // But callers look up the original variable (`%0`) — e.g., for
    // function parameter binding. Propagate the resolved entry back so
    // alias sources are also correctly marked.
    for &src in aliases.keys() {
        let root = resolve(&aliases, src);
        if let Some(entry) = usage.get(&root).cloned() {
            usage.entry(src).or_insert(entry);
        }
    }

    usage
}
