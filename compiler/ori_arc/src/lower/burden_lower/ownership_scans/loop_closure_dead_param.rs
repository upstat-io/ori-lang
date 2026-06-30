//! RL-5 dead-at-entry release for a fresh `PartialApply` closure threaded
//! through a loop and dead at the post-loop block-param (annex-e §AIMS RL-5 +
//! RL-2 + RL-4).
//!
//! Shape: `let f = (n) -> ...; for i in xs do { ... f(i) ... }` — a FRESH
//! `PartialApply` closure (TF-7: Owned, Unique, `BlockLocal`, `NonReusable`, an
//! allocation with RC = 1 at birth) defined BEFORE a loop, threaded through the
//! loop's block-params as a loop-carried value (root → block-param via every
//! `Jump` arg, including the back-edge), BORROW-used inside the loop body — an
//! `ApplyIndirect` borrow-receiver (the direct-call shape) OR a BORROWED
//! `Apply`/`Invoke` call arg to a callee whose param contract borrows it (the
//! `apply(f: scale, x: i)` HOF shape) — and DEAD at the post-loop merge
//! block-param (`Cardinality = Absent`, used nowhere beyond its binding).
//!
//! Per `AimsProof.Realization::RL5_dead_at_entry_cleanup` an Owned non-scalar
//! `Absent` block-param owes EXACTLY ONE immediate dec; the fresh closure's RC=1
//! reference enters that param and is never used, so the param is the lineage's
//! sole release point. The base Phase-5 walk emits none → leak (the int/str
//! capture HOF shape, no spurious dec) OR emits a spurious per-iteration
//! borrow-receiver `BurdenDec` on the loop-carried alias → double-free (the
//! direct-`ApplyIndirect` FM shape: `[inc, dec, dec]` per iteration = net −1,
//! freeing the loop-invariant closure on iteration 1).
//!
//! The cure removes the whole same-alloc closure lineage from
//! `owned_vars_needing_rc` (suppressing the spurious per-iteration ops + the
//! net-0 keep-alive pairs) and places EXACTLY ONE whole-var `BurdenDec` at the
//! dead post-loop block-param entry (`RL5_cleanup_balanced`: the closure's
//! single birth reference is released exactly once — net 0).

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::StringInterner;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::function_used_vars;
use super::successor_reachable_blocks;
use super::ForwarderReleasePos;

