//! Phase-5 lineage-death-point treatment for two borrowed-may-unwind-`Invoke`
//! lineage families sharing one dead-block-param death point:
//!  - a FRESH `Construct` COLLECTION (`ListLiteral` / `MapLiteral` /
//!    `SetLiteral`) borrowed into a may-unwind `Invoke` (`__index` / a user
//!    borrowed-receiver call) — the RECEIVER lineage;
//!  - a heap RESULT of a may-unwind borrowed-receiver user call returning a
//!    self-inc'd heap value (`@first(xs) = xs[0]`) — the RESULT lineage.
//!
//! Shape, over-emission mechanism, cure, and admission gates: see
//! [`compute_borrowed_invoke_collection_lineage`]. Sibling of `live_extract.rs`
//! — same single-placed-release emission surface (`ForwarderReleasePos`),
//! DISTINCT root families + a DISTINCT consume site.

mod death_point;
mod vet;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};

use super::{function_used_vars, ForwarderReleasePos};
use death_point::{choose_death_point, DeathPoint};
use vet::same_alloc_closure_vetted;

/// One root's candidate admission, held until the pairwise-disjoint gate runs
/// over the full candidate set.
struct Candidate {
    root: ArcVarId,
    members: FxHashSet<ArcVarId>,
    /// The death-point treatment chosen for this closure (gate e).
    death: DeathPoint,
}

