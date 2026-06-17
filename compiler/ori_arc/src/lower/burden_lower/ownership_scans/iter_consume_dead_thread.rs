//! RL-1 + RL-2 orphaned-inc elision for a fresh collection iter-consumed by an
//! inline `for`-loop whose loop-carried thread is dead post-loop (annex-e §AIMS
//! RL-1 + RL-2; BUG-04-155 back-edge-aware net).
//!
//! Shape: `let xs = [..]; let ys = for x in xs.iter() yield ...` — a fresh
//! `Construct` collection `xs` (rc=1) whose SOLE genuine consume is the inline
//! for-loop's `@iter [own]` (an `Invoke @iter` TERMINATOR, or an `Apply @iter`,
//! whose `ori_iter_drop` frees the buffer INSIDE the loop — `ApplyToIterConsumingParam`
//! transfer, no caller dec per `RL2_iter_consuming_no_caller_dec`). The for-loop
//! lowering ALSO Jump-arg-threads `xs` across the loop back-edge through a chain
//! of post-loop branch-split block-params that terminate in a DEAD param (never
//! genuinely read — only re-threaded). The base walk's use-count counts that
//! dead-thread as a SECOND live use → classifies `xs` as duplicated → emits an
//! orphaned FRESH-site keep-alive `BurdenInc` whose paired scope-exit dec is RL-2
//! transfer-suppressed by the `@iter [own]` consume → +1 leak (exit 2).
//!
//! The cure: the dead-thread is a `JumpArg` transfer (RL-4 exemption) into a dead
//! param — NOT a genuine duplication. So `xs` is move-once into `@iter`; the
//! FRESH-site inc is ELIDABLE (`RL1_duplication_balanced` move-once: lifecycle
//! `[]`, net 0). Remove the whole same-alloc dead-thread lineage (the fresh
//! `Construct` root + `Let`-Var aliases + Jump-arg-threaded DEAD block-params)
//! from `owned_vars_needing_rc` — eliding the orphan inc + the symmetric per-thread
//! pairs. The `@iter`'s `ori_iter_drop` is the single release of the rc=1 buffer.
//!
//! Foreclosed-and-avoided (dead-ends #163/#164/#180/#181/#185): a FORWARD-only
//! threaded-rep scan SEPARATES the loop-carried thread from the `@iter` rep
//! (the thread crosses the back-edge) — this scan follows Jump-args across ALL
//! edges (forward AND back) via `compute_param_edge_args`. It does NOT relax the
//! `@iter` COW-taint (#181's over-fire surface) — it is a pure inc-elision of a
//! proven-dead-thread lineage, touching no COW / release surface.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, ValueRepr};

/// RL-1 + RL-2 dead-thread orphaned-inc elision. Returns the same-alloc lineage
/// vars to remove from `owned_vars_needing_rc` (eliding the orphan FRESH-site inc
/// + the symmetric per-thread pairs). Empty when no admitted root.
///
/// Admission gates (ALL hold per root; ANY failure declines — the status-quo leak
/// is the migration floor, never a regression introduced here):
///  (a) FRESH collection `Construct` root (`ListLiteral`/`MapLiteral`/`SetLiteral`)
///      with `RcPointer` repr, in `owned_vars_needing_rc`.
///  (b) the root (or a `Let`-Var alias of it) is consumed at an `@iter [own]`
///      arg — `Apply @iter` OR `Invoke @iter` terminator, OR a user-callee
///      `iter_consumes` param (the inline for-loop's iterator owns + frees).
///  (c) the FULL same-alloc lineage (root + `Let`-Var aliases + Jump-arg-threaded
///      block-params across ALL edges, forward + back) is DEAD-THREADED: every
///      block-param member is used ONLY as a Jump-arg re-thread (never read at an
///      `Apply`/`Invoke`/`Construct`/`Project`/`Set`/`Branch`/`Switch`/`Return`
///      operand). A genuine post-loop read of any member declines (a surviving
///      use needs the keep-alive — the `str_list_explicit_last_owner` shape).
///  (d) the lineage's ONLY genuine (non-alias, non-thread) consume is the
///      `@iter [own]` (no owned store / capture / second call / return). Any
///      other owned-position consume declines.
///  (e) pairwise-DISJOINT lineages.
pub(in crate::lower::burden_lower) fn compute_iter_consume_dead_thread_orphan_inc(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_consumed = collect_iter_consumed_vars(func, contracts, interner);
    if iter_consumed.is_empty() {
        return FxHashSet::default();
    }
    let param_edge_args =
        crate::aims::intraprocedural::project_aliases::compute_param_edge_args(func);

    let mut suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut admitted_lineages: Vec<FxHashSet<ArcVarId>> = Vec::new();

    for root in collect_fresh_collection_roots(func, interner) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "iter-consume dead-thread orphan-inc root declined"
            );
        };
        // Gate (a): RC-tracked fresh collection root.
        if !owned_vars_needing_rc.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::RcPointer))
        {
            decline("a:owned/repr");
            continue;
        }
        // Grow the full same-alloc lineage (Let-Var aliases + Jump-arg threading
        // across ALL edges, forward + back).
        let lineage = grow_thread_lineage(func, root, &param_edge_args);
        // Gate (b): a lineage member is iter-consumed.
        if !lineage.iter().any(|v| iter_consumed.contains(v)) {
            decline("b:not-iter-consumed");
            continue;
        }
        // Gate (c): every block-param member is dead-threaded (never genuinely
        // read), and (d) the lineage's only genuine consume is the @iter [own].
        if !dead_thread_vetted(func, &lineage, &iter_consumed) {
            decline("cd:not-dead-threaded");
            continue;
        }
        admitted_lineages.push(lineage);
    }

    // Gate (e): decline EVERY lineage overlapping another's.
    let overlapping: Vec<bool> = admitted_lineages
        .iter()
        .map(|l| {
            admitted_lineages
                .iter()
                .filter(|o| !std::ptr::eq(*o, l))
                .any(|o| !l.is_disjoint(o))
        })
        .collect();
    for (lineage, overlaps) in admitted_lineages.into_iter().zip(overlapping) {
        if overlaps {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                gate = "e:lineage-overlap",
                "iter-consume dead-thread orphan-inc root declined"
            );
            continue;
        }
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            members = lineage.len(),
            "iter-consume dead-thread orphan-inc elided (RL-1 move-once)"
        );
        suppressed.extend(lineage);
    }
    suppressed
}

