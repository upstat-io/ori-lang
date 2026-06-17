//! RL-2 release for a fresh `PartialApply` closure borrowed into a lazy-iterator
//! builtin (`@map` / `@filter`) whose result iterator retains the closure env
//! across the chain's terminal consumer (annex-e §AIMS RL-2 + TF-4 + TF-7).
//!
//! Shape: `let f = (x) -> ...captured...; xs.iter().map(f).collect()` — a FRESH
//! `PartialApply` closure (TF-7: Owned, Unique, `BlockLocal`, an allocation with
//! RC = 1 at birth) capturing a heap value, passed at a BORROWED arg position to
//! a lazy-iterator-producing builtin (`@map` / `@filter`). The builtin stores the
//! closure env as a BORROWED raw pointer in the lazy `IterState::Mapped` /
//! `Filtered` adapter (the runtime adapter Drop is a bare `_ => {}` arm — the
//! iterator NEVER incs or drops the env, so the caller retains ownership). The
//! lazy iterator invokes the closure during the chain's TERMINAL consumer
//! (`@collect` / `@fold` / `@count` / …), AFTER the `@map`/`@filter` call returns.
//!
//! Per `RL2_borrowed_param_emits_caller_dec` the caller releases a borrowed arg,
//! and per `RL2_release_exactly_once` exactly once — but AFTER the value's final
//! transitive use. The base Phase-5 walk places the closure's `BurdenDec` at its
//! last *var*-use (the `@map`/`@filter` borrowed arg), BEFORE the lazy iterator is
//! consumed: the env (and its cascade-freed captured heap payload) is freed while
//! the lazy iterator still holds the borrowed env pointer → use-after-free when
//! the closure runs at `@collect` time (exit −139). `@fold` survives the identical
//! early dec because it is EAGER (the closure runs synchronously inside `@fold`,
//! before the env is freed); `@map`/`@filter` are LAZY (deferred to `@collect`).
//!
//! The cure removes the whole same-alloc closure lineage from
//! `owned_vars_needing_rc` (suppressing the early borrowed-arg dec) and places
//! EXACTLY ONE whole-var `BurdenDec` at the lazy-iterator chain's terminal
//! consumer's normal-successor entry — after the consumer ran the closure for the
//! last time (`RL2_release_exactly_once`: the closure's single birth reference is
//! released exactly once, net 0). Dead-end #154 confirms the runtime never owns
//! the env; the release is the CALLER's obligation deferred past the chain.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{Name, StringInterner};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::ForwarderReleasePos;