/// Result of [`compute_loop_closure_dead_param_lineage`]: the same-alloc closure
/// lineage to suppress (kills every per-iteration inc/dec the base walk emits)
/// plus the single RL-5 dead-at-entry release.
pub(in crate::lower::burden_lower) struct LoopCarriedDeadParam {
    /// Every var in an admitted loop-carried closure lineage — removed from
    /// `owned_vars_needing_rc` so the only release is the placed dec below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, BlockEntry) -> [dead param var]` — one whole-var `BurdenDec`
    /// per admitted lineage at the post-loop dead block-param. Merged into the
    /// `forwarder_result_releases` emission surface.
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// One admitted lineage's facts, held until the pairwise-disjoint gate runs.
struct Candidate {
    members: FxHashSet<ArcVarId>,
    release_block: usize,
    release_var: ArcVarId,
}

/// RL-5 dead-at-entry treatment for a loop-carried `PartialApply` closure dead at
/// the post-loop block-param.
///
/// Admission gates (ALL hold per closure; ANY failure declines the root and
/// keeps current behavior — the status-quo leak/double-free is the migration
/// floor, never a regression introduced here):
///  (a) FRESH closure root: a `PartialApply` body dst (TF-7 fresh closure).
///  (b) root in `owned_vars_needing_rc` (heap-carrying, RC-tracked) AND
///      `FatValue` repr (closure two-word value).
///  (c) vetted borrow-only lineage ([`closure_lineage_vetted`]): grow over
///      `Let { Var }` aliases AND `Jump`-arg → block-param threading; EVERY use
///      of every member is an alias-edge, an `ApplyIndirect` BORROW-receiver
///      (pos 0), a BORROWED `Apply`/`Invoke` call arg (`!is_owned_position`), or
///      a `Jump`-arg threading edge — any owned-consume position, `Set`/`SetTag`,
///      `Construct`/`Reuse`/`PartialApply` capture, `Project`, `Return`, or
///      `Branch`/`Switch`/`Resume`/`Unreachable` operand declines (the
///      escaped/stored/captured closure is a DIFFERENT shape — a lazy-iterator
///      capture is an owned `PartialApply`/`Construct` arg and declines here).
///  (d) loop-carried: at least one member is a block-param fed by a BACK-edge
///      (the loop carry — excludes the non-loop single-call shape, which the
///      base walk already balances).
///  (e) EXACTLY ONE dead post-loop member block-param (`function_used_vars`
///      miss, non-scalar repr) fed by a FORWARD single-predecessor edge — the
///      lineage is ONE allocation owing ONE release; zero or >1 dead params
///      declines (a fork the single dec cannot cover).
///  (f) pairwise-DISJOINT lineages: two admitted roots sharing any member
///      converge on one allocation web; each would place its own release,
///      double-freeing the web. ALL roots of an overlapping group decline.
pub(in crate::lower::burden_lower) fn compute_loop_closure_dead_param_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> LoopCarriedDeadParam {
    let mut out = LoopCarriedDeadParam {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    let mut candidates: Vec<Candidate> = Vec::new();

    for root in collect_partial_apply_roots(func) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "loop-closure dead-param root declined"
            );
        };
        // Gate (b): heap-carrying + FatValue closure repr.
        if !owned_vars_needing_rc.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::FatValue))
        {
            decline("b:owned/repr");
            continue;
        }
        // Gate (c): grow the same-alloc lineage, then vet borrow-only use.
        let members = grow_same_alloc_lineage(func, root);
        if !closure_lineage_vetted(func, &members) {
            decline("c:vet");
            continue;
        }
        // Gate (d): loop-carried (a member block-param fed by a back-edge).
        if !is_loop_carried(func, &members) {
            decline("d:not-loop-carried");
            continue;
        }
        // Gate (e): exactly one dead post-loop member block-param.
        let Some((release_block, release_var)) = find_dead_post_loop_param(func, &members, &used)
        else {
            decline("e:no-unique-dead-param");
            continue;
        };
        candidates.push(Candidate {
            members,
            release_block,
            release_var,
        });
    }

    // Gate (f): decline EVERY candidate whose lineage overlaps another's.
    let overlapping: Vec<bool> = candidates
        .iter()
        .map(|c| {
            candidates
                .iter()
                .filter(|o| !std::ptr::eq(*o, c))
                .any(|o| !c.members.is_disjoint(&o.members))
        })
        .collect();
    for (cand, overlaps) in candidates.into_iter().zip(overlapping) {
        if overlaps {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                gate = "f:lineage-overlap",
                "loop-closure dead-param root declined"
            );
            continue;
        }
        out.suppressed_lineage_vars
            .extend(cand.members.iter().copied());
        out.releases
            .entry((cand.release_block, ForwarderReleasePos::BlockEntry))
            .or_default()
            .push(cand.release_var);
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            release_block = cand.release_block,
            release_var = cand.release_var.index(),
            "loop-closure dead-param release placed (RL-5)"
        );
    }
    out
}

