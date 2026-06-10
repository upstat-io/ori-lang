//! Phase-5 lineage-death-point treatment for two borrowed-may-unwind-`Invoke`
//! lineage families sharing one dead-block-param death point:
//!  - a FRESH `Construct` COLLECTION (`ListLiteral` / `MapLiteral` /
//!    `SetLiteral`) borrowed into a may-unwind `Invoke` (`__index` / a user
//!    borrowed-receiver call) — the RECEIVER lineage;
//!  - a heap RESULT of a may-unwind borrowed-receiver user call returning a
//!    self-inc'd heap value (`@first(xs) = xs[0]`) — the RESULT lineage.
//!
//! Shape, over-emission mechanism, cure, and admission gates are documented on
//! [`compute_borrowed_invoke_collection_lineage`]. Sibling of `live_extract.rs`
//! (the niche-family-sum live-extract treatment): same single-placed-release
//! emission surface (`ForwarderReleasePos`), DISTINCT root families and a
//! DISTINCT consume site (a borrowed may-unwind `Invoke` arg / result reaching a
//! dead merge block-param vs a niche-family-sum match-extract).

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

use super::{function_used_vars, successor_reachable_blocks, ForwarderReleasePos};

/// One root's candidate admission, held until the pairwise-disjoint gate runs
/// over the full candidate set.
struct Candidate {
    root: ArcVarId,
    members: FxHashSet<ArcVarId>,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    dec_var: ArcVarId,
}

