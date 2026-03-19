//! Field usage scanning for surgical struct loading.
//!
//! Scans an ARC function to determine which struct fields are actually
//! accessed via `Project` instructions. This enables loading only the
//! fields that are needed, replacing unused fields with `undef`.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::Name;
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
#[expect(
    clippy::too_many_lines,
    reason = "field scan walks all instructions tracking projection usage"
)]
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

// Helpers for compute_pointer_only_params.

/// If var is a param alias, mark its root as needing load.
fn mark_needs_load(
    var: ArcVarId,
    var_to_param: &FxHashMap<ArcVarId, ArcVarId>,
    needs_load: &mut FxHashSet<ArcVarId>,
) {
    if let Some(&root) = var_to_param.get(&var) {
        needs_load.insert(root);
    }
}

/// Helper: mark all vars in a slice as needing load.
fn mark_needs_load_slice(
    vars: &[ArcVarId],
    var_to_param: &FxHashMap<ArcVarId, ArcVarId>,
    needs_load: &mut FxHashSet<ArcVarId>,
) {
    for v in vars {
        mark_needs_load(*v, var_to_param, needs_load);
    }
}

/// Identify Indirect/Reference parameters whose loaded aggregate values are
/// provably never needed — all uses go through pointer forwarding.
///
/// A parameter is "pointer-only" when every use of it (and its aliases) in
/// the ARC IR is as an `Apply` or `Invoke` arg to a callee that the emitter
/// resolves through the ABI path (which checks `borrowed_param_ptrs`).
#[expect(
    clippy::too_many_lines,
    reason = "pointer-only scan walks all instructions checking forwarding safety"
)]
pub(super) fn compute_pointer_only_params(
    func: &ArcFunction,
    is_forwarding_safe: impl Fn(Name, &[ArcVarId]) -> bool,
) -> FxHashSet<ArcVarId> {
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    if param_vars.is_empty() {
        return FxHashSet::default();
    }

    // Build alias map: dst → src (Let { dst, Var(src) }).
    let mut reverse_aliases: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                reverse_aliases.entry(*src).or_default().push(*dst);
            }
        }
    }

    // Collect all aliases (transitive) of each param var.
    let mut var_to_param: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for &pv in &param_vars {
        var_to_param.insert(pv, pv);
        let mut worklist = vec![pv];
        while let Some(v) = worklist.pop() {
            if let Some(dsts) = reverse_aliases.get(&v) {
                for &d in dsts {
                    if var_to_param.insert(d, pv).is_none() {
                        worklist.push(d);
                    }
                }
            }
        }
    }

    // Track which param roots need loading.
    let mut needs_load: FxHashSet<ArcVarId> = FxHashSet::default();

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                // Apply: safe IF callee uses pointer forwarding for its args.
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    if !is_forwarding_safe(*callee, args) {
                        mark_needs_load_slice(args, &var_to_param, &mut needs_load);
                    }
                }

                // PrimOp needs loaded values. Let{Var} aliases and Let{Literal}
                // don't need the param value.
                ArcInstr::Let {
                    value: ori_arc::ir::ArcValue::PrimOp { args, .. },
                    ..
                } => mark_needs_load_slice(args, &var_to_param, &mut needs_load),

                ArcInstr::Let { .. } => {}

                ArcInstr::PartialApply { args, .. }
                | ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. } => {
                    mark_needs_load_slice(args, &var_to_param, &mut needs_load);
                }
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    mark_needs_load(*closure, &var_to_param, &mut needs_load);
                    mark_needs_load_slice(args, &var_to_param, &mut needs_load);
                }
                ArcInstr::CollectionReuse { old_var, args, .. } => {
                    mark_needs_load(*old_var, &var_to_param, &mut needs_load);
                    mark_needs_load_slice(args, &var_to_param, &mut needs_load);
                }
                ArcInstr::Project { value, .. } => {
                    mark_needs_load(*value, &var_to_param, &mut needs_load);
                }
                ArcInstr::Set { base, value, .. } => {
                    mark_needs_load(*base, &var_to_param, &mut needs_load);
                    mark_needs_load(*value, &var_to_param, &mut needs_load);
                }
                ArcInstr::SetTag { base, .. } => {
                    mark_needs_load(*base, &var_to_param, &mut needs_load);
                }
                ArcInstr::Select {
                    cond,
                    true_val,
                    false_val,
                    ..
                } => {
                    mark_needs_load(*cond, &var_to_param, &mut needs_load);
                    mark_needs_load(*true_val, &var_to_param, &mut needs_load);
                    mark_needs_load(*false_val, &var_to_param, &mut needs_load);
                }
                ArcInstr::RcInc { var, .. }
                | ArcInstr::RcDec { var, .. }
                | ArcInstr::IsShared { var, .. }
                | ArcInstr::Reset { var, .. } => {
                    mark_needs_load(*var, &var_to_param, &mut needs_load);
                }
            }
        }

        // Terminators.
        match &block.terminator {
            ArcTerminator::Return { value } => {
                mark_needs_load(*value, &var_to_param, &mut needs_load);
            }
            // Invoke args: same as Apply — safe if callee uses forwarding.
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => {
                if !is_forwarding_safe(*callee, args) {
                    mark_needs_load_slice(args, &var_to_param, &mut needs_load);
                }
            }
            // Jump args transfer values to block params — conservative: needs load.
            ArcTerminator::Jump { args, .. } => {
                mark_needs_load_slice(args, &var_to_param, &mut needs_load);
            }
            ArcTerminator::Branch { cond, .. } => {
                mark_needs_load(*cond, &var_to_param, &mut needs_load);
            }
            ArcTerminator::Switch { scrutinee, .. } => {
                mark_needs_load(*scrutinee, &var_to_param, &mut needs_load);
            }
            ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        }
    }

    // Return param vars that don't need loading.
    param_vars.difference(&needs_load).copied().collect()
}
