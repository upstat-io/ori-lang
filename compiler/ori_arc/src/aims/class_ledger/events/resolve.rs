//! Member-variable resolution for one class event: mapping an event's
//! recorded site back to the specific `ArcVarId` naming the class member.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use crate::aims::intraprocedural::ledger_events::{ClassInstr, EventSite};
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

/// Resolve the member variable an event names. Reads and mutates carry it;
/// every other kind resolves through the source instruction's variables
/// (operands first for a consume — the handed-off reference — destination
/// first otherwise).
pub(super) fn resolve_event_var(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
    block: usize,
    site: EventSite,
    instr: &ClassInstr,
) -> Option<ArcVarId> {
    if let ClassInstr::Read { value, .. }
    | ClassInstr::Mutate { value, .. }
    | ClassInstr::SelectCredit { var: value, .. } = *instr
    {
        return Some(value);
    }
    let consume = matches!(instr, ClassInstr::Consume { .. });
    let candidates = match site {
        EventSite::BlockEntry => block_entry_candidates(func, block),
        EventSite::Body(index) => body_candidates(func, block, index, consume),
        EventSite::Terminator => terminator_candidates(func, block, instr),
    };
    candidates
        .into_iter()
        .find(|&var| is_member(partition, var, class))
}

/// Whether `var`'s whole-variable node belongs to `class`.
fn is_member(partition: &mut BirthSitePartition, var: ArcVarId, class: NodeIdx) -> bool {
    let node = partition.register_node(var, FieldPath::whole_var());
    partition.rep_of(node) == class
}

/// Candidate vars at a block-entry site: function params (entry block),
/// this block's params, and every `Invoke`/`InvokeIndirect` result
/// materialized at this block's entry (its normal successor).
fn block_entry_candidates(func: &ArcFunction, block: usize) -> Vec<ArcVarId> {
    let mut candidates: Vec<ArcVarId> = Vec::new();
    if block == func.entry.index() {
        candidates.extend(func.params.iter().map(|p| p.var));
    }
    if let Some(arc_block) = func.blocks.get(block) {
        candidates.extend(arc_block.params.iter().map(|&(param, _)| param));
    }
    for pred_block in &func.blocks {
        if let ArcTerminator::Invoke { dst, normal, .. }
        | ArcTerminator::InvokeIndirect { dst, normal, .. } = &pred_block.terminator
        {
            if normal.index() == block {
                candidates.push(*dst);
            }
        }
    }
    candidates
}

/// Candidate vars at a body site.
fn body_candidates(func: &ArcFunction, block: usize, index: usize, consume: bool) -> Vec<ArcVarId> {
    let Some(instr) = func
        .blocks
        .get(block)
        .and_then(|arc_block| arc_block.body.get(index))
    else {
        return Vec::new();
    };
    let defined: Vec<ArcVarId> = instr.defined_var().into_iter().collect();
    let used: Vec<ArcVarId> = instr.used_vars().into_iter().collect();
    if consume {
        used.into_iter().chain(defined).collect()
    } else {
        defined.into_iter().chain(used).collect()
    }
}

/// Candidate vars at a terminator site. A cross-class Jump CREDIT names the
/// target block's param; an Invoke result birth/credit names the
/// destination; everything else names the terminator's own operand vars.
fn terminator_candidates(func: &ArcFunction, block: usize, instr: &ClassInstr) -> Vec<ArcVarId> {
    let Some(arc_block) = func.blocks.get(block) else {
        return Vec::new();
    };
    let terminator = &arc_block.terminator;
    let mut candidates: Vec<ArcVarId> = Vec::new();
    if matches!(instr, ClassInstr::Credit { .. } | ClassInstr::Birth { .. }) {
        match terminator {
            ArcTerminator::Jump { target, .. } => {
                if let Some(target_block) = func.blocks.get(target.index()) {
                    candidates.extend(target_block.params.iter().map(|&(param, _)| param));
                }
            }
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                candidates.push(*dst);
            }
            _ => {}
        }
    }
    candidates.extend(terminator.used_vars());
    candidates
}