/// Gate (b) support: every var consumed at an `@iter [own]` arg (`Apply @iter` /
/// `Invoke @iter` terminator) OR a user-callee `iter_consumes` param. The
/// `Invoke @iter` terminator form is the inline for-loop's iterator (dead-end
/// #164: it is a TERMINATOR, not an `ArcInstr::Apply`).
fn collect_iter_consumed_vars(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let mut consumed: FxHashSet<ArcVarId> = FxHashSet::default();
    let user_iter = |callee: Name, args: &[ArcVarId], out: &mut FxHashSet<ArcVarId>| {
        if let Some(contract) = contracts.get(&callee) {
            for (pos, &arg) in args.iter().enumerate() {
                if contract.params.get(pos).is_some_and(|p| p.iter_consumes) {
                    out.insert(arg);
                }
            }
        }
    };
    let iter_owned_args = |args: &[ArcVarId],
                           arg_ownership: &[crate::ir::ArgOwnership],
                           out: &mut FxHashSet<ArcVarId>| {
        for (pos, &arg) in args.iter().enumerate() {
            if arg_ownership
                .get(pos)
                .is_none_or(|o| *o == crate::ir::ArgOwnership::Owned)
            {
                out.insert(arg);
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                if *callee == iter_name {
                    iter_owned_args(args, arg_ownership, &mut consumed);
                } else {
                    user_iter(*callee, args, &mut consumed);
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            if *callee == iter_name {
                iter_owned_args(args, arg_ownership, &mut consumed);
            } else {
                user_iter(*callee, args, &mut consumed);
            }
        }
    }
    consumed
}

/// Gate (a): fresh collection roots. TWO allocation forms, lattice-IDENTICAL
/// (both `Owned / Linear / Once / Unique / fresh`, RC = 1):
///  - a `Construct` collection literal (`ListLiteral`/`MapLiteral`/`SetLiteral`)
///    body dst (TF-3 FRESH);
///  - an `Apply __collect_set` result body dst (TF-6 with the `CollectSet`
///    protocol's `Unique ∧ preserves_freshness` return — `.iter().collect()`
///    into a `Set` builds a fresh `{str}` exactly like a `SetLiteral`). A
///    collect-builtin result move-once-consumed at a downstream `@iter [own]`
///    (the chained `original.iter().collect()` shape) carries the SAME orphaned
///    funding inc a fresh `Construct` root does; the move-once elision is the
///    same `RL1_duplication_balanced` arithmetic (root-set widening only, no new
///    realization theorem — proven invariant across source kind:
///    `scratch CollectBuiltinDeadThreadRoot.widening_sound`).
fn collect_fresh_collection_roots(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> Vec<ArcVarId> {
    let collect_set_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet.name());
    let mut roots = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, ctor, .. }
                    if matches!(
                        ctor,
                        CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral
                    ) =>
                {
                    roots.push(*dst);
                }
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if *callee == collect_set_name => {
                    roots.push(*dst);
                }
                _ => {}
            }
        }
    }
    roots
}

