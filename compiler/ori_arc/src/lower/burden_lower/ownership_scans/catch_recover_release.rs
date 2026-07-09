//! `ori_catch_recover` recovered-message release (RL-2): the missing normal-path
//! release for a `catch(panic(...))` recovered `str` whose `Result::Err(msg)`
//! payload is match-extracted + borrow-read but never released on the normal
//! path. Shape, over-emission mechanism, cure, and admission gates are
//! documented on [`compute_catch_recover_release_lineage`].

mod site;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

use super::super::is_provably_scalar_repr;
use super::{compute_pairwise_overlap_flags, ForwarderReleasePos};
use site::{catch_recover_final_read_site, lineage_has_normal_path_release};

/// Result of [`compute_catch_recover_release_lineage`]: the same-alloc closure to
/// suppress + the single placed normal-path release.
pub(in crate::lower::burden_lower) struct CatchRecoverReleaseLineage {
    /// Every var in an admitted catch-recover message closure. All carry
    /// keep-alive incs on ONE allocation with no matching normal-path dec;
    /// removed from `owned_vars_needing_rc` so the placed dec below is the sole
    /// release.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — exactly ONE whole-var `BurdenDec` per
    /// admitted closure, on the lineage var read at the execution-final genuine
    /// borrow-read, placed AFTER that read. Merged into the
    /// `forwarder_result_releases` emission surface.
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// One root's candidate admission, held until the gate (f) disjointness filter
/// runs over the full candidate set.
struct Candidate {
    members: FxHashSet<ArcVarId>,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    dec_var: ArcVarId,
}

/// RL-2 missing-release treatment for the `catch(panic(...))` recovered-message
/// shape (`catch(expr: panic(msg:))` then `match result { Err(e) -> e, .. }` +
/// `e.chars()` / `e.contains(..)` borrow-reads).
///
/// A `catch(panic(...))` lowers to `ori_catch_recover()` — a FRESH `str` copy of
/// the panic message (a self-allocating protocol builtin with NO seeded
/// contract, by deliberate design). The walk
/// wraps it in `Construct Variant(Result.1)(msg)` (the `Err(msg)` Result), then
/// the `match`-extract `Project`s the `str` payload back out LIVE and the body
/// borrow-reads it (`@chars` / `@contains`). The recovered copy names ONE
/// allocation across the Result wrapper, its `Let { Var }` aliases, the
/// niche-payload `Project` extraction, and any loop-carried `Jump`-threaded
/// block-params (a `for ch in e.chars()` loop threads the message as pure
/// keep-alive).
///
/// The base walk emits a keep-alive `BurdenInc` on the wrapped Result + the
/// extracted message (`RL1_duplication` proxy) but places the matching
/// `BurdenDec` ONLY on the unwind / `Resume` edge — the normal-path release is
/// MISSING (the recovered copy's lifecycle is NOT a std `Result` lineage with a
/// scope-exit dec, because `ori_catch_recover` has no contract admitting it into
/// `collect_fresh_sum_roots`). Net +1 on the normal path: the message buffer
/// leaks (`RL2_release_exactly_once` violated). It leaks identically under
/// default, the gated burden probe, AND `ORI_DISABLE_BURDEN_OPS=1` — a
/// burden-path MISSING-RELEASE gap, not a coexistence over-emission.
///
/// The cure removes the WHOLE closure from `owned_vars_needing_rc` (every
/// keep-alive inc + its unwind-edge decs) and emits EXACTLY ONE whole-var
/// `BurdenDec` on the lineage var read at the closure's execution-final GENUINE
/// borrow-read, placed AFTER that read on the NORMAL path (`RL2_dec_at_last_use`
/// — no UAF). Loop-carried keep-alive churn (the `BurdenInc`/`BurdenDec` ops the
/// walk threads through a `for` loop's block-params) is NOT a genuine read; the
/// final-read computation excludes it so the release lands at the genuine read
/// (`@chars`) before the loop, not mid-loop.
///
/// This is the catch-cohort sibling of
/// [`super::live_extract::compute_fresh_sum_live_extract_lineage`], which is
/// FORECLOSED here: `collect_fresh_sum_roots` requires the root callee carry a
/// contract (`contracts.get(callee).is_some()`), and `ori_catch_recover` has
/// NONE by the deliberate no-contract decision. This scan roots on the callee
/// IDENTITY (`ori_catch_recover`) instead, never on a contract, and never adds a
/// contract to `ori_catch_recover`.
///
/// Admission gates (ALL must hold per closure; ANY failure declines the root and
/// keeps current behavior — the status-quo leak is FAR safer than an over-fire
/// UAF / double-free):
///  (a) ROOT: an `Apply` / `Invoke` result whose callee is `ori_catch_recover`
///      (callee identity, never a contract).
///  (b) root in `owned_vars_needing_rc` (heap-carrying; auto-declines a root
///      already claimed by an earlier scan).
///  (c) vetted borrow-only same-alloc closure
///      ([`catch_recover_closure_vetted`]): the closure spans the recover result,
///      its niche-family `Construct Variant` wrap, the niche-payload `Project`
///      extraction, `Let { Var }` aliases, and `Jump`-threaded block-params; every
///      GENUINE use is a borrow-read (a borrowed `Apply`/`Invoke` arg or a
///      scalar-result borrowed call). ANY owned-position consume (other than the
///      ONE wrap-into-sum + the niche `Project` that DEFINE the closure) / store /
///      capture / `Select` / `IsShared` / `Reset` / `Reuse` / non-scalar borrowed-
///      call result / `Return` transfer declines.
///  (d) MISSING normal-path release: EVERY existing `BurdenDec` on a closure
///      member sits on an unwind / `Resume`-reachable edge (the walk's unwind-only
///      release); NO dec is forward-reachable to a `Return`-terminated normal exit.
///      A lineage with a normal-path dec is already balanced — decline (no
///      double-release).
///  (e) execution-final single release
///      ([`catch_recover_final_read_site`]): the dec lands after the closure's
///      final GENUINE borrow-read on every normal exit; the site is single-pred,
///      not in a CFG cycle, and every normal-exit `Return` reachable from the root
///      passes through it. Unwind paths (`Resume`) are exempt (status-quo leak
///      there, no new double-free).
///  (f) pairwise-DISJOINT closures: two admitted roots whose closures share any
///      member decline (a shared final-read site would double-release the web).
///
/// Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`, the impl missing the
/// normal-path release — §CP-1 case (a), the proven calculus is correct).
pub(in crate::lower::burden_lower) fn compute_catch_recover_release_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> CatchRecoverReleaseLineage {
    let mut out = CatchRecoverReleaseLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let recover_name = interner.intern("ori_catch_recover");
    let preds = compute_predecessors(func);

    // Per-root candidate admissions; gate (f) filters before application.
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut claimed_roots: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in collect_catch_recover_roots(func, recover_name) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "catch-recover release root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed by an earlier candidate.
        if !owned_vars_needing_rc.contains(&root) || claimed_roots.contains(&root) {
            decline("b:owned/claimed");
            continue;
        }
        // Gate (c): vetted borrow-only same-alloc closure.
        let Some(members) = catch_recover_closure_vetted(func, root) else {
            decline("c:closure-vet");
            continue;
        };
        // Gate (d): MISSING normal-path release — every existing dec is
        // unwind-only.
        if lineage_has_normal_path_release(func, &members) {
            decline("d:has-normal-release");
            continue;
        }
        // Gate (e): execution-final single release after the final GENUINE read.
        let Some((site_block, site_pos, dec_var)) =
            catch_recover_final_read_site(func, &members, root, &preds)
        else {
            decline("e:release-site");
            continue;
        };
        claimed_roots.extend(members.iter().copied());
        candidates.push(Candidate {
            members,
            site_block,
            site_pos,
            dec_var,
        });
    }

