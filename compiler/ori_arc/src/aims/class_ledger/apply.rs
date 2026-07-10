//! Plan materialization: insert planned burden ops into a function's
//! instruction stream. Production consumer: the per-function replacement
//! path (`replace::attempt_replacement`).

use crate::ir::{ArcFunction, ArcInstr};

use super::emit::{PlanSlot, PlannedOp, PlannedOpKind};

/// Insert every planned op at its slot. Insertions within one block apply
/// back-to-front so earlier indices stay valid. Ops sharing an insertion
/// point apply Inc BEFORE Dec (fund-before-release: a container release and
/// a same-slot funding inc of its extracted view name one allocation, and
/// releasing first frees the memory the inc then touches); within one kind,
/// plan order holds.
pub(crate) fn apply_plan(func: &mut ArcFunction, ops: &[PlannedOp]) {
    for block in 0..func.blocks.len() {
        let mut inserts: Vec<(usize, u8, usize, ArcInstr)> = ops
            .iter()
            .enumerate()
            .filter(|(_, op)| op.slot.block() == block)
            .map(|(order, op)| {
                let kind_rank = match op.kind {
                    PlannedOpKind::Inc => 0,
                    PlannedOpKind::Dec | PlannedOpKind::DecPartial { .. } => 1,
                };
                (
                    insertion_index(func, op.slot),
                    kind_rank,
                    order,
                    materialize(op),
                )
            })
            .collect();
        inserts.sort_by_key(|&(index, kind_rank, order, _)| (index, kind_rank, order));
        for (index, _, _, instr) in inserts.into_iter().rev() {
            func.blocks[block].body.insert(index, instr);
        }
    }
}

/// The body index an insertion lands at.
fn insertion_index(func: &ArcFunction, slot: PlanSlot) -> usize {
    match slot {
        PlanSlot::BlockFront { .. } => 0,
        PlanSlot::BeforeBody { index, .. } => index,
        PlanSlot::AfterBody { index, .. } => index + 1,
        PlanSlot::BeforeTerminator { block } => func
            .blocks
            .get(block)
            .map_or(0, |arc_block| arc_block.body.len()),
    }
}

/// The burden op a plan entry materializes to.
fn materialize(op: &PlannedOp) -> ArcInstr {
    match &op.kind {
        PlannedOpKind::Inc => ArcInstr::BurdenInc { var: op.var },
        PlannedOpKind::Dec => ArcInstr::BurdenDec { var: op.var },
        PlannedOpKind::DecPartial { skip_fields } => ArcInstr::BurdenDecPartial {
            var: op.var,
            skip_fields: skip_fields.clone(),
        },
    }
}
