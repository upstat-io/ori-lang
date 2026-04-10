//! Project-escape `RcInc` compensation (Phase 2.1 of unified RC emission).
//!
//! When edge cleanup emits `RcDec [AggFields]` for a parent aggregate that is
//! live at block exit but dead in a successor, it decrements ALL struct fields —
//! including projected children that were passed to the successor via Jump args.
//! This module inserts compensating `RcInc` operations so those projected
//! children survive their parent's dec.

use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

/// Emit `RcInc` for projected children of deferred parents that escape the
/// block via terminator arguments (Jump/Branch/Switch).
///
/// When a parent aggregate's `RcDec [AggFields]` is deferred to edge cleanup
/// (emitted AFTER the terminator), it decrements ALL struct fields. If a
/// projected child is passed to a successor via the terminator args, the child
/// must survive the parent's dec. Without `RcInc`, the field's refcount drops.
///
/// Insert `RcInc` for projected children that escape via terminator args
/// when the parent aggregate will be cleaned up by edge cleanup.
///
/// Edge cleanup emits `RcDec [AggFields]` for parent aggregates that are
/// live at block exit but dead in a successor. This decs ALL struct fields,
/// including projected ones that were passed to the successor via Jump args.
/// Without a compensating `RcInc`, the projected field is freed while the
/// successor still holds a reference → use-after-free.
///
/// This function scans each block: if a parent aggregate is live at exit
/// and has a projected child that escapes via the terminator, it inserts
/// `RcInc` for the projected child at the end of the block body (before
/// the terminator).
pub(super) fn emit_project_escape_incs(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
    _all_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    use crate::aims::emit_rc::{block_id, rc_strategy};

    let mut incs_to_insert: Vec<(usize, Vec<ArcInstr>)> = Vec::new();
    let mut succ_decs: Vec<(usize, ArcVarId, RcStrategy)> = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);
        let ArcTerminator::Jump { args, target, .. } = &block.terminator else {
            continue;
        };
        if args.is_empty() {
            continue;
        }
        let target_idx = target.index();

        let doomed_parents =
            find_edge_decced_project_parents(block, blk, args, state_map, func_project_sources);
        if doomed_parents.is_empty() {
            continue;
        }

        // For each arg tracing to a doomed parent: RcInc on the arg itself
        // (the specific projected child that is escaping), plus a successor
        // RcDec on the corresponding merge-block parameter.
        //
        // Previously this used a single `parent -> project_dst` map which
        // collapsed multiple escaping children from the same parent into one
        // slot. Now each arg gets its own RcInc keyed to its own identity.
        let var_to_parent = build_var_to_parent(block, args, func_project_sources);
        let mut block_incs = Vec::new();
        for (arg_pos, &arg) in args.iter().enumerate() {
            let Some(&parent) = var_to_parent.get(&arg) else {
                continue;
            };
            if !doomed_parents.contains(&parent) {
                continue;
            }
            let Some(strategy) = rc_strategy(func, arg, pool) else {
                continue;
            };

            block_incs.push(ArcInstr::RcInc {
                var: arg,
                count: 1,
                strategy,
            });

            let final_target = follow_jump_chain(func, target_idx);
            if let Some(&(param_var, _)) = func
                .blocks
                .get(final_target)
                .and_then(|b| b.params.get(arg_pos))
            {
                let ps = rc_strategy(func, param_var, pool).unwrap_or(strategy);
                succ_decs.push((final_target, param_var, ps));
            }
        }
        if !block_incs.is_empty() {
            incs_to_insert.push((block_idx, block_incs));
        }
    }

    for (block_idx, incs) in incs_to_insert {
        func.blocks[block_idx].body.extend(incs);
    }
    let mut seen: FxHashSet<(usize, u32)> = FxHashSet::default();
    for (succ_idx, var, strategy) in succ_decs {
        if succ_idx < func.blocks.len() && seen.insert((succ_idx, var.raw())) {
            func.blocks[succ_idx]
                .body
                .push(ArcInstr::RcDec { var, strategy });
        }
    }
}

/// Build a mapping from variables to their Project source parent.
///
/// Combines block-local Projects, function-level Project sources for
/// terminator args, and Let aliases in the block body.
fn build_var_to_parent(
    block: &crate::ir::ArcBlock,
    args: &[ArcVarId],
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut map: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, value, .. } = instr {
            map.insert(*dst, *value);
        }
    }
    for &arg in args {
        if let Some(&parent) = func_project_sources.get(&arg) {
            map.entry(arg).or_insert(parent);
        }
    }
    for instr in &block.body {
        if let ArcInstr::Let {
            dst,
            value: crate::ir::ArcValue::Var(src),
            ..
        } = instr
        {
            let parent = map
                .get(src)
                .copied()
                .or_else(|| func_project_sources.get(src).copied());
            if let Some(p) = parent {
                map.insert(*dst, p);
            }
        }
    }
    map
}

/// Find parent aggregates that will be edge-dec'd and have projected children
/// escaping via terminator args.
///
/// Returns the set of doomed parent variable IDs. Each arg that traces to a
/// doomed parent needs its own compensating `RcInc` — the caller resolves
/// per-arg identity, not this function.
fn find_edge_decced_project_parents(
    block: &crate::ir::ArcBlock,
    blk: ArcBlockId,
    args: &[ArcVarId],
    state_map: &AimsStateMap,
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    use crate::aims::emit_rc::is_live_at_exit;
    use crate::aims::lattice::Cardinality;
    use crate::graph::successor_block_ids;

    let var_to_parent = build_var_to_parent(block, args, func_project_sources);
    let successors = successor_block_ids(&block.terminator);

    let mut doomed: FxHashSet<ArcVarId> = FxHashSet::default();
    for &parent in var_to_parent.values() {
        if !is_live_at_exit(state_map, blk, parent) {
            continue;
        }
        for succ_id in &successors {
            if state_map
                .var_state_at_block_entry(*succ_id, parent)
                .cardinality
                == Cardinality::Absent
            {
                doomed.insert(parent);
                break;
            }
        }
    }

    doomed
}

/// Follow a chain of Jump terminators through trampoline blocks to find
/// the final merge block.
///
/// Edge cleanup creates trampoline blocks between predecessors and merge
/// blocks. Trampolines have params (they receive Jump args) but are
/// single-use intermediaries. This function skips trampolines (blocks
/// whose body is only `RcDec` instructions) and returns the first block
/// with non-`RcDec` body content or the end of the chain.
fn follow_jump_chain(func: &ArcFunction, mut idx: usize) -> usize {
    let mut visited = 0;
    while visited < func.blocks.len() {
        let Some(block) = func.blocks.get(idx) else {
            break;
        };
        // A trampoline block has body containing only RcDec instructions.
        let is_trampoline = !block.body.is_empty()
            && block
                .body
                .iter()
                .all(|i| matches!(i, ArcInstr::RcDec { .. }));
        if !is_trampoline {
            return idx;
        }
        if let ArcTerminator::Jump { target, .. } = &block.terminator {
            idx = target.index();
            visited += 1;
        } else {
            break;
        }
    }
    idx
}