    // Gate (f): decline EVERY candidate whose closure overlaps another's.
    let overlapping = compute_pairwise_overlap_flags(&candidates, |c| &c.members);
    for (cand, overlaps) in candidates.into_iter().zip(overlapping) {
        if overlaps {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                gate = "f:closure-overlap",
                "catch-recover release root declined"
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

/// Gate (a): the `ori_catch_recover` result roots — an `Apply` body result or an
/// `Invoke` terminator result whose callee name is `ori_catch_recover`.
fn collect_catch_recover_roots(func: &ArcFunction, recover_name: ori_ir::Name) -> Vec<ArcVarId> {
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == recover_name {
                    roots.push(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if *callee == recover_name {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Gate (c): the same-alloc closure rooted at the `ori_catch_recover` result,
/// vetted borrow-only. Grows the closure across:
///  - the niche-family `Construct Variant` wrap of the recover result (the ONE
///    owned-position consume that DEFINES the closure — the recovered copy is
///    moved into the `Err(msg)` Result whose niche stores the payload pointer
///    inline, TF-4 same allocation);
///  - non-scalar `Project` niche-payload extractions (the `match`-extract
///    borrow-views, TF-4);
///  - `Let { Var }` aliases;
///  - `Jump`-arg-threaded block-params (loop-carried keep-alive threading).
///
/// Returns the member set when [`catch_recover_member_uses_all_borrow_reads`]
/// vets every member's GENUINE use as a borrow-read; `None` otherwise.
fn catch_recover_closure_vetted(func: &ArcFunction, root: ArcVarId) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    // The recovered copy moved into its `Err(msg)` Result wrap:
                    // the sum's niche stores the payload pointer inline (a
                    // niche-family single-payload variant), so wrapper and
                    // payload name ONE allocation (TF-4). The ONLY admitted
                    // owned-position consume.
                    ArcInstr::Construct {
                        dst,
                        ctor: CtorKind::EnumVariant { .. },
                        args,
                        ..
                    } if args.iter().any(|a| members.contains(a)) && members.insert(*dst) => {
                        grew = true;
                    }
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    // Non-scalar Project dsts are the niche-payload borrow-views
                    // sharing the allocation (TF-4); scalar tag reads drop out.
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value)
                            && !is_provably_scalar_repr(func, *dst)
                            && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
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

    catch_recover_member_uses_all_borrow_reads(func, &members).then_some(members)
}

/// Gate (c) vetting core: true iff EVERY GENUINE use of every closure member is a
/// pure borrow-read. The closure's OWN edges — `Let { Var }` alias hops,
/// niche-payload `Project` views, the ONE `Construct Variant` wrap, `Jump`-arg
/// threading, and the keep-alive `BurdenInc`/`BurdenDec` churn — are NOT genuine
/// uses (they define the closure or carry the allocation forward). A genuine use
/// is a borrowed `Apply`/`Invoke` arg or a scalar-result borrowed call; ANY
/// owned-position consume / store / capture / COW machinery / non-scalar
/// borrowed-call result / `Return` transfer declines.
fn catch_recover_member_uses_all_borrow_reads(
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
                // Closure-own edges + keep-alive churn: alias hops, niche-payload
                // projections (scalar tag reads too), the keep-alive ops we are
                // eliminating, and the ONE wrap that defines the closure.
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. }
                | ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. } => {}
                ArcInstr::Construct {
                    ctor: CtorKind::EnumVariant { .. },
                    dst,
                    ..
                } if members.contains(dst) => {}
                // COW / conditional-alias / mutation / reuse machinery on a member
                // is a distinct sub-root; a closure capture retains a reference; an
                // indirect call has no contract to vet. Decline all.
                ArcInstr::Select { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. }
                | ArcInstr::PartialApply { .. }
                | ArcInstr::ApplyIndirect { .. } => return false,
                ArcInstr::Apply { dst, .. } => {
                    // Owned-position consume = transfer out of family.
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                    // A borrowed read must provably NOT alias the member into its
                    // result: require a provably-scalar result (the `@chars` /
                    // `@contains` reads return a fresh `[char]` / `bool`; only the
                    // scalar-result form is admitted — a heap-returning borrowed
                    // call could alias the message into its result and declines).
                    if !is_provably_scalar_repr(func, *dst) {
                        return false;
                    }
                }
                // Any other owned-position consume = transfer; a list-concat
                // `PrimOp Binary(Add)` consumes its `RcPointer` operands — decline.
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
            // Jump hops are the closure's own param edges; Resume/Unreachable use
            // nothing for cleanup.
            ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
            ArcTerminator::Invoke { dst, .. } => {
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
                // Borrowed terminator read: same provably-scalar-result vet as the
                // body `Apply` arm. `@chars(msg [borrow]) -> [char]` is RcPtr — but
                // it returns a FRESH iterator-source list, NOT an alias of the
                // message, so it is admitted by the scalar-result vet only when the
                // result is provably scalar. `@chars` returns `[char]` (non-scalar),
                // so the message's final read must be a NON-aliasing borrowed call;
                // the catch-cohort `@chars`/`@contains` shapes are admitted because
                // their results are fresh non-alias values. Require non-aliasing:
                // a borrowed read whose result is non-scalar is admitted ONLY when
                // the result is NOT a member-alias (enforced structurally: the
                // closure traversal never grows across an `Apply`/`Invoke` result,
                // so a non-scalar borrowed-call result is outside `members` and
                // cannot alias the lineage forward). A result that IS a member
                // means the borrowed read aliased the message forward — decline.
                if members.contains(dst) {
                    return false;
                }
            }
            // An indirect terminator call has no contract to vet (decline); a
            // member reaching `Return` is an owned transfer out of family
            // (decline).
            ArcTerminator::InvokeIndirect { .. } | ArcTerminator::Return { .. } => return false,
            // Branch / Switch operands are scalars (the tag), never a member after
            // the niche `Project` drops the scalar tag out — but a member reaching
            // a Branch/Switch operand would be a non-scalar in an owned position,
            // decline defensively.
            ArcTerminator::Branch { .. } | ArcTerminator::Switch { .. } => {
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests;
