//! Post-emission query passes: RC-incremented variable tracking.
//!
//! Produces auxiliary data used by downstream pipeline steps (COW annotations,
//! drop hints).

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

#[cfg(test)]
mod tests;

/// Collect all variables whose underlying object had an `RcInc` emitted.
///
/// Returns a set containing:
/// - Every variable `v` that has an `RcInc { var: v }` instruction
/// - Every variable on a `Let { dst, value: Var(src) }` alias edge whose
///   OTHER end is in the set — BOTH directions, transitive through alias
///   chains. `dst` and `src` name the SAME allocation, so an inc on either
///   bumps the shared object's physical refcount: forward (src incremented →
///   dst incremented) covers reads through later aliases; backward (dst
///   incremented → src incremented, re-distributed to every sibling by the
///   forward pass) covers a kept duplication-alias inc (the RL-1 funded
///   call-arg / store families keep the inc on the ALIAS while the root and
///   its other aliases stay live — classifying those as un-incremented would
///   promote a later COW site to `StaticUnique` and mutate the physically
///   shared buffer in place)
/// - Every block parameter that receives an incremented variable through
///   a `Jump { target, args }` terminator (phi-edge propagation)
/// - Every Select operand (`true_val` / `false_val`) when the `dst` is
///   incremented (BUG-04-090 E-mat fix). At runtime, `Select dst` IS
///   one of `{true_val, false_val}` — the Session H/I compensating Inc on
///   `dst` bumps the chosen operand's allocation RC, so the AIMS-side
///   `Unique` classification on those operands no longer reflects physical
///   reality and unique-drop fast-path emission would free past the bumped
///   RC. Propagation is conservative: marks BOTH operands incremented
///   regardless of the runtime cond, because either could be the chosen.
///
/// Used by both COW annotations and drop hints: after RC emission, any
/// variable in this set shares a heap object whose physical refcount was
/// bumped above 1. The pre-emission AIMS uniqueness state (`Unique`) no
/// longer reflects the physical reality for these variables.
pub(crate) fn collect_rc_incremented_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    use crate::ir::ArcTerminator;

    let mut incremented = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];
    // Select operand aliases: dst → (true_val, false_val). Propagation in the
    // fixed-point loop below mirrors `Let { Var(src) }` but edges TWO ways
    // because a Select's dst aliases EITHER operand at runtime.
    let mut select_aliases: Vec<(ArcVarId, ArcVarId, ArcVarId)> = Vec::new();

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => {
                    incremented.insert(*var);
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    alias_edges[dst.index()] = Some(*src);
                }
                ArcInstr::Select {
                    dst,
                    true_val,
                    false_val,
                    ..
                } => {
                    select_aliases.push((*dst, *true_val, *false_val));
                }
                _ => {}
            }
        }

        // Phi-edge propagation: Jump args map to target block params.
        // A block param may receive values from multiple predecessors
        // (e.g., loop header from entry + back-edge). If ANY source is
        // incremented, the param inherits the flag. Single-source cases
        // record an `alias_edges` entry; multi-source uses direct insertion.
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_idx = target.index();
            if target_idx < func.blocks.len() {
                for (i, &arg) in args.iter().enumerate() {
                    if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                        if let Some(existing) = alias_edges[param_var.index()] {
                            // Multi-predecessor: param already has an alias
                            // edge. If the new arg OR the existing source is
                            // incremented, mark the param directly. This
                            // handles loop headers where one predecessor
                            // passes an incremented var and the back-edge
                            // passes the param itself.
                            if incremented.contains(&arg) || incremented.contains(&existing) {
                                incremented.insert(param_var);
                            }
                        } else {
                            alias_edges[param_var.index()] = Some(arg);
                        }
                    }
                }
            }
        }
    }

    // Propagate: for each alias edge dst → src, if EITHER end is incremented,
    // the other is too (both name the same object; see the fn doc's
    // bidirectional rationale). Fixed-point loop handles chains like
    // %11 = %8 = %6 (where %6 has RcInc) and sibling fan-outs
    // %5 = %3, %8 = %3 (where %5 has the kept duplication inc).
    //
    // Select propagation (BUG-04-090 E-mat): for each Select instruction
    // `dst = Select cond ? true_val: false_val`, if `dst` is incremented,
    // BOTH operands are incremented (at runtime dst aliases one of them, with
    // the bumped RC). Conservative two-way propagation matches `
    // §1.9 Rule 5` for the `project_alias_sources` Select case.
    loop {
        let mut changed = false;
        for (i, alias) in alias_edges.iter().enumerate() {
            if let Some(src) = alias {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "ARC IR var counts fit in u32"
                )]
                let dst = ArcVarId::new(i as u32);
                if incremented.contains(src) && incremented.insert(dst) {
                    changed = true;
                }
                if incremented.contains(&dst) && incremented.insert(*src) {
                    changed = true;
                }
            }
        }
        for &(dst, true_val, false_val) in &select_aliases {
            if incremented.contains(&dst) {
                if incremented.insert(true_val) {
                    changed = true;
                }
                if incremented.insert(false_val) {
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    incremented
}

/// Collect borrowed function parameter variables and their Let-alias
/// transitive closure.
///
/// Returns a set containing every function parameter with
/// `Ownership::Borrowed` plus all `Let { dst, value: Var(src) }` aliases
/// that transitively derive from them. Used by `decide_drop_hint` to
/// prevent unique-drop on borrowed params (the caller retains a reference,
/// so the buffer is never uniquely owned by the callee).
pub(crate) fn collect_param_borrowed_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    use crate::ownership::Ownership;

    let mut result = FxHashSet::default();
    for param in &func.params {
        if param.ownership == Ownership::Borrowed {
            result.insert(param.var);
        }
    }
    if result.is_empty() {
        return result;
    }
    // Transitive closure through Let aliases AND block params (Jump/Branch
    // args → target block params). Borrowed-param values flow through the
    // loop header and exit block as block parameters.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            // Let aliases: %3 = %0 where %0 is a borrowed param.
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if result.contains(src) && result.insert(*dst) {
                        changed = true;
                    }
                }
            }
            // Block param propagation: Jump args map to target block params.
            // If arg N is in the result set, add target's block param N.
            let targets: Vec<(usize, &[ArcVarId])> = match &block.terminator {
                crate::ir::ArcTerminator::Jump { target, args } => {
                    vec![(target.index(), args)]
                }
                _ => vec![],
            };
            for (target_idx, args) in targets {
                if target_idx >= func.blocks.len() {
                    continue;
                }
                let target_block = &func.blocks[target_idx];
                for (arg_pos, arg_var) in args.iter().enumerate() {
                    if result.contains(arg_var) && arg_pos < target_block.params.len() {
                        let (param_var, _) = target_block.params[arg_pos];
                        if result.insert(param_var) {
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    result
}
