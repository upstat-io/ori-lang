//! Gate (d) closure-vetting for the borrowed-`Invoke` lineage scan: grow the
//! same-alloc closure from a root + vet every member use as a pure borrow-read.
//! Split from `borrowed_invoke_lineage.rs` for the 500-line cap.

use rustc_hash::FxHashSet;

use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

/// Gate (d): grow the same-alloc closure from `root` (Let-Var aliases +
/// `Jump`-arg → block-param hops), then vet every member use as a pure
/// borrow-read / borrowed-`Invoke` arg / length `Project`. `None` on any vet
/// failure.
///
/// The closure does NOT follow non-scalar `Project` views into the buffer's
/// ELEMENTS (unlike `live_extract.rs`, whose niche-payload `Project` IS the
/// same allocation): a collection-buffer element extracted by `__index` is the
/// `Invoke` RESULT (a distinct allocation the result-lineage owns), not a
/// same-alloc view of the buffer. Following it would over-suppress the element's
/// own release. The length `Project` (`Project xs.0 : int`, scalar) is a
/// borrow-read that drops out of the closure (scalar tag).
///
/// The closure grows into a `Jump`-target block-param via the Jump-arg hop,
/// then a FIXPOINT validation pass rejects any joined block-param that is a phi
/// merging DISTINCT allocations: every predecessor of the param's block must
/// reach it via a `Jump` whose arg at that position is itself a closure member
/// (so the param is the SAME allocation on every incoming edge). The check runs
/// at fixpoint (not mid-growth) so a LOOP back-edge — whose loop-carried arg is
/// a member only after earlier iterations grow it — validates correctly; a phi
/// fed by a member on one arm and a foreign allocation on a sibling arm fails.
/// On any failed param the WHOLE closure declines (`None`) — the conservative
/// status-quo leak, never the `04B.2-cross-class-uaf` foreign-merge bug
/// (wrong suppression / a release on a merged-in foreign allocation).
pub(in crate::lower::burden_lower) fn same_alloc_closure_vetted(
    func: &ArcFunction,
    root: ArcVarId,
) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    // Block-params joined via a Jump-arg hop, to validate at fixpoint.
    let mut joined_params: Vec<(usize, usize)> = Vec::new();
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if members.contains(src) && members.insert(*dst) {
                        grew = true;
                    }
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                for (pos, &arg) in args.iter().enumerate() {
                    if !members.contains(&arg) {
                        continue;
                    }
                    let Some(&(param, _)) = func.blocks[target_idx].params.get(pos) else {
                        continue;
                    };
                    if members.insert(param) {
                        grew = true;
                        joined_params.push((target_idx, pos));
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    // Fixpoint phi-merge validation: a joined block-param whose predecessors do
    // not ALL pass a member at its position merges a foreign allocation.
    let preds = compute_predecessors(func);
    for &(target, pos) in &joined_params {
        if !all_preds_pass_member(func, &preds, target, pos, &members) {
            return None;
        }
    }

    member_uses_all_borrow_reads(func, &members).then_some(members)
}

/// True iff EVERY predecessor of `target` reaches it via a `Jump` whose arg at
/// `pos` is a closure member — so the merged block-param at `pos` is the SAME
/// allocation on every incoming edge. Evaluated at fixpoint (`members` complete),
/// so a loop back-edge whose carried arg is a member validates; a predecessor
/// reaching `target` via a non-`Jump` edge, or whose `Jump` arg at `pos` is not
/// a member, fails (the param could carry a different allocation on that edge).
fn all_preds_pass_member(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    target: usize,
    pos: usize,
    members: &FxHashSet<ArcVarId>,
) -> bool {
    let Some(pred_blocks) = preds.get(target) else {
        return false;
    };
    if pred_blocks.is_empty() {
        return false;
    }
    pred_blocks.iter().all(|&p| {
        matches!(
            &func.blocks[p].terminator,
            ArcTerminator::Jump { target: t, args }
                if t.index() == target
                    && args.get(pos).is_some_and(|a| members.contains(a))
        )
    })
}

/// Gate (d) vetting core: true iff EVERY use of every closure member is a pure
/// borrow-read — a length / element `Project` of a member (TF-4 Borrowed), a
/// borrowed call arg (`Apply` / `Invoke` non-owned position), or a closure-own
/// hop (`Let { Var }` re-bind, `Jump` arg). ANY owned-position consume / store /
/// capture / COW-machinery use / escape declines the whole closure (the
/// gate list — a double-free is FAR worse than the status-quo leak).
pub(super) fn member_uses_all_borrow_reads(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches_member = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches_member {
                continue;
            }
            match instr {
                // Alias hops + element / length `Project` borrow-views are the
                // closure's own borrow-read edges.
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. } => {}
                // COW / conditional-alias / mutation / reuse machinery on a
                // member, a closure capture (`PartialApply` retains a
                // reference), or an indirect call (`ApplyIndirect` has no
                // contract to vet) are distinct sub-roots — decline.
                ArcInstr::Select { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. }
                | ArcInstr::PartialApply { .. }
                | ArcInstr::ApplyIndirect { .. } => return false,
                // A body `Apply` (e.g. `len` / a user borrowed-arg call) may
                // read a member ONLY at a borrowed position; an owned-position
                // consume transfers the buffer out of family.
                ArcInstr::Apply { .. } => {
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                }
                // Owned-position consume at any other instruction = transfer; a
                // list-concat `PrimOp Binary(Add)` consumes its `RcPointer`
                // operands (the dual-consuming runtime contract) — decline.
                _ => {
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                    if super::super::list_concat_consumed_operands(instr, func)
                        .iter()
                        .any(|v| members.contains(v))
                    {
                        return false;
                    }
                }
            }
        }
        let term = &block.terminator;
        let term_touches = term.used_vars().iter().any(|v| members.contains(v));
        if !term_touches {
            continue;
        }
        match term {
            // Jump hops are the closure's own param edges; Resume / Unreachable
            // use nothing of the closure; Branch / Switch read a scalar — all
            // are borrow-read-compatible no-ops.
            ArcTerminator::Jump { .. }
            | ArcTerminator::Resume
            | ArcTerminator::Unreachable
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. } => {}
            // A borrowed `Invoke` / `InvokeIndirect` arg (the carrier) is a
            // borrow-read; an owned-position consume transfers out — decline.
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
            }
            // A `Return` of a member transfers the buffer out (the caller owns
            // the release) — decline.
            ArcTerminator::Return { .. } => return false,
        }
    }
    true
}
