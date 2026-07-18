//! Plan materialization: insert planned burden ops into a function's
//! instruction stream. Production consumer: the per-function replacement
//! path (`replace::attempt_replacement`).

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};
use crate::ownership::Ownership;

use super::emit::{PlanSlot, PlannedOp, PlannedOpKind};

/// Insert every planned op at its slot. Insertions within one block apply
/// back-to-front so earlier indices stay valid. Ops sharing an insertion
/// point apply Inc BEFORE Dec (fund-before-release: a container release and
/// a same-slot funding inc of its extracted view name one allocation, and
/// releasing first frees the memory the inc then touches); within one kind,
/// plan order holds.
pub(crate) fn apply_plan(func: &mut ArcFunction, ops: &[PlannedOp]) {
    let (ops, borrowed_credit_aliases) = remap_borrowed_credit_releases(func, ops);
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
    if !borrowed_credit_aliases.is_empty() {
        let aliases = borrowed_credit_aliases
            .into_iter()
            .map(|(source, alias)| ArcInstr::Let {
                dst: alias,
                ty: func.var_type(source),
                value: ArcValue::Var(source),
            })
            .collect::<Vec<_>>();
        func.blocks[func.entry.index()].body.splice(0..0, aliases);
    }
}

/// A borrowed ABI parameter can nevertheless carry a callee-owned credit
/// when its memory contract requests an incoming whole-value credit. The
/// planner may need to release that credit before any source alias dominates
/// (notably on an early unwind edge). Keep the ABI annotation Borrowed, but
/// materialize one entry alias for every such release subject so the current
/// RC carrier never targets the borrowed parameter directly.
fn remap_borrowed_credit_releases(
    func: &mut ArcFunction,
    ops: &[PlannedOp],
) -> (Vec<PlannedOp>, Vec<(ArcVarId, ArcVarId)>) {
    #[expect(
        clippy::needless_collect,
        reason = "the collect releases the immutable borrow of func.params before \
                  func.fresh_var_like's mutable borrow below; a single chained \
                  iterator does not borrow-check (E0500)"
    )]
    let borrowed_release_params = func
        .params
        .iter()
        .filter(|param| {
            param.ownership == Ownership::Borrowed
                && ops.iter().any(|op| {
                    op.var == param.var
                        && matches!(
                            &op.kind,
                            PlannedOpKind::Dec | PlannedOpKind::DecPartial { .. }
                        )
                })
        })
        .map(|param| param.var)
        .collect::<Vec<_>>();
    let aliases = borrowed_release_params
        .into_iter()
        .map(|source| (source, func.fresh_var_like(source)))
        .collect::<Vec<_>>();
    let remapped = ops
        .iter()
        .cloned()
        .map(|mut op| {
            if matches!(
                &op.kind,
                PlannedOpKind::Dec | PlannedOpKind::DecPartial { .. }
            ) {
                if let Some((_, alias)) = aliases.iter().find(|(source, _)| *source == op.var) {
                    op.var = *alias;
                }
            }
            op
        })
        .collect();
    (remapped, aliases)
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
