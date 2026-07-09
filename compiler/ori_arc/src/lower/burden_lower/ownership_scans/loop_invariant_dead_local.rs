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
//! keystone `compute_genuine_same_alloc_reps`) and emits ONE RL-5 dead-at-entry
//! `BurdenDec` at the lineage's terminal dead block-param. Per
//! `AimsProof.Realization::RL5_dead_at_entry_cleanup`
//! the dead Owned non-scalar param entered with a live RC=1 reference that is
//! never used; `RL5_cleanup_balanced` proves `[inc, dec]` nets 0.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

/// `ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE=1` declines the RL-5 release
/// for a purely-dead loop-invariant fresh-collection local (the bisection
/// surface; default off -> the release is emitted). Spec: Annex E §AIMS RL-5.
// Env: ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE — declines the purely-dead RL-5 release for bisection, debug-only
fn disabled() -> bool {
    std::env::var("ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE").as_deref() == Ok("1")
}

/// `ORI_DISABLE_LOOP_INVARIANT_BORROW_ONLY_RELEASE=1` declines the RL-5 release
/// for the BORROW-ONLY-READ loop-invariant fresh-collection local (the
/// borrow-read sibling of the purely-dead family; default off -> the release is
/// emitted). Spec: Annex E §AIMS RL-5.
// Env: ORI_DISABLE_LOOP_INVARIANT_BORROW_ONLY_RELEASE — declines the borrow-only-read RL-5 release for bisection, debug-only
fn borrow_only_disabled() -> bool {
    std::env::var("ORI_DISABLE_LOOP_INVARIANT_BORROW_ONLY_RELEASE").as_deref() == Ok("1")
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
            ArcInstr::Construct { dst, ctor, .. }
                if ctor.is_collection_literal() && owned_vars_needing_rc.contains(dst) =>
            {
                Some(*dst)
            }
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
        // Purely-dead family (threaded only, never read): the original RL-5
        // recovery. Borrow-only-read family (loop-borrowed via `[borrow]` reads,
        // then dead at the terminal exit param): the sibling recovery. The two
        // are disjoint by their read discriminator; the contains-gate below
        // keeps the merge idempotent if both ever match one root.
        let admitted = admit_lineage(func, root, &param_loc, owned_vars_needing_rc).or_else(|| {
            if borrow_only_disabled() {
                None
            } else {
                admit_borrow_only_lineage(func, root, &param_loc, owned_vars_needing_rc)
            }
        });
        if let Some((block_idx, param)) = admitted {
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

/// Admit the BORROW-ONLY-READ loop-invariant variant: a fresh-collection
/// `Construct` threaded UNCHANGED through loop block-params whose ONLY non-thread
/// uses are `[borrow]`-position `Apply`/`Invoke` args (and their `Let`-Var
/// aliases), reaching EXACTLY ONE terminal dead block-param. Shape: a map/list
/// borrow-read each iteration (`let v = m[k]` lowers to `@__index(m [borrow], ..)`
/// off a `Let`-alias of the threaded param), dead at the loop-normal-exit param.
/// The loop back-edge fractures the union-find lineage so the base walk emits no
/// scope-exit release on the normal path (only the unwind-edge cleanup decs),
/// leaking the buffer. Per `RL5_dead_at_entry_cleanup` + `RL5_cleanup_balanced`
/// the terminal Owned non-scalar Absent param entered with a live RC=1 reference
/// never consumed; ONE RL-5 release balances it. Spec: Annex E §AIMS RL-5.
fn admit_borrow_only_lineage(
    func: &ArcFunction,
    root: ArcVarId,
    param_loc: &FxHashMap<ArcVarId, (usize, usize)>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Option<(usize, ArcVarId)> {
    let members = grow_lineage_with_aliases(func, root);
    // CLOSURE gate (the loop-INVARIANT discriminator): every member block-param's
    // EVERY incoming `Jump`-arg MUST itself be a member. A reassigned loop local
    // (`s = [i,i+1]; s = s.push(7)`) feeds a FRESH per-iteration value back into
    // the loop slot via the back-edge — that feeder is a non-member, so the slot
    // is NOT a single allocation threaded unchanged. Only a value constructed ONCE
    // (the sole root) and threaded by its own self-aliases satisfies RL-5's
    // dead-at-entry single-reference premise; a reassigned slot carries a fresh
    // allocation per iteration whose release the base walk already owns.
    if !lineage_is_closed_under_feeders(func, &members, param_loc) {
        return None;
    }
    if !lineage_is_borrow_only_dead(func, &members) {
        return None;
    }
    find_sole_terminal(func, &members, param_loc, owned_vars_needing_rc)
}

/// True iff every member that is a block-param has ALL its incoming `Jump`-args
/// (across every predecessor block, every back-edge) be members. A non-member
/// feeder means a fresh-per-iteration value is reassigned into the loop slot —
/// the slot is NOT loop-invariant, so the dead-at-entry single-reference premise
/// of RL-5 fails and the base walk already releases each fresh value.
fn lineage_is_closed_under_feeders(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    param_loc: &FxHashMap<ArcVarId, (usize, usize)>,
) -> bool {
    for block in &func.blocks {
        let ArcTerminator::Jump { target, args } = &block.terminator else {
            continue;
        };
        let target_params = &func.blocks[target.index()].params;
        for (pos, &arg) in args.iter().enumerate() {
            let Some(&(tp, _)) = target_params.get(pos) else {
                continue;
            };
            // This Jump-arg feeds a member block-param at `pos`.
            if members.contains(&tp) && param_loc.contains_key(&tp) && !members.contains(&arg) {
                return false;
            }
        }
    }
    true
}

/// Grow the member set from `root` over BOTH `Jump`-arg -> block-param edges
/// (the loop threading, including back-edges) AND `Let { value: Var(member) }`
/// alias edges (the borrow-read view's binding). A borrow-read `let v = m[k]`
/// binds `%alias = Let Var(%threaded)` then reads `@__index(%alias [borrow])`;
/// `%alias` joins the lineage so its borrow-read is vetted by
/// [`lineage_is_borrow_only_dead`].
fn grow_lineage_with_aliases(func: &ArcFunction, root: ArcVarId) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    let mut work: Vec<ArcVarId> = vec![root];
    while let Some(v) = work.pop() {
        // Jump-arg -> block-param threading (loop edges).
        for block in &func.blocks {
            if let ArcTerminator::Jump { target, args } = &block.terminator {
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
        // Let { Var(member) } alias edges (the borrow-read view's binding).
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if *src == v && members.insert(*dst) {
                        work.push(*dst);
                    }
                }
            }
        }
    }
    members
}

/// True iff EVERY use of every member is one of: a `Jump`-arg feeding a member
/// block-param (the loop threading), a `Let { Var(member) }` alias edge (an
/// internal lineage hop), a BORROWED-position `Apply`/`ApplyIndirect`/`Invoke`
/// arg (the read-only borrow), or a balanced `BurdenInc`/`BurdenDec` keep-alive
/// pair. ANY owned-position consume / store / `Project` / `Construct`-arg /
/// `Set` / `Select` / non-thread terminator operand / `Jump`-arg feeding a
/// NON-member declines (a real transfer-out with its own release the base walk
/// emits). Mirrors the borrow-only vetting in `closure_extract_borrow_view`,
/// extended to permit the loop's member-threading `Jump`-args.
fn lineage_is_borrow_only_dead(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    let mut saw_borrow_read = false;
    for block in &func.blocks {
        for instr in &block.body {
            let touches = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches {
                continue;
            }
            match instr {
                // Balanced keep-alive pair on a member: a no-op, not a read.
                ArcInstr::BurdenInc { .. } | ArcInstr::BurdenDec { .. } => {}
                // Alias-edge: the lineage's own internal hop.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // Borrowed call: EVERY member arg MUST be at a BORROWED position.
                ArcInstr::Apply { args, .. } | ArcInstr::ApplyIndirect { args, .. } => {
                    for (pos, a) in args.iter().enumerate() {
                        if members.contains(a) {
                            if instr.is_owned_position(pos) {
                                return false;
                            }
                            saw_borrow_read = true;
                        }
                    }
                }
                // Any other instruction touching a member is an owned consume /
                // store / capture / projection / construct-arg — decline.
                _ => return false,
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
            // A member at a BORROWED `Invoke` terminator arg is a borrow-read
            // (the may-unwind `@__index` / `@unwrap` lowering); any owned arg
            // position or any other terminator operand is an escape — decline.
            ArcTerminator::Invoke { args, .. } | ArcTerminator::InvokeIndirect { args, .. } => {
                let term = &block.terminator;
                for (pos, &arg) in args.iter().enumerate() {
                    if members.contains(&arg) {
                        if term.is_owned_position(pos) {
                            return false;
                        }
                        saw_borrow_read = true;
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
    // At least one genuine borrow-read distinguishes this from the purely-dead
    // family (which the first admission path already owns).
    saw_borrow_read
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

#[cfg(test)]
mod tests;