/// Result of [`compute_borrowed_invoke_collection_lineage`]: the same-alloc
/// closure to suppress (the inline borrowed-`Invoke` dec + lineage-wide
/// duplication incs) + the single placed release at the lineage's death point
/// (dead-param mode) OR the claim that hands the per-edge release to the landed
/// Category-2 `deadAtSucc` machinery (no-sink mode).
pub(in crate::lower::burden_lower) struct BorrowedInvokeCollectionLineage {
    /// Every var in an admitted fresh-collection borrowed-`Invoke` closure.
    /// Removed from `owned_vars_needing_rc` so the walk emits NO inline
    /// terminator dec (the use-after-free) AND no lineage-wide duplication inc
    /// (the multi-borrow `%5 = %2` dup-alias inc); the sole release is the
    /// placed dec below (dead-param mode) or the per-edge Category-2 dec
    /// (no-sink mode).
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — DEAD-PARAM MODE ONLY: exactly ONE
    /// whole-var `BurdenDec` per admitted closure with a dead merge block-param
    /// sink, placed AFTER the lineage's execution-final transitive borrow-read
    /// (the normal successor of the LAST borrowed-`Invoke` that reads the
    /// lineage). Merged into the `forwarder_result_releases` emission surface
    /// (identical `ForwarderReleasePos` placement contract). The unwind /
    /// unreachable per-edge releases on the dying paths are owned by the
    /// Surface-1 Category-2 `collect_invoke_edge_decs` `deadAtSucc` conjunct
    /// (`edge_cleanup.rs`) — disjoint edges, no double-release.
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
    /// NO-SINK MODE: the receiver var (the borrowed-`Invoke` carrier) of an
    /// admitted no-dead-param closure, CLAIMED for the landed Category-2 per-edge
    /// `deadAtSucc` emission. The inline-before-terminator dec is suppressed
    /// (via `suppressed_lineage_vars`); the per-edge release lands on each dying
    /// successor edge of the carrier's `Invoke` (normal + unwind). Cat-2's
    /// `release_with_burden_edge` gates its paired `BurdenDec` on
    /// `carries_burden` (`func.burden_emitted[var]`), so the claim sets that bit
    /// — without it the suppressed var carries no burden ops and Cat-2 filters
    /// out the burden dec, leaving the lowered path's per-value ledger unbalanced.
    /// A live-across receiver (read past the carrier) takes its release at the
    /// LAST borrowed-`Invoke` in its lineage — Cat-2's per-edge `deadAtSucc`
    /// fires only where the carrier is genuinely dead at the successor, so the
    /// post-call last read is the natural death point with no separate placement.
    pub claimed_no_sink_vars: FxHashSet<ArcVarId>,
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
/// collection buffer while `%2` is still live → use-after-free / double-free.
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
/// # Admission gates
///
/// ALL must hold per closure; ANY failure declines the root and keeps current
/// behavior — the status-quo leak is FAR safer than an over-fire double-free.
///  (a) FRESH collection-`Construct` root: a `Construct { ctor: ListLiteral |
///      MapLiteral | SetLiteral }` dst (the fresh buffer). String literals /
///      panic messages have NO `Construct` definer → never candidates (they
///      decline naturally — keeps the panic-message shapes Category-2
///      deliberately skips out of scope).
///  (b) root in `owned_vars_needing_rc` (the buffer carries RC) AND not already
///      claimed by a PRIOR ownership-scan treatment: the construct-fed dead-param
///      family AND the fresh-sum live-extract family (both run first). The
///      live-extract scan is the SSOT for the niche-family-sum match-extract
///      RESULT lineage (`match result { Some(s) -> s }`); its claimed-member set
///      is threaded here so this scan's RESULT-root family declines any closure
///      overlapping it — the two would otherwise both place a death-point release
///      on the same allocation (double-free). Disjointness is enforced at MEMBER
///      grain (the live-extract closure suppresses whole webs), not root-only.
///  (c) at least one closure member is a borrowed `Invoke` / `InvokeIndirect`
///      terminator arg (the carrier that needs the inline-dec suppression).
///  (d) vetted same-alloc closure ([`same_alloc_closure_vetted`]): every member
///      use is a borrow-read / borrowed-`Invoke` arg; any owned-position consume
///      / store / capture / `Select` / `IsShared` / `Reset` / `Reuse` / `Return`
///      / indirect-call / `Set` / `SetTag` declines.
///  (e) death-point treatment ([`choose_death_point`]) — TWO modes:
///      - DEAD-PARAM MODE: the closure reaches EXACTLY ONE dead merge
///        block-param, forward-reachable from every member-borrow-read block, so
///        the placed dec lands after the closure's final borrow-read on every
///        normal exit; unwind / unreachable paths are owned by Surface 1.
///      - NO-SINK MODE: the closure has NO dead-param sink — the
///        receiver dies on the borrowed-`Invoke` carrier's successor edges
///        directly (the `is_ref(a: [7,8])` minimal — the base walk's inline
///        `BurdenDec` before the borrowing terminator nets the fresh inc to zero
///        pre-call, leaking the rc=1 allocation on every executing path). The
///        per-edge release is handed to the landed Category-2 `deadAtSucc`
///        machinery (`edge_cleanup.rs`); the carrier var is CLAIMED so Cat-2's
///        paired `BurdenDec` is admitted (`carries_burden` gate). Requires the
///        carrier `Invoke` to be may-unwind (a `normal`/`unwind` edge split) and
///        the receiver to NOT be live past its LAST borrowed-`Invoke` carrier —
///        a live-across receiver takes its release at that last carrier's
///        successor edge (Cat-2's `deadAtSucc` fires only where the carrier is
///        genuinely dead, so the post-call last read is the natural death point).
///        Spec: Annex E §AIMS RL-2 `RL2_borrowed_param_emits_caller_dec` + RL-4
///        `RL4_edge_release_balanced`.
///  (f) pairwise-DISJOINT closures: two admitted roots whose closures overlap
///      converge on one shared final read; each would place its own dec there,
///      double-freeing. ALL roots of an overlapping group decline.
pub(in crate::lower::burden_lower) fn compute_borrowed_invoke_collection_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    claimed_by_construct_fed: &FxHashSet<ArcVarId>,
    claimed_by_live_extract: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> BorrowedInvokeCollectionLineage {
    let mut out = BorrowedInvokeCollectionLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
        claimed_no_sink_vars: FxHashSet::default(),
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
    let collection_roots = collect_fresh_construct_roots(func, owned_vars_needing_rc);
    let (result_root_vec, fresh_result_roots) =
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
        if let Some(cand) = try_build_candidate(
            func,
            root,
            &used,
            owned_vars_needing_rc,
            claimed_by_construct_fed,
            claimed_by_live_extract,
            &result_roots,
            &fresh_result_roots,
            &claimed_roots,
            contracts,
        ) {
            claimed_roots.extend(cand.members.iter().copied());
            candidates.push(cand);
        }
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
        match cand.death {
            DeathPoint::DeadParam {
                site_block,
                site_pos,
                dec_var,
            } => {
                out.releases
                    .entry((site_block, site_pos))
                    .or_default()
                    .push(dec_var);
            }
            DeathPoint::NoSink { claim } => {
                tracing::trace!(
                    target: "ori_arc::aims::realize",
                    fn_name = ?func.name,
                    root = cand.root.index(),
                    claim = claim.index(),
                    mode = "no-sink",
                    "borrowed-invoke collection lineage admitted (edge-death claim)"
                );
                out.claimed_no_sink_vars.insert(claim);
            }
        }
    }
    out
}