/// Result of [`compute_borrowed_invoke_collection_lineage`]: the same-alloc
/// closure to suppress (the inline borrowed-`Invoke` dec + lineage-wide
/// duplication incs) + the single placed release at the lineage's death point.
pub(in crate::lower::burden_lower) struct BorrowedInvokeCollectionLineage {
    /// Every var in an admitted fresh-collection borrowed-`Invoke` closure.
    /// Removed from `owned_vars_needing_rc` so the walk emits NO inline
    /// terminator dec (the cycle-1 UAF) AND no lineage-wide duplication inc
    /// (the multi-borrow `%5 = %2` dup-alias inc); the sole release is the
    /// placed dec below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — exactly ONE whole-var `BurdenDec` per
    /// admitted closure, placed AFTER the lineage's execution-final transitive
    /// borrow-read (the normal successor of the LAST borrowed-`Invoke` that
    /// reads the lineage). Merged into the `forwarder_result_releases` emission
    /// surface (identical `ForwarderReleasePos` placement contract). The unwind
    /// / unreachable per-edge releases on the dying paths are owned by the
    /// Surface-1 Category-2 `collect_invoke_edge_decs` `deadAtSucc` conjunct
    /// (`edge_cleanup.rs`) — disjoint edges, no double-release.
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// RL-2 + RL-4 lineage-death-point treatment for a borrowed-`Invoke`-terminator
/// arg whose lineage root is a FRESH `Construct` collection buffer.
///
/// Shape (the `catch(expr: xs[9]); xs.len()` / `catch(expr: xs[1])` family —
/// a fresh `[str]` / `[int]` collection indexed inside a `catch`): a fresh
/// collection `%2 = Construct List(..)` is read through `Let { Var }` aliases
/// (`%5 = %2`, `%13 = %2`) and a length `Project`, then BORROWED into a
/// may-unwind `Invoke @__index(%5 [borrow], ..)` terminator. The collection is
/// LIVE-ACROSS the catch (read again past it — `xs.len()` / `xs[0]`), so the
/// construct-fed-dead-param collection arm (`compute_construct_fed_dead_param_lineage`
/// gate (a), call-site-count ≤ 1) DECLINES it. The base walk then OVER-emits:
/// a dup-alias `BurdenInc` on `%5` (the `use_counts >= 2` proxy mis-classes the
/// same-alloc alias as a duplication: `%2` is "live" only because it ALSO feeds
/// the second `Invoke` / the post-catch read) + an inline `BurdenDec %5`
/// emitted BEFORE the terminator that reads `%5`. The inline dec releases the
/// collection buffer while `%2` is still live → use-after-free / double-free
/// (cycle-1, attempt-147 teeth).
///
/// The cure removes the WHOLE same-alloc closure (the Construct root + Let-Var
/// aliases + Jump-arg block-params) from `owned_vars_needing_rc` — suppressing
/// the dup-alias incs AND the inline terminator dec — and emits EXACTLY ONE
/// whole-var `BurdenDec` on the lineage var read at the closure's
/// execution-FINAL borrow-read, placed AFTER that read
/// (`RL2_release_exactly_once`, no UAF). The dying unwind / unreachable edges
/// are released by the Surface-1 Category-2 `deadAtSucc` conjunct
/// (`edge_cleanup.rs::collect_invoke_edge_decs`) — disjoint edges from the
/// placed normal-path release.
///
/// Admission gates (ALL must hold per closure; ANY failure declines the root
/// and keeps current behavior — the status-quo leak is FAR safer than an
/// over-fire double-free, per the attempt-147 DO-NOT-RE-TRY):
///  (a) FRESH collection-`Construct` root: a `Construct { ctor: ListLiteral |
///      MapLiteral | SetLiteral }` dst (the fresh buffer). String literals /
///      panic messages have NO `Construct` definer → never candidates (they
///      decline naturally, per the re-consensus — keeps the panic-message
///      shapes Category-2 deliberately skips out of scope).
///  (b) root in `owned_vars_needing_rc` (the buffer carries RC) AND not already
///      claimed by the construct-fed dead-param family (which runs first; its
///      call-site-count ≤ 1 lineages are the dead-param shape, not this one).
///  (c) at least one closure member is a borrowed `Invoke` / `InvokeIndirect`
///      terminator arg (the carrier that needs the inline-dec suppression).
///  (d) vetted same-alloc closure ([`same_alloc_closure_vetted`]): every member
///      use is a borrow-read / borrowed-`Invoke` arg; any owned-position consume
///      / store / capture / `Select` / `IsShared` / `Reset` / `Reuse` / `Return`
///      / indirect-call / `Set` / `SetTag` declines (the codex gate list).
///  (e) execution-final single release ([`choose_release_site`] +
///      [`release_site_sound`]): the dec lands after the closure's final
///      borrow-read on every normal exit; unwind paths are owned by Surface 1.
///  (f) pairwise-DISJOINT closures: two admitted roots whose closures overlap
///      converge on one shared final read; each would place its own dec there,
///      double-freeing. ALL roots of an overlapping group decline.
pub(in crate::lower::burden_lower) fn compute_borrowed_invoke_collection_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    claimed_by_construct_fed: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> BorrowedInvokeCollectionLineage {
    let mut out = BorrowedInvokeCollectionLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    // Two root families share the dead-param death-point treatment:
    //  - FRESH collection-`Construct` buffers borrowed into a may-unwind
    //    `Invoke` (`__index` / a user borrowed-receiver call) — the RECEIVER
    //    lineage.
    //  - heap RESULTS of a may-unwind borrowed-receiver user call that returns
    //    a heap value (`@first(xs) = xs[0]` returns a self-inc'd element) —
    //    borrow-read-only then reaching a dead block-param. Its FRESH-site inc
    //    is spurious (the callee already transferred +1); suppressing the
    //    lineage strips it and the dead-param release frees the callee's +1.
    let collection_roots = collect_fresh_collection_construct_roots(func);
    let result_root_vec =
        collect_borrowed_call_result_roots(func, owned_vars_needing_rc, contracts);
    let result_roots: FxHashSet<ArcVarId> = result_root_vec.iter().copied().collect();
    let mut roots: Vec<ArcVarId> = collection_roots;
    roots.extend(result_root_vec);
    if roots.is_empty() {
        return out;
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut claimed_roots: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in roots {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "borrowed-invoke collection lineage declined"
            );
        };
        // Gate (b): heap-carrying buffer + not already claimed by the
        // construct-fed dead-param family (the call-site-count ≤ 1 dead-param
        // shape) + not already claimed within this scan.
        if !owned_vars_needing_rc.contains(&root)
            || claimed_by_construct_fed.contains(&root)
            || claimed_roots.contains(&root)
        {
            decline("b:owned/claimed");
            continue;
        }
        // Gate (d): vetted same-alloc closure (incl. the Jump-arg → dead
        // block-param hop — the lineage's death point).
        let Some(members) = same_alloc_closure_vetted(func, root) else {
            decline("d:closure-vet");
            continue;
        };
        // Gate (c): a COLLECTION root must have a member at a borrowed `Invoke`
        // arg (the carrier whose inline dec is the bug). A RESULT root is itself
        // a may-unwind `Invoke` result (qualified by the collector) — no
        // borrowed-arg carrier needed.
        if !result_roots.contains(&root) && !closure_has_borrowed_invoke_arg(func, &members) {
            decline("c:no-borrowed-invoke");
            continue;
        }
        // Gate (c2): DECLINE when any member is a borrowed-`Invoke` arg to an
        // ITER-CONSUMING callee (`for w in coll` inside the callee → its
        // `ori_iter_drop` frees the buffer; RL-2 `iter_consumes` ownership
        // transfer DESPITE the borrowed position). The callee owns the release,
        // so a dead-param release here double-frees — the cycle-1 attempt-147
        // DO-NOT-RE-TRY `unwind_list_reusable_after_catch` over-fire (a fresh
        // collection borrowed into a catch'd iter-consuming user call AND reused
        // after). Restricts the family to NON-iter-consuming borrowed-`Invoke`
        // carriers (`__index`, `@first(xs) = xs[0]`) whose borrowed arg is a
        // genuine borrow-read, not a transfer.
        if closure_member_iter_consumed_at_invoke(func, &members, contracts) {
            decline("c2:iter-consume-transfer");
            continue;
        }
        // Gate (e): the lineage's death point is a DEAD block-param (the
        // post-catch-merge sink the receiver is carried to), execution-final
        // (forward-reachable from every member-read block). The release lands at
        // that dead-param block's entry — the receiver is freed exactly once
        // after all borrow-reads complete.
        let Some((site_block, dec_var)) = choose_dead_param_release_site(func, &members, &used)
        else {
            decline("e:dead-param-site");
            continue;
        };
        claimed_roots.extend(members.iter().copied());
        candidates.push(Candidate {
            root,
            members,
            site_block,
            site_pos: ForwarderReleasePos::BlockEntry,
            dec_var,
        });
    }

