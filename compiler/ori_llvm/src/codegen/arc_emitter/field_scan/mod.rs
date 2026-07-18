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

fn resolve_alias(aliases: &FxHashMap<ArcVarId, ArcVarId>, mut var: ArcVarId) -> ArcVarId {
    let mut depth = 0;
    while let Some(&source) = aliases.get(&var) {
        var = source;
        depth += 1;
        if depth > 100 {
            debug_assert!(false, "alias cycle detected for var {var:?}");
            break;
        }
    }
    var
}

fn mark_all_fields(
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
    var: ArcVarId,
) {
    usage.insert(resolve_alias(aliases, var), None);
}

fn mark_all_fields_in(
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
    vars: &[ArcVarId],
) {
    for &var in vars {
        mark_all_fields(aliases, usage, var);
    }
}

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
    let aliases = collect_aliases(func);

    let mut usage: FxHashMap<ArcVarId, Option<FxHashSet<u32>>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            record_field_usage(instr, &aliases, &mut usage);
        }
        record_terminator_field_usage(&block.terminator, &aliases, &mut usage);
    }

    propagate_alias_field_usage(&aliases, &mut usage);
    usage
}

fn collect_aliases(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut aliases = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(source),
                ..
            } = instr
            {
                aliases.insert(*dst, *source);
            }
        }
    }
    aliases
}

fn record_field_usage(
    instr: &ArcInstr,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
) {
    match instr {
        ArcInstr::Project { value, field, .. } => {
            let root = resolve_alias(aliases, *value);
            if !matches!(usage.get(&root), Some(None)) {
                if let Some(fields) = usage
                    .entry(root)
                    .or_insert_with(|| Some(FxHashSet::default()))
                    .as_mut()
                {
                    fields.insert(*field);
                }
            }
        }
        ArcInstr::Let {
            value: ori_arc::ir::ArcValue::PrimOp { args, .. },
            ..
        }
        | ArcInstr::Apply { args, .. }
        | ArcInstr::PartialApply { args, .. }
        | ArcInstr::Construct { args, .. }
        | ArcInstr::Reuse { args, .. } => mark_all_fields_in(aliases, usage, args),
        ArcInstr::Let { .. } => {}
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            mark_all_fields(aliases, usage, *closure);
            mark_all_fields_in(aliases, usage, args);
        }
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            mark_all_fields(aliases, usage, *old_var);
            mark_all_fields_in(aliases, usage, args);
        }
        ArcInstr::Set { base, value, .. } => {
            mark_all_fields(aliases, usage, *base);
            mark_all_fields(aliases, usage, *value);
        }
        ArcInstr::SetTag { base, .. }
        | ArcInstr::RcDecField { base, .. }
        | ArcInstr::BurdenDecField { base, .. } => mark_all_fields(aliases, usage, *base),
        ArcInstr::Select {
            cond,
            true_val,
            false_val,
            ..
        } => {
            mark_all_fields(aliases, usage, *cond);
            mark_all_fields(aliases, usage, *true_val);
            mark_all_fields(aliases, usage, *false_val);
        }
        ArcInstr::RcInc { var, .. }
        | ArcInstr::RcDec { var, .. }
        | ArcInstr::RcDecPartial { var, .. }
        | ArcInstr::RcDecVariant { var }
        | ArcInstr::BurdenInc { var }
        | ArcInstr::BurdenDec { var }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::BurdenDecVariant { var }
        | ArcInstr::IsShared { var, .. }
        | ArcInstr::Reset { var, .. } => mark_all_fields(aliases, usage, *var),
    }
}

fn record_terminator_field_usage(
    terminator: &ArcTerminator,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
) {
    match terminator {
        ArcTerminator::Return { value } => mark_all_fields(aliases, usage, *value),
        ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => {
            mark_all_fields_in(aliases, usage, args);
        }
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            mark_all_fields(aliases, usage, *closure);
            mark_all_fields_in(aliases, usage, args);
        }
        ArcTerminator::Branch { cond, .. } => mark_all_fields(aliases, usage, *cond),
        ArcTerminator::Switch { scrutinee, .. } => mark_all_fields(aliases, usage, *scrutinee),
        ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}