/// Result of [`compute_lazy_iter_closure_borrow_lineage`]: the same-alloc closure
/// lineage to suppress (kills the early borrowed-arg dec the base walk emits) plus
/// the single RL-2 release at the lazy-iterator chain's terminal consumer.
pub(in crate::lower::burden_lower) struct LazyIterClosureBorrow {
    /// Every var in an admitted fresh-closure lineage — removed from
    /// `owned_vars_needing_rc` so the only release is the placed dec below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, BlockEntry) -> [closure root var]` — one whole-var `BurdenDec`
    /// per admitted lineage at the terminal consumer's normal-successor entry.
    /// Merged into the `forwarder_result_releases` emission surface.
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// One admitted lineage's facts, held until the pairwise-disjoint gate runs.
struct Candidate {
    members: FxHashSet<ArcVarId>,
    release_block: usize,
    release_var: ArcVarId,
}

/// Lazy-iterator-producing builtins that store the closure env as a BORROWED raw
/// pointer in the adapter and invoke it at the chain's terminal consumer. `@fold`
/// is EAGER (excluded — it runs the closure synchronously, so the base walk's
/// early dec is harmless).
const LAZY_CLOSURE_BUILTINS: &[&str] = &["map", "filter"];

/// Iterator-result-consuming builtins: each takes the lazy iterator at an OWNED
/// position and either advances the chain (adapter) or terminally consumes it.
/// The terminal members (`collect` / `count` / `join` / …) run the retained
/// closure for the last time; the release lands after them.
const ITER_CHAIN_BUILTINS: &[&str] = &[
    "map",
    "filter",
    "take",
    "skip",
    "collect",
    "count",
    "any",
    "all",
    "find",
    "join",
    "enumerate",
];

/// RL-2 treatment for a fresh closure borrowed into a lazy-iterator builtin.
///
/// Admission gates (ALL hold per closure; ANY failure declines the root and
/// keeps current behavior — the status-quo use-after-free is the migration
/// floor, never a regression introduced here):
///  (a) FRESH closure root: a `PartialApply` body dst (TF-7 fresh closure).
///  (b) root in `owned_vars_needing_rc` (heap-carrying, RC-tracked) AND
///      `FatValue` repr (closure two-word value).
///  (c) vetted borrow-only lineage ([`closure_lineage_vetted`]): grow over
///      `Let { Var }` aliases; EVERY use of every member is an alias-edge or a
///      BORROWED arg — any owned-consume / store / capture / `Project` / `Return`
///      / terminator-operand declines.
///  (d) the lineage's SOLE non-alias use is a BORROWED arg to a lazy-iterator
///      builtin (`@map` / `@filter`) — the borrowed-into-lazy site. Zero or >1
///      such site declines (a fork the single release cannot cover).
///  (e) the lazy-iterator chain from the builtin result reaches EXACTLY ONE
///      terminal consumer whose normal-successor entry dominates no further chain
///      use — the single release site. `None` on a fork / unconsumed chain.
///  (f) pairwise-DISJOINT lineages: two admitted roots sharing any member
///      converge on one allocation web; ALL roots of an overlapping group decline.
pub(in crate::lower::burden_lower) fn compute_lazy_iter_closure_borrow_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &StringInterner,
) -> LazyIterClosureBorrow {
    let mut out = LazyIterClosureBorrow {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let lazy_names: FxHashSet<Name> = LAZY_CLOSURE_BUILTINS
        .iter()
        .map(|n| interner.intern(n))
        .collect();
    let chain_names: FxHashSet<Name> = ITER_CHAIN_BUILTINS
        .iter()
        .map(|n| interner.intern(n))
        .collect();
    let mut candidates: Vec<Candidate> = Vec::new();

    for root in collect_partial_apply_roots(func) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "lazy-iter closure-borrow root declined"
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
        let members = grow_closure_lineage(func, root);
        if !closure_lineage_vetted(func, &members) {
            decline("c:vet");
            continue;
        }
        // Gate (d): exactly one lazy-builtin borrowed-arg site over the lineage.
        let Some(lazy_result) = unique_lazy_builtin_result(func, &members, &lazy_names) else {
            decline("d:no-unique-lazy-site");
            continue;
        };
        // Gate (e): trace the lazy-iterator chain to its terminal consumer; the
        // release lands at that consumer's normal-successor entry.
        let Some((release_block, release_var)) =
            chain_terminal_release_site(func, lazy_result, root, &chain_names)
        else {
            decline("e:no-terminal-consumer");
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
                "lazy-iter closure-borrow root declined"
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
            "lazy-iter closure-borrow release placed (RL-2)"
        );
    }
    out
}

/// Gate (a): candidate roots — every `PartialApply` body dst (TF-7 fresh closure).
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

/// Gate (c): grow the same-alloc closure lineage from `root` over `Let { Var }`
/// aliases. A `Let`-Var alias is RC-identity-preserving (no birth, no inc).
fn grow_closure_lineage(func: &ArcFunction, root: ArcVarId) -> FxHashSet<ArcVarId> {
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
        }
        if !grew {
            break;
        }
    }
    members
}