    // Gate (f): decline EVERY candidate whose closure overlaps another's.
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
                root = cand.root.index(),
                gate = "f:closure-overlap",
                "borrowed-invoke collection lineage declined"
            );
            continue;
        }
        out.suppressed_lineage_vars
            .extend(cand.members.iter().copied());
        out.releases
            .entry((cand.site_block, cand.site_pos))
            .or_default()
            .push(cand.dec_var);
    }
    out
}

/// Gate (a): candidate roots — every `Construct { ctor: ListLiteral |
/// MapLiteral | SetLiteral }` dst (the fresh collection buffer). A
/// `Let { Literal::String }` heap-str body is NOT a candidate (no `Construct`
/// definer; the re-consensus declines panic-message / string-literal lineages
/// naturally).
fn collect_fresh_collection_construct_roots(func: &ArcFunction) -> Vec<ArcVarId> {
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct {
                dst,
                ctor: CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral,
                ..
            } = instr
            {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Candidate RESULT roots: a heap `Invoke` result of a may-unwind USER call
/// (`@first(xs) = xs[0]` returns a self-inc'd element) that the callee returns
/// owned (`return_info.uniqueness ∈ {Unique, MaybeShared}`) WITHOUT transferring
/// a param through the return (a forwarder result is the ARG's allocation, owned
/// by the forwarder scans) and WITHOUT iter-consuming a param (the iter-consume
/// transfer owns the release). The result's FRESH-site inc is spurious — the
/// callee already handed +1 — and the result, borrow-read-only, dies at a dead
/// block-param; the suppress + dead-param release frees the callee's +1 exactly
/// once. The death-point + vet gates ([`choose_dead_param_release_site`] +
/// [`same_alloc_closure_vetted`]) bound the over-fire surface.
fn collect_borrowed_call_result_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<ArcVarId> {
    let owned_result_non_forwarder_non_iter = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            matches!(
                c.return_info.uniqueness,
                crate::aims::lattice::Uniqueness::Unique
                    | crate::aims::lattice::Uniqueness::MaybeShared
            ) && !c.params.iter().any(|p| p.transfers_through_return)
                && !c.params.iter().any(|p| p.iter_consumes)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_vars_needing_rc.contains(dst) && owned_result_non_forwarder_non_iter(callee) {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Gate (c): true iff any closure member is used at a BORROWED (non-owned) arg
/// position of an `Invoke` / `InvokeIndirect` terminator — the carrier whose
/// inline-before-terminator `BurdenDec` is the cycle-1 UAF.
fn closure_has_borrowed_invoke_arg(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        let term = &block.terminator;
        if !matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if members.contains(&v) && !term.is_owned_position(pos) {
                return true;
            }
        }
    }
    false
}

/// Gate (c2): true iff any closure member is used at a borrowed `Invoke` /
/// `InvokeIndirect` arg position whose callee `iter_consumes` that position (a
/// `for w in coll` inside the callee → its `ori_iter_drop` frees the buffer; an
/// RL-2 ownership transfer DESPITE the borrowed arg position, per
/// `ParamContract.iter_consumes`). The callee owns the release, so a dead-param
/// release on such a lineage double-frees. Mirrors the `iter_consumes` detection
/// in `compute_ttr_iter_consume_dup_aliases` (the same SSOT signal).
fn closure_member_iter_consumed_at_invoke(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    for block in &func.blocks {
        // Only a named-callee `Invoke` carries a contract with `iter_consumes`;
        // an `InvokeIndirect` (closure) has no contract — conservatively NOT an
        // iter-consume transfer here (it declines elsewhere via the `Apply
        // Indirect` vet anyway).
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            let Some(contract) = contracts.get(callee) else {
                continue;
            };
            for (pos, &arg) in args.iter().enumerate() {
                if members.contains(&arg)
                    && contract.params.get(pos).is_some_and(|p| p.iter_consumes)
                {
                    return true;
                }
            }
        }
    }
    false
}

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
fn same_alloc_closure_vetted(func: &ArcFunction, root: ArcVarId) -> Option<FxHashSet<ArcVarId>> {
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
fn member_uses_all_borrow_reads(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
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
                    if super::list_concat_consumed_operands(instr, func)
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

/// Gate (e): the lineage's death point — a DEAD block-param member (a member
/// bound by a block-param AND unused: `Cardinality = Absent`, its only
/// appearance is its own binding slot) reached by the receiver's Jump-arg
/// hand-offs, that is EXECUTION-FINAL: forward-reachable from every block that
/// borrows a member, so every borrow-read completes before the release. Returns
/// `(dead_param_block, dead_param_var)`; the release lands at that block's entry
/// (`ForwarderReleasePos::BlockEntry`). `None` when the lineage has no dead
/// block-param, or has more than one (a fork the single release cannot cover),
/// or the dead-param block is not forward-reachable from some member-read block
/// (a member borrowed AFTER the dec would UAF). A CFG cycle re-reaching the
/// dead-param block declines (a re-reached dec double-frees).
fn choose_dead_param_release_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    used: &FxHashSet<ArcVarId>,
) -> Option<(usize, ArcVarId)> {
    // Dead block-param members: a member that is a block-param of some block AND
    // is never used (the carried-to-merge sink). Collect (block_idx, param_var).
    let mut dead_params: Vec<(usize, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(param, _) in &block.params {
            if members.contains(&param) && !used.contains(&param) {
                dead_params.push((block_idx, param));
            }
        }
    }
    // Exactly one dead block-param sink — a fork (two dead params on disjoint
    // paths) cannot be covered by a single release; decline.
    if dead_params.len() != 1 {
        return None;
    }
    let (site_block, dec_var) = dead_params[0];
    // (e1) the dead-param block must not sit in a CFG cycle (a re-reached dec
    // double-frees).
    if successor_reachable_blocks(func, site_block).contains(&site_block) {
        return None;
    }
    // (e2) every block that BORROW-READS a member must forward-reach the
    // dead-param block — every read completes before the release (no UAF).
    for (block_idx, block) in func.blocks.iter().enumerate() {
        if block_idx == site_block {
            continue;
        }
        let reads_member = block
            .body
            .iter()
            .any(|i| i.used_vars().iter().any(|v| members.contains(v)))
            || block
                .terminator
                .used_vars()
                .iter()
                .any(|v| members.contains(v));
        if reads_member && !successor_reachable_blocks(func, block_idx).contains(&site_block) {
            return None;
        }
    }
    Some((site_block, dec_var))
}

/// `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE=1` declines the Phase-5
/// borrowed-`Invoke`-collection lineage treatment (the inline-dec suppression +
/// the single placed death-point release). Read once at first access.
pub(in crate::lower::burden_lower) fn borrowed_invoke_lineage_release_disabled() -> bool {
    use std::sync::LazyLock;
    static DISABLED: LazyLock<bool> = LazyLock::new(|| {
        std::env::var("ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE").as_deref() == Ok("1")
    });
    *DISABLED
}

#[cfg(test)]
mod tests;