/// Run gates (b) .. (e) for one `root`, returning the admitted [`Candidate`] or
/// `None` (declined). Extracted from
/// [`compute_borrowed_invoke_collection_lineage`] for the 100-line cap; the
/// per-root gate sequence is unchanged.
#[expect(
    clippy::too_many_arguments,
    reason = "the per-root gate sequence threads the full claimed-set context \
              (construct-fed, live-extract, in-scan, result-roots) — bundling \
              would obscure the gate-(b)/(b')/(c) claim disjointness"
)]
fn try_build_candidate(
    func: &ArcFunction,
    root: ArcVarId,
    used: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    claimed_by_construct_fed: &FxHashSet<ArcVarId>,
    claimed_by_live_extract: &FxHashSet<ArcVarId>,
    result_roots: &FxHashSet<ArcVarId>,
    fresh_result_roots: &FxHashSet<ArcVarId>,
    claimed_roots: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Option<Candidate> {
    let decline = |gate: &str| {
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            root = root.index(),
            gate,
            "borrowed-invoke collection lineage declined"
        );
    };
    // Gate (b): heap-carrying buffer + not already claimed by the construct-fed
    // dead-param family (the call-site-count ≤ 1 dead-param shape) + not already
    // claimed within this scan.
    if !owned_vars_needing_rc.contains(&root)
        || claimed_by_construct_fed.contains(&root)
        || claimed_roots.contains(&root)
    {
        decline("b:owned/claimed");
        return None;
    }
    // Gate (d): vetted same-alloc closure (incl. the Jump-arg → dead block-param
    // hop — the lineage's death point).
    let members = same_alloc_closure_vetted(func, root).or_else(|| {
        decline("d:closure-vet");
        None
    })?;
    // Gate (b'): MEMBER-grain disjointness from the fresh-sum live-extract
    // family. The live-extract scan (SSOT for niche-family-sum match-extract
    // RESULT lineages) suppresses a whole same-alloc web — its claimed set names
    // every member, not just the root. A RESULT root whose closure overlaps that
    // web is the SAME allocation the live-extract scan already placed its
    // execution-final release on; admitting it here would place a second
    // death-point release → double-free. Decline the whole closure.
    if !members.is_disjoint(claimed_by_live_extract) {
        decline("b':live-extract-claimed");
        return None;
    }
    // Gate (c): a COLLECTION root must have a member at a borrowed `Invoke` arg
    // (the carrier whose inline dec is the bug). A RESULT root is itself a
    // may-unwind `Invoke` result (qualified by the collector) — no borrowed-arg
    // carrier needed.
    if !result_roots.contains(&root) && !closure_has_borrowed_invoke_arg(func, &members) {
        decline("c:no-borrowed-invoke");
        return None;
    }
    // Gate (c2): DECLINE when any member is a borrowed-`Invoke` arg to an
    // ITER-CONSUMING callee (`for w in coll` inside the callee → its
    // `ori_iter_drop` frees the buffer; RL-2 `iter_consumes` ownership transfer
    // DESPITE the borrowed position). The callee owns the release, so a release
    // here double-frees — the `unwind_list_reusable_after_catch` over-fire.
    if closure_member_iter_consumed_at_invoke(func, &members, contracts) {
        decline("c2:iter-consume-transfer");
        return None;
    }
    // Gate (e): choose the death-point treatment — a DEAD merge block-param sink
    // (dead-param mode) OR the borrowed-`Invoke` carrier's successor edges
    // (no-sink mode). The NO-SINK mode is restricted to FRESH-collection-
    // `Construct` roots (a buffer the caller allocates at rc=1 and owns) — a
    // RESULT root is a borrowed-call result whose release is the callee's
    // transfer / the existing result-lineage machinery, NOT a caller-fresh
    // allocation; the no-sink claim would place a spurious Cat-2 dec on a slice /
    // forwarded view (`@substring`/`@repeat` return a same-buffer slice →
    // double-free). A RESULT root joins the no-sink mode ONLY when its callee
    // contract proves a FRESH allocation (Unique + preserves_freshness, never
    // ttr) — the contract-aware discriminator separating provably-fresh results
    // from same-buffer views (`@substring` / `@repeat`, where an edge release
    // double-frees); all other result roots stay DEAD-PARAM-mode-only.
    let allow_no_sink = !result_roots.contains(&root) || fresh_result_roots.contains(&root);
    let death = choose_death_point(func, &members, used, allow_no_sink).or_else(|| {
        decline("e:death-point");
        None
    })?;
    Some(Candidate {
        root,
        members,
        death,
    })
}