/// Gate (c) support: grow the same-alloc lineage from `root` over `Let { Var }`
/// aliases AND Jump-arg → block-param threading across ALL edges (forward AND
/// back — the back-edge is the loop carry the forward-only scans missed). A
/// `Let`-Var alias and a Jump-arg handoff are RC-identity-preserving.
fn grow_thread_lineage(
    func: &ArcFunction,
    root: ArcVarId,
    param_edge_args: &FxHashMap<
        ArcVarId,
        smallvec::SmallVec<[crate::aims::intraprocedural::project_aliases::ParamEdgeArg; 2]>,
    >,
) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    loop {
        let mut grew = false;
        // Let-Var aliases.
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
        }
        // Jump-arg -> block-param threading across ALL edges (forward + back):
        // a block-param whose every arriving edge arg is a lineage member joins
        // the lineage (the loop-carried thread). Per-param edge args are the
        // `compute_param_edge_args` SSOT.
        for (&param, edges) in param_edge_args {
            if members.contains(&param) {
                continue;
            }
            if edges.is_empty() {
                continue;
            }
            // A param joins the lineage when every FORWARD edge arg is a member
            // AND every BACK-edge arg is a member OR the param itself (the loop
            // carries the same allocation: the back-edge re-threads this param's
            // own value, so a back-edge arg == param is the loop carry, not a
            // distinct second source). At least one forward edge must bring a
            // member (the loop-entry thread) — a param fed ONLY by back-edges of
            // non-members is not in this lineage.
            let forward_ok = edges
                .iter()
                .filter(|e| !e.is_back_edge)
                .all(|e| members.contains(&e.arg));
            let any_forward_member = edges
                .iter()
                .any(|e| !e.is_back_edge && members.contains(&e.arg));
            let back_ok = edges
                .iter()
                .filter(|e| e.is_back_edge)
                .all(|e| members.contains(&e.arg) || e.arg == param);
            if forward_ok && any_forward_member && back_ok && members.insert(param) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    members
}

/// Gates (c) + (d): every block-param member is dead-threaded (used ONLY as a
/// Jump-arg re-thread) AND the lineage's only genuine consume is the `@iter [own]`.
/// Returns false (decline) on ANY genuine read of a member or any owned-position
/// consume that is not the iter-consume.
fn dead_thread_vetted(
    func: &ArcFunction,
    lineage: &FxHashSet<ArcVarId>,
    iter_consumed: &FxHashSet<ArcVarId>,
) -> bool {
    // The lineage's BLOCK-PARAM members are the loop-carried thread (the dead
    // carry); a non-param member (the fresh-Construct root + its `Let`-Var
    // aliases) may be borrow-read BEFORE the @iter consume (`xs[0]` length /
    // index). A block-param member read at ANY position other than a Jump-arg
    // re-thread means the post-loop thread is LIVE (the `str_list_explicit_last_owner`
    // shape) — decline. Collect the param members so the vet can hold them to the
    // stricter Jump-arg-only rule while allowing root/alias borrow-reads.
    let param_members: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .flat_map(|b| b.params.iter().map(|&(p, _)| p))
        .filter(|p| lineage.contains(p))
        .collect();
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().iter().any(|v| lineage.contains(v)) {
                continue;
            }
            match instr {
                // Alias-edge internal to the lineage.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if lineage.contains(src) => {}
                // A `Project` of a NON-param lineage member (the root / a `Let`-Var
                // alias) is a BORROW-view (a length / index read like `xs[0]` before
                // the loop) — the value survives move-once into @iter. A `Project`
                // of a PARAM member means the loop-carried thread is LIVE post-loop
                // -> decline.
                ArcInstr::Project { value, .. }
                    if lineage.contains(value) && !param_members.contains(value) => {}
                // An `Apply` consume of a lineage member: the `@iter [own]` consume
                // is the sanctioned genuine consume; a BORROWED-position NON-param
                // member arg (e.g. `@__index(%5 [borrow])` for `xs[0]`) is a benign
                // borrow-read. Decline an OWNED-position non-@iter member arg (a
                // transfer / duplication) OR ANY read of a PARAM member (the
                // loop-carried thread must stay dead).
                ArcInstr::Apply { args, .. } => {
                    if args.iter().enumerate().any(|(i, a)| {
                        lineage.contains(a)
                            && (param_members.contains(a)
                                || (instr.is_owned_position(i) && !iter_consumed.contains(a)))
                    }) {
                        return false;
                    }
                }
                // Any other instruction touching a lineage member (Construct / Set
                // / Reuse store, ApplyIndirect, etc.) is a genuine owned consume
                // -> decline.
                _ => return false,
            }
        }
        let term = &block.terminator;
        match term {
            // Jump-arg re-thread of a lineage member: the dead carry. Benign.
            ArcTerminator::Jump { .. } => {}
            // An `Invoke` terminator consume of a lineage member: the `@iter [own]`
            // consume is the sanctioned genuine consume; a BORROWED-position member
            // arg is a benign borrow-read (value survives). Decline only an
            // OWNED-position member arg that is NOT the @iter consume.
            ArcTerminator::Invoke { args, .. } => {
                if args.iter().enumerate().any(|(i, a)| {
                    lineage.contains(a)
                        && (param_members.contains(a)
                            || (term.is_owned_position(i) && !iter_consumed.contains(a)))
                }) {
                    return false;
                }
            }
            // Any other terminator carrying a lineage member (Return / Branch /
            // Switch / Resume / InvokeIndirect) is a genuine read or escape ->
            // decline (the post-loop thread must be DEAD; a Branch/Switch/Return
            // operand is a genuine read).
            other => {
                if other.used_vars().iter().any(|v| lineage.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests;
