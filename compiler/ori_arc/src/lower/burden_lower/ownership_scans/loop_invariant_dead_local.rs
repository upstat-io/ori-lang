//! RL-5 release for a purely-dead loop-invariant fresh-collection local.
//!
//! `let root = [1]; let xs = []; for i in 0..N do { xs = xs.push(i) }; xs[k]`
//! constructs `root` (a fresh `[int]` buffer), threads it UNCHANGED through the
//! loop's block-params, and NEVER reads it — `root` is a dead loop-invariant
//! local. The loop back-edge fractures the union-find lineage
//! (`dead_param_single_feeding_rep` resolves the dead loop-exit param to the
//! loop-header param's rep, NOT the `Construct` rep), so
//! `compute_construct_fed_dead_param_lineage`'s fresh-collection gate declines
//! it and no RL-5 release is emitted -> the buffer leaks (alloc +1, all burden
//! ops are balanced keep-alive pairs, no terminal release).
//!
//! This self-contained scan recognizes the shape directly (decoupled from the
//! keystone `compute_genuine_same_alloc_reps` per the reverted broad-union
//! dead-end) and emits ONE RL-5 dead-at-entry `BurdenDec` at the lineage's
//! terminal dead block-param. Per `AimsProof.Realization::RL5_dead_at_entry_cleanup`
//! the dead Owned non-scalar param entered with a live RC=1 reference that is
//! never used; `RL5_cleanup_balanced` proves `[inc, dec]` nets 0.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};

/// `ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE=1` declines the RL-5 release
/// for a purely-dead loop-invariant fresh-collection local (the bisection
/// surface; default off -> the release is emitted). Spec: Annex E §AIMS RL-5.
fn disabled() -> bool {
    std::env::var("ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE").as_deref() == Ok("1")
}

