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
//! Per the RL-5 calculus (Lean mirror `AimsProof.Realization::RL5_dead_at_entry_cleanup`,
//! `.proof` sibling `aims-proof/proofs/08-realization/RL-5.proof`) an Owned non-scalar
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

mod vetting;
use vetting::{closure_lineage_vetted, collection_lineage_vetted};

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

/// Shared (a)-(f) RL-5 loop-carried dead-param skeleton. The closure scan and the
/// collection scan differ ONLY in `roots`, `expected_repr`, `grow`, and `vet`;
/// every admission gate below is identical.
///
/// Admission gates (ALL hold per root; ANY failure declines the root and keeps
/// current behavior — the status-quo leak/double-free is the migration floor,
/// never a regression introduced here):
///  (a) FRESH root: supplied by `roots` (a `PartialApply` closure dst, or an
///      `ori_list_take` for-yield finalizer dst).
///  (b) root in `owned_vars_needing_rc` (heap-carrying, RC-tracked) AND its repr
///      equals `expected_repr` (`FatValue` closure / `RcPointer` collection).
///  (c) vetted lineage: `grow` builds the same-alloc lineage; `vet` confirms
///      every use of every member is a benign borrow-or-threading appearance —
///      any owned-consume / store / capture / projection / escape declines.
///  (d) loop-carried: at least one member is a block-param fed by a BACK-edge
///      (the loop carry — excludes the non-loop single-call shape the base walk
///      already balances).
///  (e) EXACTLY ONE dead post-loop member block-param (`function_used_vars` miss,
///      non-scalar repr) fed by a FORWARD single-predecessor edge — the lineage
///      is ONE allocation owing ONE release; zero or >1 declines.
///  (f) pairwise-DISJOINT lineages: two admitted roots sharing any member
///      converge on one allocation web; each would place its own release,
///      double-freeing it. ALL roots of an overlapping group decline.
fn compute_dead_param_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    roots: &[ArcVarId],
    expected_repr: ValueRepr,
    grow: &dyn Fn(&ArcFunction, ArcVarId) -> FxHashSet<ArcVarId>,
    vet: &dyn Fn(&ArcFunction, &FxHashSet<ArcVarId>) -> bool,
) -> LoopCarriedDeadParam {
    let mut out = LoopCarriedDeadParam {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    let mut candidates: Vec<Candidate> = Vec::new();

    for &root in roots {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "loop-carried dead-param root declined"
            );
        };
        // Gate (b): heap-carrying + expected repr.
        if !owned_vars_needing_rc.contains(&root) || func.var_repr(root) != Some(expected_repr) {
            decline("b:owned/repr");
            continue;
        }
        // Gate (c): grow the same-alloc lineage, then vet.
        let members = grow(func, root);
        if !vet(func, &members) {
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
                "loop-carried dead-param root declined"
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
            "loop-carried dead-param release placed (RL-5)"
        );
    }
    out
}

/// RL-5 dead-at-entry treatment for a loop-carried `PartialApply` closure dead at
/// the post-loop block-param — the `FatValue` closure binding of the shared
/// [`compute_dead_param_lineage`] skeleton (empty `cow_names`, so the lineage is
/// `Let`-Var aliases + `Jump`-arg threading only; the borrow-only vet rejects any
/// escaped/stored/captured closure shape the base walk already balances).
pub(in crate::lower::burden_lower) fn compute_loop_closure_dead_param_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> LoopCarriedDeadParam {
    let no_cow: FxHashSet<ori_ir::Name> = FxHashSet::default();
    compute_dead_param_lineage(
        func,
        owned_vars_needing_rc,
        &collect_partial_apply_roots(func),
        ValueRepr::FatValue,
        &|f, root| grow_lineage(f, root, &no_cow),
        &|f, members| closure_lineage_vetted(f, members),
    )
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
/// (Lean mirror `AimsProof.Realization::{RL5_dead_at_entry_cleanup, RL5_cleanup_balanced}`,
/// `.proof` sibling `aims-proof/proofs/08-realization/RL-5.proof`) + RL-2.
pub(in crate::lower::burden_lower) fn compute_loop_carried_dead_collection_param_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &StringInterner,
) -> LoopCarriedDeadParam {
    let list_take = interner.intern("ori_list_take");
    let cow_names = crate::borrow::all_cow_method_names(interner);
    compute_dead_param_lineage(
        func,
        owned_vars_needing_rc,
        &collect_list_take_roots(func, list_take),
        ValueRepr::RcPointer,
        &|f, root| grow_lineage(f, root, &cow_names),
        &|f, members| collection_lineage_vetted(f, members, &cow_names),
    )
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

/// Gate (c) growth (shared, COW-aware): grow the single-owned-reference lineage
/// from `root` over `Let { Var }` aliases, `Jump`-arg → block-param threading,
/// AND — when `cow_names` is non-empty — the COW-mutator-result edge. An EMPTY
/// `cow_names` (the closure scan) yields the `Let`-Var + `Jump` lineage only (the
/// COW arms never match); a populated `cow_names` (the collection scan) ALSO
/// threads a member consumed at the OWNED RECEIVER (pos 0) of a COW method
/// (`updated`/`sort`/… in `cow_names`) into its result, whether the runtime
/// mutates in place (unique) or copies-and-releases-old (shared). One owned
/// reference flows through the whole chain (the `doors[idx] = ..` COW
/// reassignment loop), so the chain is ONE release lineage. The COW-mutator may
/// be an `Apply` (no-unwind) or an `Invoke` terminator (the result is the Invoke
/// dst on the normal successor).
fn grow_lineage(
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

#[cfg(test)]
mod tests;