/// Gate (c) vetting core: true iff EVERY use of every member is a benign
/// borrow-or-alias appearance. Any owned-consume / store / capture / projection /
/// escape declines (the closure must flow ONLY as a borrowed arg + alias edges).
fn closure_lineage_vetted(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches {
                continue;
            }
            match instr {
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // Apply: a member may appear ONLY at a BORROWED arg position.
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
                // store / capture / projection / ApplyIndirect — decline.
                _ => return false,
            }
        }
        let term = &block.terminator;
        match term {
            ArcTerminator::Jump { .. } => {
                if term.used_vars().iter().any(|v| members.contains(v)) {
                    // The closure flowing as a Jump arg is a control-flow thread
                    // this single-use lazy shape does not model — decline.
                    return false;
                }
            }
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
            other => {
                if other.used_vars().iter().any(|v| members.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Gate (d): the UNIQUE result dst of a lazy-iterator builtin (`@map`/`@filter`)
/// that borrows a lineage member. Returns the result var, or `None` when zero or
/// more than one such site exists. The builtin must be an `Apply` or `Invoke`
/// with the closure at a borrowed arg position and a heap-iterator result dst.
fn unique_lazy_builtin_result(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    lazy_names: &FxHashSet<Name>,
) -> Option<ArcVarId> {
    let mut found: Option<ArcVarId> = None;
    let mut record = |dst: ArcVarId| -> bool {
        if found.is_some() {
            return false; // >1 site — caller declines
        }
        found = Some(dst);
        true
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                let borrows_member = lazy_names.contains(callee)
                    && args
                        .iter()
                        .enumerate()
                        .any(|(i, a)| members.contains(a) && !instr.is_owned_position(i));
                if borrows_member && !record(*dst) {
                    return None;
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            let borrows_member = lazy_names.contains(callee)
                && args
                    .iter()
                    .enumerate()
                    .any(|(i, a)| members.contains(a) && !block.terminator.is_owned_position(i));
            if borrows_member && !record(*dst) {
                return None;
            }
        }
    }
    found
}

/// Gate (e): trace the lazy-iterator chain from `lazy_result` through owned
/// iterator-result-consuming builtins (`@map`/`@filter`/`@take`/`@collect`/…) to
/// the chain's TERMINAL consumer. The release lands at the terminal consumer's
/// normal-successor entry (the iterator — and its borrowed closure pointer — is
/// dead once consumed). `None` on a fork (the chain result feeds >1 consumer) or
/// when the terminal consumer has no single normal successor.
///
/// `release_var` is the closure `root` (the suppressed lineage's allocation), in
/// scope at the terminal successor since the `PartialApply` dominates the chain.
fn chain_terminal_release_site(
    func: &ArcFunction,
    lazy_result: ArcVarId,
    root: ArcVarId,
    chain_names: &FxHashSet<Name>,
) -> Option<(usize, ArcVarId)> {
    // Walk the iterator chain: each step is an owned consume of the current
    // iterator var by a chain builtin, yielding the next iterator/result var.
    let mut current = lazy_result;
    // Bound the walk by the instruction count (each chain step consumes a
    // distinct dst; a cycle is impossible in straight-line iterator chains).
    let max_steps = func.blocks.iter().map(|b| b.body.len() + 1).sum::<usize>() + 1;
    for _ in 0..max_steps {
        let consumers = iter_owned_consumers(func, current, chain_names);
        match consumers.as_slice() {
            // Terminal: the current iterator is consumed by exactly one chain
            // builtin whose own result is NOT further consumed as an iterator.
            [(block_idx, dst, is_terminator)] => {
                let next_consumers = iter_owned_consumers(func, *dst, chain_names);
                if next_consumers.is_empty() {
                    // `dst` is the terminal consumer's result (a collection / scalar).
                    // Place the release at the consumer's normal-successor entry.
                    let release_block = if *is_terminator {
                        normal_successor(func, *block_idx)?
                    } else {
                        // A body Apply consumer: the closure is dead right after
                        // the chain completes in this block; release at this
                        // block's terminal flow — use the block's normal successor
                        // if it has one, else the block itself is terminal.
                        normal_successor(func, *block_idx).unwrap_or(*block_idx)
                    };
                    return Some((release_block, root));
                }
                current = *dst;
            }
            // Zero consumers (the lazy iterator is produced but never consumed —
            // a leak shape this scan does not model) OR a fork (>1 consumer of one
            // iterator): decline rather than place a release that could double-free.
            _ => return None,
        }
    }
    None
}

/// All owned-position consumers of iterator var `v` by a chain builtin. Returns
/// `(block_idx, result_dst, is_terminator)` per consume site.
fn iter_owned_consumers(
    func: &ArcFunction,
    v: ArcVarId,
    chain_names: &FxHashSet<Name>,
) -> Vec<(usize, ArcVarId, bool)> {
    let mut out = Vec::new();
    for (bi, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if chain_names.contains(callee)
                    && args
                        .iter()
                        .enumerate()
                        .any(|(i, a)| *a == v && instr.is_owned_position(i))
                {
                    out.push((bi, *dst, false));
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            if chain_names.contains(callee)
                && args
                    .iter()
                    .enumerate()
                    .any(|(i, a)| *a == v && block.terminator.is_owned_position(i))
            {
                out.push((bi, *dst, true));
            }
        }
    }
    out
}

/// The single normal successor block of `block_idx`, or `None` when the block's
/// terminator has zero or multiple normal successors (a fork the single release
/// site cannot cover).
fn normal_successor(func: &ArcFunction, block_idx: usize) -> Option<usize> {
    let block = func.blocks.get(block_idx)?;
    match &block.terminator {
        ArcTerminator::Jump { target, .. } => Some(target.index()),
        ArcTerminator::Invoke { normal, .. } => Some(normal.index()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
