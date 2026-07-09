//! Member-field-extract seed collection + the cross-block liveness check
//! consumed by both the NO-SINK and CARRIER-SUCC death-point arms: a
//! same-alloc `Project` extracted from a lineage member is a distinct
//! allocation the result-lineage owns, and a release at the carrier's edge
//! must decline when such an extract is still live elsewhere. Spec: Annex E
//! §AIMS RL-2.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

/// Seed set: every `Project` dst whose SOURCE is a lineage member (a
/// same-alloc view extracted from the closure). Members themselves are
/// excluded — they are the borrowed receiver, not an extract. SSOT for the
/// member-field-extract seed computation — consumed by [`has_live_project_extract`]
/// (grows the seed transitively + checks cross-block liveness) and by the
/// parent module's unconditional STRUCT-root decline (gate s1), which only
/// needs non-emptiness.
pub(in crate::lower::burden_lower::ownership_scans::borrowed_invoke_lineage) fn collect_member_field_extract_seeds(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut extract: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if members.contains(value) && !members.contains(dst) {
                    extract.insert(*dst);
                }
            }
        }
    }
    extract
}

/// True iff a same-allocation `Project` extracted from a lineage member is LIVE
/// across the carrier's release edges — read in a block OTHER than the carrier's
/// own block. The extract's transitive flow (`Let { Var }` aliases + `Jump`-arg →
/// block-param hops) is grown FIRST (the same discipline `same_alloc_closure_vetted`
/// applies to members), then any extract-closure member used in a non-carrier
/// block declines the no-sink claim — an edge release would double-free the
/// buffer the extract still holds.
pub(super) fn has_live_project_extract(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    carrier_block: usize,
) -> bool {
    let mut extract = collect_member_field_extract_seeds(func, members);
    if extract.is_empty() {
        return false;
    }
    // Grow the extract's transitive aliases: `Let { Var(src) }` re-binds,
    // `Project`-of-extract chains (an extract of an extract still views the
    // same allocation tree), and Jump-arg → block-param hops. A Jump arg that
    // is an extract member flows into the target block-param (the live-across
    // receiver of the extracted buffer).
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
                    if extract.contains(src) && !members.contains(dst) && extract.insert(*dst) {
                        grew = true;
                    }
                }
                if let ArcInstr::Project { dst, value, .. } = instr {
                    if extract.contains(value) && !members.contains(dst) && extract.insert(*dst) {
                        grew = true;
                    }
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                for (pos, &arg) in args.iter().enumerate() {
                    if !extract.contains(&arg) {
                        continue;
                    }
                    if let Some(&(param, _)) = func.blocks[target_idx].params.get(pos) {
                        if !members.contains(&param) && extract.insert(param) {
                            grew = true;
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    // Any extract member used in a block OTHER than the carrier's own block is
    // live across the carrier's release edges.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        if block_idx == carrier_block {
            continue;
        }
        let used_here = block
            .body
            .iter()
            .any(|i| i.used_vars().iter().any(|v| extract.contains(v)))
            || block
                .terminator
                .used_vars()
                .iter()
                .any(|v| extract.contains(v));
        if used_here {
            return true;
        }
    }
    false
}