/// Gate (a): candidate roots — every `PartialApply` body dst (a freshly-minted
/// closure allocation, TF-7).
fn collect_partial_apply_roots(func: &ArcFunction) -> Vec<ArcVarId> {
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::PartialApply { dst, .. } = instr {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Grow the same-alloc lineage from `root` over `Let { Var }` aliases AND
/// `Jump`-arg → block-param threading (the loop-carry + loop-exit edges). Every
/// member names ONE allocation: a `Let`-Var alias and a Jump-arg handoff are
/// RC-identity-preserving (no birth, no inc). Root-agnostic — shared by the
/// loop-closure (gate c) and the loop-carried-dead-collection scans.
fn grow_same_alloc_lineage(func: &ArcFunction, root: ArcVarId) -> FxHashSet<ArcVarId> {
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
                if let Some(target_block) = func.blocks.get(target.index()) {
                    for (i, &arg) in args.iter().enumerate() {
                        if members.contains(&arg) {
                            if let Some(&(param, _)) = target_block.params.get(i) {
                                if members.insert(param) {
                                    grew = true;
                                }
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
    members
}

/// Gate (c) vetting core: true iff EVERY use of every member is a benign
/// borrow-or-threading appearance. Any owned-consume / store / capture /
/// projection / escape declines.
fn closure_lineage_vetted(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches {
                continue;
            }
            match instr {
                // Alias-edge: the lineage's own internal hop.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // Borrow-receiver read: the member MUST be the closure receiver
                // (pos 0, always borrowed) and MUST NOT appear as an owned arg.
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    if !members.contains(closure) {
                        return false;
                    }
                    if args.iter().any(|a| members.contains(a)) {
                        return false;
                    }
                }
                // Apply: a member may appear ONLY at a BORROWED arg position
                // (the callee borrows it). Any owned position declines.
                ArcInstr::Apply { args, .. } => {
                    if args
                        .iter()
                        .enumerate()
                        .any(|(i, a)| members.contains(a) && instr.is_owned_position(i))
                    {
                        return false;
                    }
                }
                // Any other instruction touching a member is an owned consume /
                // store / capture / projection — decline.
                _ => return false,
            }
        }
        // Terminator uses.
        let term = &block.terminator;
        match term {
            // Jump-arg threading: members feed successor block-params (grown
            // into the lineage). Benign.
            ArcTerminator::Jump { .. } => {}
            // Invoke: a member may appear ONLY at a BORROWED arg position.
            ArcTerminator::Invoke { args, .. } => {
                if args
                    .iter()
                    .enumerate()
                    .any(|(i, a)| members.contains(a) && term.is_owned_position(i))
                {
                    return false;
                }
            }
            // InvokeIndirect: the called closure (pos 0) borrows; member args
            // at owned positions decline.
            ArcTerminator::InvokeIndirect { closure, args, .. } => {
                if args.iter().any(|a| members.contains(a)) {
                    // arg positions in used_vars are offset by 1 (closure at 0).
                    let owned_arg = args
                        .iter()
                        .enumerate()
                        .any(|(i, a)| members.contains(a) && term.is_owned_position(i + 1));
                    if owned_arg {
                        return false;
                    }
                }
                let _ = closure;
            }
            // Return / Branch / Switch / Resume / Unreachable carrying a member
            // is an escape or a non-closure operand — decline.
            other => {
                if other.used_vars().iter().any(|v| members.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Gate (d): true iff some member is a block-param fed by a BACK-edge — the loop
/// carry. A back-edge `B -> P` exists when `P`'s block is reachable from `B`'s
/// successors (so the edge closes a cycle).
fn is_loop_carried(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for (bp, block) in func.blocks.iter().enumerate() {
        let has_member_param = block.params.iter().any(|&(p, _)| members.contains(&p));
        if !has_member_param {
            continue;
        }
        // Find a predecessor Jump feeding this block whose target is reachable
        // back from `bp` (a back-edge into `bp`).
        for (b, pred) in func.blocks.iter().enumerate() {
            if let ArcTerminator::Jump { target, args } = &pred.terminator {
                if target.index() == bp && args.iter().any(|a| members.contains(a)) {
                    // Edge b -> bp. Back-edge iff bp can reach b.
                    if successor_reachable_blocks(func, bp).contains(&b) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Gate (e): the UNIQUE dead post-loop member block-param fed by a FORWARD
/// single-predecessor edge. `None` when zero or more than one qualify.
fn find_dead_post_loop_param(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    used: &FxHashSet<ArcVarId>,
) -> Option<(usize, ArcVarId)> {
    let mut found: Option<(usize, ArcVarId)> = None;
    for (bp, block) in func.blocks.iter().enumerate() {
        for &(param, _) in &block.params {
            if !members.contains(&param) || used.contains(&param) {
                continue; // not a member, or live (not dead)
            }
            if super::super::is_provably_scalar_repr(func, param) {
                continue;
            }
            // Feeding edges into this param's position must be FORWARD and
            // single-predecessor.
            let pos = block.params.iter().position(|&(p, _)| p == param)?;
            let mut feeding_preds: Vec<usize> = Vec::new();
            for (b, pred) in func.blocks.iter().enumerate() {
                if let ArcTerminator::Jump { target, args } = &pred.terminator {
                    if target.index() == bp && args.get(pos).is_some() {
                        feeding_preds.push(b);
                    }
                }
            }
            // Single predecessor.
            let [pred] = feeding_preds.as_slice() else {
                continue;
            };
            // FORWARD edge: the dead-param block must NOT reach its predecessor
            // (a back-edge would make this the loop carry, not the exit).
            if successor_reachable_blocks(func, bp).contains(pred) {
                continue;
            }
            // The dead-param block must not itself sit in a cycle (a re-reached
            // release double-frees).
            if successor_reachable_blocks(func, bp).contains(&bp) {
                continue;
            }
            // Uniqueness: exactly one dead post-loop param across the lineage.
            if found.is_some() {
                return None;
            }
            found = Some((bp, param));
        }
    }
    found
}

/// RL-5 dead-at-entry release for a fresh `ori_list_take` (for-yield collect)
/// collection threaded through a loop and dead at the post-loop block-param — the
/// collection analog of [`compute_loop_closure_dead_param_lineage`] (BUG-04-238).
///
/// Shape: `let a = for _ in 0..n yield e; for i in 0..m yield f` — the first
/// for-yield's `ori_list_take` result `a` (dead) is threaded as a loop-carried
/// block-param through the SECOND for-yield loop and arrives dead at its post-loop
/// param, with only net-0 keep-alive pairs and no release -> a `+1` leak. The
/// clean single-loop / literal-return shapes never thread `a` (no second loop) so
/// they are released directly and unaffected.
///
/// Admission gates mirror the closure scan with the root + repr changed:
///  (a) FRESH root: an `ori_list_take` result dst (the for-yield finalizer, a
///      fresh self-alloc list at rc=1).
///  (b) root in `owned_vars_needing_rc` AND `RcPointer` repr (vs the closure's
///      `FatValue`).
///  (c) vetted borrow-only-or-dead lineage ([`collection_lineage_vetted`]).
///  (d) loop-carried (a member block-param fed by a back-edge).
///  (e) EXACTLY ONE dead post-loop member block-param.
///  (f) pairwise-DISJOINT lineages.
///
/// Cure: remove the whole same-alloc lineage from `owned_vars_needing_rc`
/// (suppress the net-0 keep-alive pairs) and place EXACTLY ONE whole-var
/// `BurdenDec` at the dead post-loop block-param entry (`RL5_cleanup_balanced` —
/// the single birth reference released once, net 0). Spec: Annex E §AIMS RL-5
/// (`RL5_dead_at_entry_cleanup` + `RL5_cleanup_balanced`) + RL-2.
pub(in crate::lower::burden_lower) fn compute_loop_carried_dead_collection_param_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &StringInterner,
) -> LoopCarriedDeadParam {
    let mut out = LoopCarriedDeadParam {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    let list_take = interner.intern("ori_list_take");
    let cow_names = crate::borrow::all_cow_method_names(interner);
    let mut candidates: Vec<Candidate> = Vec::new();

    for root in collect_list_take_roots(func, list_take) {
        let decline = |gate: &str, n: usize| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                members = n,
                "loop-carried dead-collection-param root declined"
            );
        };
        // Gate (b): heap-carrying + RcPointer collection repr.
        if !owned_vars_needing_rc.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::RcPointer))
        {
            decline("b:owned/repr", 0);
            continue;
        }
        // Gate (c): grow the (COW-aware) ownership lineage, then vet
        // borrow-only-or-dead use. The COW-mutator-result edge (a member consumed
        // at the owned receiver of `updated`/`sort`/… and re-bound as the result)
        // threads the single owned reference through the `doors[idx] = ..` COW
        // reassignment chain so the dead-at-end collection is reached.
        let members = grow_collection_lineage(func, root, &cow_names);
        if !collection_lineage_vetted(func, &members, &cow_names) {
            decline("c:vet", members.len());
            continue;
        }
        // Gate (d): loop-carried (a member block-param fed by a back-edge).
        if !is_loop_carried(func, &members) {
            decline("d:not-loop-carried", members.len());
            continue;
        }
        // Gate (e): exactly one dead post-loop member block-param.
        let Some((release_block, release_var)) = find_dead_post_loop_param(func, &members, &used)
        else {
            decline("e:no-unique-dead-param", members.len());
            continue;
        };
        candidates.push(Candidate {
            members,
            release_block,
            release_var,
        });
    }

    // Gate (f): decline EVERY candidate whose lineage overlaps another's.
    let overlapping: Vec<bool> = candidates
        .iter()
        .map(|c| {
            candidates
                .iter()
                .filter(|o| !std::ptr::eq(*o, c))
                .any(|o| !c.members.is_disjoint(&o.members))
        })
        .collect();
    for (cand, overlaps) in candidates.into_iter().zip(overlapping) {
        if overlaps {
            continue;
        }
        out.suppressed_lineage_vars
            .extend(cand.members.iter().copied());
        out.releases
            .entry((cand.release_block, ForwarderReleasePos::BlockEntry))
            .or_default()
            .push(cand.release_var);
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            release_block = cand.release_block,
            release_var = cand.release_var.index(),
            "loop-carried dead-collection-param release placed (RL-5)"
        );
    }
    out
}

/// Gate (a) for the collection scan: candidate roots — every `ori_list_take`
/// result dst (the for-yield finalizer, a fresh self-alloc list at rc=1).
fn collect_list_take_roots(func: &ArcFunction, list_take: ori_ir::Name) -> Vec<ArcVarId> {
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == list_take {
                    roots.push(*dst);
                }
            }
        }
    }
    roots
}

/// Gate (c) growth (COW-aware): grow the single-owned-reference lineage from
/// `root` over `Let { Var }` aliases, `Jump`-arg → block-param threading, AND the
/// COW-mutator-result edge — a member consumed at the OWNED RECEIVER (pos 0) of a
/// COW method (`updated`/`sort`/… in `cow_names`) re-binds its single owned
/// reference into the result, whether the runtime mutates in place (unique) or
/// copies-and-releases-old (shared). One owned reference flows through the whole
/// chain (the `doors[idx] = ..` COW reassignment loop), so the chain is ONE
/// release lineage. The COW-mutator may be an `Apply` (no-unwind) or an `Invoke`
/// terminator (the result is the Invoke dst on the normal successor).
fn grow_collection_lineage(
    func: &ArcFunction,
    root: ArcVarId,
    cow_names: &FxHashSet<ori_ir::Name>,
) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) => {
                        grew |= members.insert(*dst);
                    }
                    ArcInstr::Apply {
                        dst,
                        func: callee,
                        args,
                        ..
                    } if cow_names.contains(callee)
                        && args.first().is_some_and(|a| members.contains(a)) =>
                    {
                        grew |= members.insert(*dst);
                    }
                    _ => {}
                }
            }
            match &block.terminator {
                ArcTerminator::Jump { target, args } => {
                    if let Some(target_block) = func.blocks.get(target.index()) {
                        for (i, &arg) in args.iter().enumerate() {
                            if members.contains(&arg) {
                                if let Some(&(param, _)) = target_block.params.get(i) {
                                    grew |= members.insert(param);
                                }
                            }
                        }
                    }
                }
                ArcTerminator::Invoke {
                    dst,
                    func: callee,
                    args,
                    ..
                } if cow_names.contains(callee)
                    && args.first().is_some_and(|a| members.contains(a)) =>
                {
                    grew |= members.insert(*dst);
                }
                _ => {}
            }
        }
        if !grew {
            break;
        }
    }
    members
}