/// Gate (a): candidate roots — every FRESH heap-`Construct` dst:
///  - a collection buffer `Construct { ctor: ListLiteral | MapLiteral |
///    SetLiteral }`, OR
///  - a sum-aggregate `Construct { ctor: EnumVariant }` (Option / Result / user
///    sum) whose dst is in `owned_vars_needing_rc` — the same family the
///    construct-fed dead-param scan admits (`is_sum_aggregate_construct_rep`).
///    The `owned_vars_needing_rc` gate excludes all-scalar-payload sum
///    instantiations (`Option<int>`: niche-packed, no RC header), keeping the
///    no-sink / dead-param treatment scoped to genuine heap lineages.
///
/// Plain `Struct` ctors stay DECLINED (no failing cell; a wider admission is
/// over-fire surface without evidence). A `Let { Literal::String }` heap-str body
/// is NOT a candidate (no `Construct` definer; string-literal lineages decline
/// naturally).
fn collect_fresh_construct_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Vec<ArcVarId> {
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral,
                    ..
                } => roots.push(*dst),
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } if owned_vars_needing_rc.contains(dst) => roots.push(*dst),
                _ => {}
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
) -> (Vec<ArcVarId>, FxHashSet<ArcVarId>) {
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
    // PROVABLY-FRESH result: the callee's contract proves the returned value is
    // a NEW allocation (`Unique` + `preserves_freshness`), never a same-buffer
    // view (`@substring` / `@repeat` are ttr / non-fresh and stay excluded).
    // Only this subset is safe for the NO-SINK edge-death claim — an edge
    // release on a view double-frees the viewed buffer. Spec: Annex E §AIMS
    // RL-2.
    let provably_fresh = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            c.return_info.uniqueness == crate::aims::lattice::Uniqueness::Unique
                && c.return_info.preserves_freshness
                && !c.params.iter().any(|p| p.transfers_through_return)
                && !c.params.iter().any(|p| p.iter_consumes)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    let mut fresh: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_vars_needing_rc.contains(dst) && owned_result_non_forwarder_non_iter(callee) {
                roots.push(*dst);
                if provably_fresh(callee) {
                    fresh.insert(*dst);
                }
            }
        }
    }
    (roots, fresh)
}

/// Gate (c): true iff any closure member is used at a BORROWED (non-owned) arg
/// position of an `Invoke` / `InvokeIndirect` terminator — the carrier whose
/// inline-before-terminator `BurdenDec` is the use-after-free.
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
