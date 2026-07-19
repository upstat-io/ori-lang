//! Slot-dominance placement checks for planned class-ledger ops.
//!
//! A planned op is placeable only when its subject variable's definition
//! dominates (and, same-block, precedes) the op's insertion slot.

use rustc_hash::FxHashMap;

use crate::graph::DominatorTree;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::emit::{PlanSlot, PlannedOp};

/// The definition point of a variable, for slot-dominance checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DefPoint {
    /// A function param — defined at entry, dominates every slot.
    Entry,
    /// Defined at a block's front: a block param, or an `Invoke` /
    /// `InvokeIndirect` result (materialized on entry to the NORMAL
    /// successor — the unwind successor never sees it).
    BlockEntry(usize),
    /// Defined by the body instruction at `(block, index)`.
    Body(usize, usize),
}

/// Whether every planned op's variable is defined at a point that dominates
/// (and, same-block, precedes) the op's insertion slot.
pub(super) fn ops_placeable(func: &ArcFunction, ops: &[PlannedOp]) -> bool {
    if ops.is_empty() {
        return true;
    }
    let defs = collect_def_points(func);
    let dom = DominatorTree::build(func);
    ops.iter().all(|op| {
        let placeable = defs
            .get(&op.var)
            .is_some_and(|&def| def_reaches_slot(func, &dom, def, op.slot));
        if !placeable {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                gate = "op-var-placement",
                var = ?op.var,
                slot = ?op.slot,
                def = ?defs.get(&op.var),
                "planned op's variable definition does not dominate its insertion slot"
            );
        }
        placeable
    })
}

/// Definition points of every variable in `func`.
pub(super) fn collect_def_points(func: &ArcFunction) -> FxHashMap<ArcVarId, DefPoint> {
    let mut defs: FxHashMap<ArcVarId, DefPoint> = FxHashMap::default();
    for param in &func.params {
        defs.insert(param.var, DefPoint::Entry);
    }
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(var, _) in &block.params {
            defs.insert(var, DefPoint::BlockEntry(block_idx));
        }
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let Some(dst) = instr.defined_var() {
                defs.insert(dst, DefPoint::Body(block_idx, instr_idx));
            }
        }
        if let ArcTerminator::Invoke { dst, normal, .. }
        | ArcTerminator::InvokeIndirect { dst, normal, .. } = &block.terminator
        {
            defs.insert(*dst, DefPoint::BlockEntry(normal.index()));
        }
    }
    defs
}

/// Whether `def` dominates `slot` — cross-block via the dominator tree
/// (blocks execute atomically, so a dominating block's whole body precedes
/// the slot), same-block via body position.
pub(super) fn def_reaches_slot(
    func: &ArcFunction,
    dom: &DominatorTree,
    def: DefPoint,
    slot: PlanSlot,
) -> bool {
    let slot_block = slot.block();
    if slot_block >= func.blocks.len() {
        return false;
    }
    match def {
        DefPoint::Entry => true,
        DefPoint::BlockEntry(def_block) => {
            def_block == slot_block || dominates(func, dom, def_block, slot_block)
        }
        DefPoint::Body(def_block, def_idx) => {
            if def_block == slot_block {
                match slot {
                    PlanSlot::BlockFront { .. } => false,
                    PlanSlot::BeforeBody { index, .. } => index > def_idx,
                    PlanSlot::AfterBody { index, .. } => index >= def_idx,
                    PlanSlot::BeforeTerminator { .. } => true,
                }
            } else {
                dominates(func, dom, def_block, slot_block)
            }
        }
    }
}

/// Block-index dominance via block ids (block ids equal block indices in
/// pipeline IR; a mismatch is conservative — an unreachable block never
/// dominates).
fn dominates(func: &ArcFunction, dom: &DominatorTree, a: usize, b: usize) -> bool {
    let (Some(block_a), Some(block_b)) = (func.blocks.get(a), func.blocks.get(b)) else {
        return false;
    };
    dom.dominates(block_a.id, block_b.id)
}