/// Gate (c) vetting for a collection lineage: true iff EVERY use of every member
/// is a benign borrow-or-threading appearance (an alias-edge, a BORROWED
/// `Apply`/`Invoke` arg, a `Jump`-arg threading edge, or the OWNED RECEIVER (pos
/// 0) of a COW-mutator whose result re-joins the lineage) OR the member is never
/// used (a wholly-dead thread). A member at any NON-receiver owned position (a
/// value moved INTO a collection, a non-COW owned consume, a store / capture /
/// projection / return) declines — a transferred or escaped lineage is a
/// different shape the base walk already balances.
fn collection_lineage_vetted(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    cow_names: &FxHashSet<ori_ir::Name>,
) -> bool {
    // A member appears at an owned position OTHER than a COW-mutator's receiver
    // (pos 0) — an escape / value-into-collection / non-COW consume.
    let owned_escape =
        |callee: &ori_ir::Name, args: &[ArcVarId], is_owned: &dyn Fn(usize) -> bool| -> bool {
            let is_cow = cow_names.contains(callee);
            args.iter()
                .enumerate()
                .any(|(i, a)| members.contains(a) && is_owned(i) && !(is_cow && i == 0))
        };
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().iter().any(|v| members.contains(v)) {
                continue;
            }
            match instr {
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // A SCALAR-producing `Project` of a member is a benign field read
                // (the collection's length / capacity field, e.g. for index-bounds
                // math) — a borrow-view that carries no RC. A NON-scalar projection
                // (an owned sub-collection / element extract that could alias)
                // declines conservatively.
                ArcInstr::Project { dst, value, .. }
                    if members.contains(value)
                        && super::super::is_provably_scalar_repr(func, *dst) => {}
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    if owned_escape(callee, args, &|i| instr.is_owned_position(i)) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        match &block.terminator {
            ArcTerminator::Jump { .. } => {}
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => {
                let term = &block.terminator;
                if owned_escape(callee, args, &|i| term.is_owned_position(i)) {
                    return false;
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
