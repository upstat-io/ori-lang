//! Per-instruction USE classification for the lineage model —
//! RC-class op accounting, member/view value-reads, `[own]` hand-offs, and
//! the unmodeled-use decline boundary.
//!
//! Spec: Annex E §AIMS RL-2 + RL-1 + TF-11.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::model::{
    BlockReads, DefKind, FreshSiteInc, LineageModel, DEC_EVENT, INC_EVENT, OWN_ARG_EVENT,
    READ_EVENT,
};
use super::views::ViewChain;

/// RC-class op accounting for one body instruction: member ops count `±1`
/// (a fresh-site member inc is a removal candidate), SAME-ALLOC view ops
/// count `±1` in the unified ledger, OPAQUE view ops are excluded (their
/// per-block balance is pre-validated), partial / variant / field-grain ops
/// on a member or same-alloc view decline. Returns `None` to decline the
/// lineage; `Some(true)` when `instr` was an RC-class op; `Some(false)`
/// when it was not (the caller models it as a value use).
fn model_rc_op_use(
    is_member: &dyn Fn(ArcVarId) -> bool,
    views: &ViewChain,
    instr: &ArcInstr,
    b: usize,
    i: usize,
    def_kind: &FxHashMap<ArcVarId, DefKind>,
    model: &mut LineageModel,
) -> Option<bool> {
    match instr {
        ArcInstr::BurdenInc { var } | ArcInstr::RcInc { var, .. } => {
            if is_member(*var) {
                if def_kind.get(var) == Some(&DefKind::FreshSite) {
                    model.fresh_site_incs.push(FreshSiteInc {
                        block: b,
                        instr_idx: i,
                        event_idx: model.events[b].body.len(),
                    });
                }
                model.events[b].body.push(INC_EVENT);
            } else if views.same_alloc.contains(var) {
                model.events[b].body.push(INC_EVENT);
            }
            Some(true)
        }
        ArcInstr::BurdenDec { var } | ArcInstr::RcDec { var, .. } => {
            // The lone niche-payload release IS the lineage's release.
            if is_member(*var) || views.same_alloc.contains(var) {
                model.events[b].body.push(DEC_EVENT);
            }
            Some(true)
        }
        ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::BurdenDecVariant { var }
        | ArcInstr::RcDecPartial { var, .. }
        | ArcInstr::RcDecVariant { var } => {
            // Whole-var member grain only; a partial / variant dec on a
            // SAME-ALLOC view releases an unproven subset — unmodeled.
            if is_member(*var) {
                model.events[b].body.push(DEC_EVENT);
                Some(true)
            } else if views.same_alloc.contains(var) {
                None
            } else {
                Some(true)
            }
        }
        ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => {
            // Field-grain dec on a member or same-alloc view — outside the
            // whole-var model (an opaque view field-grain dec already failed
            // `view_ops_balanced`).
            if is_member(*base) || views.same_alloc.contains(base) {
                None
            } else {
                Some(true)
            }
        }
        _ => Some(false),
    }
}