fn propagate_alias_field_usage(
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
    usage: &mut FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
) {
    for &source in aliases.keys() {
        let root = resolve_alias(aliases, source);
        if let Some(entry) = usage.get(&root).cloned() {
            usage.entry(source).or_insert(entry);
        }
    }
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
pub(super) fn compute_pointer_only_params(
    func: &ArcFunction,
    is_forwarding_safe: impl Fn(Name, &[ArcVarId]) -> bool,
) -> FxHashSet<ArcVarId> {
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    if param_vars.is_empty() {
        return FxHashSet::default();
    }

    let var_to_param = collect_param_aliases(func, &param_vars);
    let mut needs_load: FxHashSet<ArcVarId> = FxHashSet::default();

    for block in &func.blocks {
        for instr in &block.body {
            record_pointer_load_for_instr(
                instr,
                &var_to_param,
                &mut needs_load,
                &is_forwarding_safe,
            );
        }
        record_pointer_load_for_terminator(
            &block.terminator,
            &var_to_param,
            &mut needs_load,
            &is_forwarding_safe,
        );
    }

    param_vars.difference(&needs_load).copied().collect()
}

fn collect_param_aliases(
    func: &ArcFunction,
    param_vars: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut reverse_aliases: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(source),
                ..
            } = instr
            {
                reverse_aliases.entry(*source).or_default().push(*dst);
            }
        }
    }

    let mut var_to_param = FxHashMap::default();
    for &param in param_vars {
        var_to_param.insert(param, param);
        let mut worklist = vec![param];
        while let Some(var) = worklist.pop() {
            if let Some(aliases) = reverse_aliases.get(&var) {
                for &alias in aliases {
                    if var_to_param.insert(alias, param).is_none() {
                        worklist.push(alias);
                    }
                }
            }
        }
    }
    var_to_param
}

fn record_pointer_load_for_instr(
    instr: &ArcInstr,
    var_to_param: &FxHashMap<ArcVarId, ArcVarId>,
    needs_load: &mut FxHashSet<ArcVarId>,
    is_forwarding_safe: &impl Fn(Name, &[ArcVarId]) -> bool,
) {
    match instr {
        ArcInstr::Apply {
            func: callee, args, ..
        } => {
            if !is_forwarding_safe(*callee, args) {
                mark_needs_load_slice(args, var_to_param, needs_load);
            }
        }
        ArcInstr::Let {
            value: ori_arc::ir::ArcValue::PrimOp { args, .. },
            ..
        }
        | ArcInstr::PartialApply { args, .. }
        | ArcInstr::Construct { args, .. }
        | ArcInstr::Reuse { args, .. } => mark_needs_load_slice(args, var_to_param, needs_load),
        // Aliases and literals require no loaded value. Burden markers emit no
        // LLVM IR, so loading for them would regress aggregate forwarding.
        ArcInstr::Let { .. } | ArcInstr::BurdenInc { .. } | ArcInstr::BurdenDec { .. } => {}
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            mark_needs_load(*closure, var_to_param, needs_load);
            mark_needs_load_slice(args, var_to_param, needs_load);
        }
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            mark_needs_load(*old_var, var_to_param, needs_load);
            mark_needs_load_slice(args, var_to_param, needs_load);
        }
        ArcInstr::Project { value, .. } => mark_needs_load(*value, var_to_param, needs_load),
        ArcInstr::Set { base, value, .. } => {
            mark_needs_load(*base, var_to_param, needs_load);
            mark_needs_load(*value, var_to_param, needs_load);
        }
        ArcInstr::SetTag { base, .. }
        | ArcInstr::BurdenDecField { base, .. }
        | ArcInstr::RcDecField { base, .. } => mark_needs_load(*base, var_to_param, needs_load),
        ArcInstr::Select {
            cond,
            true_val,
            false_val,
            ..
        } => {
            mark_needs_load(*cond, var_to_param, needs_load);
            mark_needs_load(*true_val, var_to_param, needs_load);
            mark_needs_load(*false_val, var_to_param, needs_load);
        }
        ArcInstr::RcInc { var, .. }
        | ArcInstr::RcDec { var, .. }
        | ArcInstr::RcDecPartial { var, .. }
        | ArcInstr::RcDecVariant { var }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::BurdenDecVariant { var }
        | ArcInstr::IsShared { var, .. }
        | ArcInstr::Reset { var, .. } => mark_needs_load(*var, var_to_param, needs_load),
    }
}

fn record_pointer_load_for_terminator(
    terminator: &ArcTerminator,
    var_to_param: &FxHashMap<ArcVarId, ArcVarId>,
    needs_load: &mut FxHashSet<ArcVarId>,
    is_forwarding_safe: &impl Fn(Name, &[ArcVarId]) -> bool,
) {
    match terminator {
        ArcTerminator::Return { value } => mark_needs_load(*value, var_to_param, needs_load),
        ArcTerminator::Invoke {
            func: callee, args, ..
        } => {
            if !is_forwarding_safe(*callee, args) {
                mark_needs_load_slice(args, var_to_param, needs_load);
            }
        }
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            mark_needs_load(*closure, var_to_param, needs_load);
            mark_needs_load_slice(args, var_to_param, needs_load);
        }
        ArcTerminator::Jump { args, .. } => {
            mark_needs_load_slice(args, var_to_param, needs_load);
        }
        ArcTerminator::Branch { cond, .. } => mark_needs_load(*cond, var_to_param, needs_load),
        ArcTerminator::Switch { scrutinee, .. } => {
            mark_needs_load(*scrutinee, var_to_param, needs_load);
        }
        ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}