/// Per-block RL-5 releases for purely-dead loop-invariant fresh-collection
/// locals: `block_idx -> [terminal dead block-param var, ...]`. Each gets ONE
/// `BurdenDec` at the block's entry (the existing dead-param emission surface).
///
/// A lineage is admitted iff ALL hold:
/// - ROOT is a fresh `Construct` `ListLiteral`/`MapLiteral`/`SetLiteral` whose
///   dst carries RC (an `RcPointer` buffer);
/// - every member's EVERY use is a `Jump`-arg feeding another member (a
///   block-param) — i.e. the lineage is threaded ONLY, never read as a value
///   (no body-instr operand, no `Branch`/`Return`/`Invoke`/`Apply`/non-Jump
///   terminator operand). This is the purely-dead discriminator: a value
///   actually read is NOT this family (it has a real last-use the base walk
///   releases);
/// - the lineage reaches EXACTLY ONE terminal block-param `P` (a member
///   block-param never threaded forward as a `Jump`-arg) that is `Owned`
///   non-scalar (RC-carrying).
pub(in crate::lower::burden_lower) fn compute_loop_invariant_dead_local_releases(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<usize, Vec<ArcVarId>> {
    let mut out: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    if disabled() {
        return out;
    }

    // Fresh-collection Construct roots whose dst carries RC.
    let roots: Vec<ArcVarId> = func
        .blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter_map(|instr| match instr {
            ArcInstr::Construct {
                dst,
                ctor: CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral,
                ..
            } if owned_vars_needing_rc.contains(dst) => Some(*dst),
            _ => None,
        })
        .collect();
    if roots.is_empty() {
        return out;
    }

    // Block-param -> (block_idx, position) index, for resolving Jump-arg targets.
    let mut param_loc: FxHashMap<ArcVarId, (usize, usize)> = FxHashMap::default();
    for (bi, block) in func.blocks.iter().enumerate() {
        for (pos, &(p, _)) in block.params.iter().enumerate() {
            param_loc.insert(p, (bi, pos));
        }
    }

    for root in roots {
        if let Some((block_idx, param)) =
            admit_lineage(func, root, &param_loc, owned_vars_needing_rc)
        {
            let entry = out.entry(block_idx).or_default();
            if !entry.contains(&param) {
                entry.push(param);
            }
        }
    }
    out
}

/// Grow the lineage from `root` by following `Jump`-arg -> block-param edges,
/// verifying every member is threaded ONLY (purely dead). Returns the single
/// terminal dead block-param `(block_idx, param)` when the shape is admitted.
fn admit_lineage(
    func: &ArcFunction,
    root: ArcVarId,
    param_loc: &FxHashMap<ArcVarId, (usize, usize)>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Option<(usize, ArcVarId)> {
    let members = grow_lineage(func, root);
    if !lineage_is_purely_dead(func, &members) {
        return None;
    }
    find_sole_terminal(func, &members, param_loc, owned_vars_needing_rc)
}

/// Grow the member set from `root` over `Jump`-arg -> block-param edges (the
/// purely-structural threading closure; includes loop back-edges).
fn grow_lineage(func: &ArcFunction, root: ArcVarId) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    let mut work: Vec<ArcVarId> = vec![root];
    while let Some(v) = work.pop() {
        for block in &func.blocks {
            let ArcTerminator::Jump { target, args } = &block.terminator else {
                continue;
            };
            for (pos, &arg) in args.iter().enumerate() {
                if arg == v {
                    if let Some(&(p, _)) = func.blocks[target.index()].params.get(pos) {
                        if members.insert(p) {
                            work.push(p);
                        }
                    }
                }
            }
        }
    }
    members
}

/// True iff EVERY use of every member is a `Jump`-arg feeding a member
/// block-param — the lineage is threaded ONLY, never read as a value. Any
/// body-instr operand (excluding balanced keep-alive `BurdenInc`/`BurdenDec`),
/// any non-Jump terminator operand, or any Jump-arg feeding a NON-member
/// disqualifies it (a real read with its own last-use the base walk releases).
fn lineage_is_purely_dead(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if instr_reads_member(instr, members) {
                return false;
            }
        }
        match &block.terminator {
            ArcTerminator::Jump { target, args } => {
                for (pos, &arg) in args.iter().enumerate() {
                    if !members.contains(&arg) {
                        continue;
                    }
                    let feeds_member = func.blocks[target.index()]
                        .params
                        .get(pos)
                        .is_some_and(|&(p, _)| members.contains(&p));
                    if !feeds_member {
                        return false;
                    }
                }
            }
            other => {
                if other.used_vars().iter().any(|v| members.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}

/// The lineage's SOLE terminal dead block-param: a member block-param never
/// threaded forward as a `Jump`-arg, `Owned` non-scalar (RC-carrying). `None`
/// when there is no such param OR more than one (a fork needs >1 release —
/// out of this single-release family). The purely-dead check guarantees a
/// terminal member is read nowhere, so its sole incoming reference is the
/// predecessor Jump-arg handoff (`Cardinality = Absent` at entry) — the RL-5
/// release point.
fn find_sole_terminal(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    param_loc: &FxHashMap<ArcVarId, (usize, usize)>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Option<(usize, ArcVarId)> {
    let threaded_forward: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .filter_map(|b| match &b.terminator {
            ArcTerminator::Jump { args, .. } => Some(args),
            _ => None,
        })
        .flatten()
        .copied()
        .filter(|v| members.contains(v))
        .collect();
    let mut terminal: Option<(usize, ArcVarId)> = None;
    for &m in members {
        let Some(&(bi, _)) = param_loc.get(&m) else {
            continue; // not a block-param (the root Construct dst)
        };
        if threaded_forward.contains(&m) {
            continue;
        }
        if !owned_vars_needing_rc.contains(&m) {
            return None;
        }
        if terminal.is_some() {
            return None;
        }
        terminal = Some((bi, m));
    }
    terminal
}

/// True iff `instr` READS a lineage member as a value operand. A keep-alive
/// `BurdenInc`/`BurdenDec` on a member is a balanced no-op pair (the alloc's
/// surplus is the leak this scan releases), NOT a value read — excluded.
fn instr_reads_member(instr: &ArcInstr, members: &FxHashSet<ArcVarId>) -> bool {
    if matches!(
        instr,
        ArcInstr::BurdenInc { .. } | ArcInstr::BurdenDec { .. }
    ) {
        return false;
    }
    // A `Let { Var(src) }` alias of a member would extend the lineage but is not
    // a read; however the purely-dead family threads ONLY through block-params,
    // so a member appearing as any body operand (including a Let-alias source)
    // is outside this family -> treat as a read (decline conservatively).
    instr.used_vars().iter().any(|v| members.contains(v))
}