/// Model one body instruction's member + view USES (whitelist; RC-class ops
/// are counted via [`model_rc_op_use`], NOT value-reads — TF-11:
/// "RcInc/RcDec { var } -> none (RC operation, not a use)"). `None` =
/// unmodeled use (decline the lineage).
#[expect(
    clippy::too_many_arguments,
    reason = "single use-classification seam threading the lineage-model \
              build context; bundling into a struct fragments the one-walk \
              shape"
)]
pub(super) fn model_body_uses(
    contracts: &FxHashMap<Name, MemoryContract>,
    is_member: &dyn Fn(ArcVarId) -> bool,
    views: &ViewChain,
    instr: &ArcInstr,
    b: usize,
    i: usize,
    def_kind: &FxHashMap<ArcVarId, DefKind>,
    model: &mut LineageModel,
) -> Option<()> {
    if model_rc_op_use(is_member, views, instr, b, i, def_kind, model)? {
        return Some(());
    }
    match instr {
        ArcInstr::Let {
            value: ArcValue::Var(_) | ArcValue::Literal(_),
            ..
        } => Some(()),
        ArcInstr::Let {
            value: ArcValue::PrimOp { args, .. },
            ..
        } => {
            if args.iter().any(|&a| is_member(a) || views.contains(a)) {
                None
            } else {
                Some(())
            }
        }
        ArcInstr::Project { value, .. } => {
            if is_member(*value) {
                model.events[b].body.push(READ_EVENT);
                record_body_read(&mut model.read_blocks, b, *value);
            } else if views.contains(*value) {
                model.events[b].body.push(READ_EVENT);
                model.read_blocks.entry(b).or_default().last_body_view = Some(*value);
            }
            Some(())
        }
        ArcInstr::Apply {
            args, func: callee, ..
        } => {
            // Why: Reads check aliveness before the own-arg hand-offs apply.
            let mut own_args = 0usize;
            for (pos, &arg) in args.iter().enumerate() {
                let member = is_member(arg);
                let view = views.contains(arg);
                if !member && !view {
                    continue;
                }
                if instr.is_owned_position(pos) {
                    // Why: An owned VIEW hand-off escapes the model.
                    if view {
                        return None;
                    }
                    own_args += 1;
                } else {
                    if callee_iter_consumes(contracts, *callee, pos) {
                        return None;
                    }
                    model.events[b].body.push(READ_EVENT);
                    if member {
                        record_body_read(&mut model.read_blocks, b, arg);
                    } else {
                        model.read_blocks.entry(b).or_default().last_body_view = Some(arg);
                    }
                }
            }
            for _ in 0..own_args {
                model.events[b].body.push(OWN_ARG_EVENT);
            }
            Some(())
        }
        // Why: Member moves, closures, reuse, COW mutations, resets,
        // and shared-checks are outside the modeled vocabulary.
        _ => {
            if instr
                .used_vars()
                .iter()
                .any(|&v| is_member(v) || views.contains(v))
            {
                None
            } else {
                Some(())
            }
        }
    }
}

/// Model the block terminator's member + view USES. `None` = unmodeled
/// (decline).
pub(super) fn model_terminator_uses(
    contracts: &FxHashMap<Name, MemoryContract>,
    is_member: &dyn Fn(ArcVarId) -> bool,
    views: &ViewChain,
    terminator: &ArcTerminator,
    b: usize,
    model: &mut LineageModel,
) -> Option<()> {
    match terminator {
        // Why: A returned member/view is an RL-2 transfer to the caller.
        ArcTerminator::Return { value } => {
            if is_member(*value) || views.contains(*value) {
                None
            } else {
                Some(())
            }
        }
        ArcTerminator::Invoke {
            args, func: callee, ..
        } => {
            let mut own_args = 0usize;
            for (pos, &arg) in args.iter().enumerate() {
                let member = is_member(arg);
                let view = views.contains(arg);
                if !member && !view {
                    continue;
                }
                if terminator.is_owned_position(pos) {
                    if view {
                        return None;
                    }
                    own_args += 1;
                } else {
                    if callee_iter_consumes(contracts, *callee, pos) {
                        return None;
                    }
                    model.events[b].term.push(READ_EVENT);
                    let entry = model.read_blocks.entry(b).or_default();
                    if member {
                        entry.terminator = Some(arg);
                    } else {
                        entry.view_terminator = true;
                    }
                }
            }
            for _ in 0..own_args {
                model.events[b].term.push(OWN_ARG_EVENT);
            }
            Some(())
        }
        // Why: Jump args are positional renames, not value-reads; a threaded VIEW escapes.
        ArcTerminator::Jump { args, .. } => {
            if args.iter().any(|&a| views.contains(a)) {
                None
            } else {
                Some(())
            }
        }
        ArcTerminator::Resume | ArcTerminator::Unreachable => Some(()),
        // Why: Branch/Switch operands are scalars, and InvokeIndirect uses are unmodeled.
        _ => {
            if terminator
                .used_vars()
                .iter()
                .any(|&v| is_member(v) || views.contains(v))
            {
                None
            } else {
                Some(())
            }
        }
    }
}

fn record_body_read(read_blocks: &mut FxHashMap<usize, BlockReads>, b: usize, var: ArcVarId) {
    read_blocks.entry(b).or_default().last_body = Some(var);
}

/// Whether `callee`'s contract marks param `pos` as iter-consuming (RL-2
/// inward transfer through a borrowed position — unmodeled here).
fn callee_iter_consumes(
    contracts: &FxHashMap<Name, MemoryContract>,
    callee: Name,
    pos: usize,
) -> bool {
    contracts
        .get(&callee)
        .and_then(|c| c.params.get(pos))
        .is_some_and(|p| p.iter_consumes)
}
