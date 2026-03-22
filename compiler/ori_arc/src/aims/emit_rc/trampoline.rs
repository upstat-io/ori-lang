//! Trampoline block insertion for merge-edge RC cleanup.
//!
//! When a multi-predecessor successor needs per-edge `RcDec` operations,
//! a trampoline block is inserted between the predecessor and successor.
//! The trampoline copies successor params, emits `RcDec` ops, and jumps
//! to the original successor.

use rustc_hash::FxHashSet;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::{block_id, EdgeDec};

/// Insert a trampoline block to carry edge-specific `RcDec` operations.
pub(super) fn insert_trampoline(
    func: &mut ArcFunction,
    pred_idx: usize,
    succ_idx: usize,
    decs: &[EdgeDec],
) {
    let trampoline_id = block_id(func.blocks.len());
    let succ_id = block_id(succ_idx);

    let body: Vec<ArcInstr> = decs
        .iter()
        .map(|&(var, strategy)| ArcInstr::RcDec { var, strategy })
        .collect();
    let body_len = body.len();

    // Copy the successor's block params — the trampoline is a pass-through
    // that inserts RcDec ops between the predecessor and successor.
    let succ_params = func.blocks[succ_idx].params.clone();

    // The trampoline receives the same args as the successor, creates fresh
    // param variables to forward them, then jumps to the successor.
    let trampoline_params: Vec<(ArcVarId, ori_types::Idx)> = succ_params
        .iter()
        .map(|&(_, ty)| (func.fresh_var(ty), ty))
        .collect();
    let forward_args: Vec<ArcVarId> = trampoline_params.iter().map(|&(var, _)| var).collect();

    let trampoline_block = ArcBlock {
        id: trampoline_id,
        params: trampoline_params,
        body,
        terminator: ArcTerminator::Jump {
            target: succ_id,
            args: forward_args,
        },
    };

    // Push matching span entry — trampoline instructions have no source spans.
    let span_entry: Vec<Option<ori_ir::Span>> = vec![None; body_len];
    func.blocks.push(trampoline_block);
    func.spans.push(span_entry);
    retarget_terminator(
        &mut func.blocks[pred_idx].terminator,
        succ_id,
        trampoline_id,
    );
}

/// Compute the set of variables defined at or before a given block index.
///
/// Variables from project-source demand propagation at merge points may
/// appear in exit states even though they're defined DOWNSTREAM (in branch
/// successors). This function precomputes which variables actually exist
/// at a given block, so edge cleanup can filter out phantom demand.
///
/// Assumes blocks are in topological order (entry = 0).
pub(super) fn compute_defined_at_or_before(
    func: &ArcFunction,
    up_to: usize,
) -> FxHashSet<ArcVarId> {
    let mut set = FxHashSet::default();
    for (bi, block) in func.blocks.iter().enumerate() {
        if bi > up_to {
            continue;
        }
        for &(param, _) in &block.params {
            set.insert(param);
        }
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                set.insert(dst);
            }
        }
        if let ArcTerminator::Invoke { dst, .. } = &block.terminator {
            set.insert(*dst);
        }
    }
    for param in &func.params {
        set.insert(param.var);
    }
    set
}

/// Retarget a terminator: replace references to `old_target` with `new_target`.
fn retarget_terminator(
    terminator: &mut ArcTerminator,
    old_target: ArcBlockId,
    new_target: ArcBlockId,
) {
    match terminator {
        ArcTerminator::Jump { target, .. } => {
            if *target == old_target {
                *target = new_target;
            }
        }
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            if *then_block == old_target {
                *then_block = new_target;
            }
            if *else_block == old_target {
                *else_block = new_target;
            }
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for (_, target) in cases.iter_mut() {
                if *target == old_target {
                    *target = new_target;
                }
            }
            if *default == old_target {
                *default = new_target;
            }
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            if *normal == old_target {
                *normal = new_target;
            }
            if *unwind == old_target {
                *unwind = new_target;
            }
        }
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}
