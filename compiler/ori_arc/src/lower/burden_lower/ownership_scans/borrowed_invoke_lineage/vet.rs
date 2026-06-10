//! Gate (d) closure-vetting for the borrowed-`Invoke` lineage scan: grow the
//! same-alloc closure from a root + vet every member use as a pure borrow-read.
//! Split from `borrowed_invoke_lineage.rs` for the 500-line cap.

use rustc_hash::FxHashSet;

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
pub(super) fn same_alloc_closure_vetted(
    func: &ArcFunction,
    root: ArcVarId,
) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
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
                for (pos, &arg) in args.iter().enumerate() {
                    if members.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            if members.insert(param) {
                                grew = true;
                            }
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    member_uses_all_borrow_reads(func, &members).then_some(members)
}

/// Gate (d) vetting core: true iff EVERY use of every closure member is a pure
/// borrow-read — a length / element `Project` of a member (TF-4 Borrowed), a
/// borrowed call arg (`Apply` / `Invoke` non-owned position), or a closure-own
/// hop (`Let { Var }` re-bind, `Jump` arg). ANY owned-position consume / store /
/// capture / COW-machinery use / escape declines the whole closure (the codex
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
