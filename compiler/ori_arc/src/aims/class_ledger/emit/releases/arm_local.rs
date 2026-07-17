//! Arm-local release pairing for extraction-funded class members.

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::ArcFunction;

use super::super::super::events::{successors_of, ClassEvents};
use super::super::{PlanSlot, PlannedOp, PlannedOpKind};

/// Pair multi-arm extraction seeds whose reads stay within their own arm.
pub(in super::super::super) fn pair_arm_local_seed_releases(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    events: &ClassEvents,
    ops: &mut Vec<PlannedOp>,
) {
    let seed_blocks: FxHashSet<usize> = ops
        .iter()
        .filter(|op| op.kind == PlannedOpKind::Inc)
        .map(|op| op.slot.block())
        .collect();
    if seed_blocks.len() < 2 {
        return;
    }
    let seeds: Vec<PlannedOp> = ops
        .iter()
        .filter(|op| op.kind == PlannedOpKind::Inc)
        .cloned()
        .collect();
    for seed in seeds {
        let block = seed.slot.block();
        let closure =
            super::super::close_over_let_aliases(func, std::iter::once(seed.var).collect());
        let mut last_body: Option<usize> = None;
        let mut terminator_read = false;
        let mut arm_local = true;
        let mut any_read = false;
        'scan: for (event_block, evs) in events.per_block.iter().enumerate() {
            for ev in evs {
                let Some(var) = ev.var else { continue };
                if !closure.contains(&var) {
                    continue;
                }
                if event_block != block || ev.delta != 0 || ev.floor == 0 {
                    arm_local = false;
                    break 'scan;
                }
                any_read = true;
                match ev.site {
                    EventSite::Body(index) => {
                        last_body = Some(last_body.map_or(index, |previous| previous.max(index)));
                    }
                    EventSite::Terminator => terminator_read = true,
                    EventSite::BlockEntry => {
                        arm_local = false;
                        break 'scan;
                    }
                }
            }
        }
        if !arm_local || !any_read {
            continue;
        }
        if terminator_read {
            let successors = successors_of(func, block);
            if successors.is_empty()
                || successors.iter().any(|&successor| {
                    successor == block
                        || preds
                            .get(successor)
                            .is_none_or(|predecessors| predecessors.len() != 1)
                })
            {
                continue;
            }
            for successor in successors {
                ops.push(PlannedOp {
                    slot: PlanSlot::BlockFront { block: successor },
                    kind: PlannedOpKind::Dec,
                    var: seed.var,
                });
            }
        } else if let Some(index) = last_body {
            ops.push(PlannedOp {
                slot: PlanSlot::AfterBody { block, index },
                kind: PlannedOpKind::Dec,
                var: seed.var,
            });
        }
    }
}
