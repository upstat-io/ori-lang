//! `ori_catch_recover` recovered-message release (RL-2): the missing normal-path
//! release for a `catch(panic(...))` recovered `str` whose `Result::Err(msg)`
//! payload is match-extracted + borrow-read but never released on the normal
//! path. Shape, over-emission mechanism, cure, and admission gates are
//! documented on [`compute_catch_recover_release_lineage`].

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

use super::super::is_provably_scalar_repr;
use super::{successor_reachable_blocks, ForwarderReleasePos};

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

/// Gate (d): true iff any existing `BurdenDec` on a closure member is
/// forward-reachable to a `Return`-terminated normal exit — i.e. the lineage
/// ALREADY has a normal-path release and must NOT receive another (double-free).
/// Only an unwind-only release (every dec reaches only `Resume` / `Unreachable`)
/// is the MISSING-release shape this scan cures.
fn lineage_has_normal_path_release(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let has_member_dec = block
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::BurdenDec { var } if members.contains(var)));
        if !has_member_dec {
            continue;
        }
        // The dec's block reaches a normal-exit Return (itself or via successors)?
        if block_reaches_return(func, block_idx) {
            return true;
        }
    }
    false
}

/// True iff `start` is `Return`-terminated OR a `Return`-terminated block is
/// forward-reachable from it (the dec releases on a path that ends at a normal
/// exit).
fn block_reaches_return(func: &ArcFunction, start: usize) -> bool {
    if matches!(
        func.blocks.get(start).map(|b| &b.terminator),
        Some(ArcTerminator::Return { .. })
    ) {
        return true;
    }
    successor_reachable_blocks(func, start).iter().any(|&b| {
        matches!(
            func.blocks.get(b).map(|b| &b.terminator),
            Some(ArcTerminator::Return { .. })
        )
    })
}

/// Gate (e): the execution-final GENUINE borrow-read site for the single placed
/// release. Collects every GENUINE read of a member — a borrowed `Apply`/`Invoke`
/// arg or a scalar-result borrowed call, EXCLUDING the closure's own edges
/// (`Let { Var }` hops, niche-payload `Project` views, the `Construct` wrap,
/// `Jump` threading, and the keep-alive `BurdenInc`/`BurdenDec` churn) — then
/// picks the unique read every other genuine read reaches. Returns
/// `(block_idx, pos, read_var)`; the dec targets `read_var` (the member holding
/// the allocation at the site). Validates the site is single-pred (for a
/// `BlockEntry` site), not in a CFG cycle, and that every normal-exit `Return`
/// reachable from the root passes through it.
fn catch_recover_final_read_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    preds: &[Vec<usize>],
) -> Option<(usize, ForwarderReleasePos, ArcVarId)> {
    // Candidate GENUINE reads: (block, pos, read var, in-block order; -1 = entry).
    let mut candidates: Vec<(usize, ForwarderReleasePos, ArcVarId, isize)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            // Skip the closure's own edges + keep-alive churn — NOT genuine reads.
            match instr {
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. }
                | ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. } => continue,
                ArcInstr::Construct {
                    ctor: CtorKind::EnumVariant { .. },
                    dst,
                    ..
                } if members.contains(dst) => continue,
                _ => {}
            }
            if let Some(&used) = instr.used_vars().iter().find(|v| members.contains(v)) {
                let order = isize::try_from(instr_idx).unwrap_or(isize::MAX);
                candidates.push((
                    block_idx,
                    ForwarderReleasePos::AfterInstr(instr_idx),
                    used,
                    order,
                ));
            }
        }
        let term = &block.terminator;
        if let ArcTerminator::Invoke { normal, .. } = term {
            for (pos, &v) in term.used_vars().iter().enumerate() {
                if members.contains(&v) && !term.is_owned_position(pos) {
                    candidates.push((normal.index(), ForwarderReleasePos::BlockEntry, v, -1));
                }
            }
        }
    }
    // Pick the unique genuine read that every other genuine read reaches
    // (execution-final). Block INDEX order is not execution order, so use CFG
    // forward-reachability.
    let mut reachable: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut reaches = |from: usize, to: usize| -> bool {
        reachable
            .entry(from)
            .or_insert_with(|| successor_reachable_blocks(func, from))
            .contains(&to)
    };
    let mut chosen: Option<(usize, ForwarderReleasePos, ArcVarId)> = None;
    'outer: for cand in &candidates {
        for other in &candidates {
            if std::ptr::eq(cand, other) {
                continue;
            }
            let after_other = if other.0 == cand.0 {
                cand.3 >= other.3
            } else {
                reaches(other.0, cand.0)
            };
            if !after_other {
                continue 'outer;
            }
        }
        chosen = Some((cand.0, cand.1, cand.2));
        break;
    }
    let (site_block, site_pos, dec_var) = chosen?;
    catch_recover_site_sound(func, members, root, site_block, site_pos, preds)
        .then_some((site_block, site_pos, dec_var))
}

/// Gate (e) site-soundness: the placed release site is execution-final, covers
/// every normal exit, is not in a CFG cycle (no re-reached double-free), and a
/// `BlockEntry` site is single-pred so the predecessor's terminator read
/// dominates the dec.
fn catch_recover_site_sound(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    preds: &[Vec<usize>],
) -> bool {
    // (e1) the site must not sit in a CFG cycle (a re-reached dec double-frees).
    if successor_reachable_blocks(func, site_block).contains(&site_block) {
        return false;
    }
    // (e2) a BlockEntry site must have a single predecessor so the var read at the
    // predecessor's terminator dominates the dec.
    if site_pos == ForwarderReleasePos::BlockEntry && preds.get(site_block).map(Vec::len) != Some(1)
    {
        return false;
    }
    // (e3) the site must be forward-reachable from every GENUINE-read block
    // (execution-final). Loop-carried keep-alive churn is NOT a genuine read; the
    // genuine-read set is the body `Apply`/`Invoke`-borrow uses (excluding the
    // closure's own edges), so the loop body's keep-alive ops do not constrain the
    // site.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let block_genuine_reads_member = block_has_genuine_member_read(block, members);
        if block_genuine_reads_member
            && block_idx != site_block
            && !successor_reachable_blocks(func, block_idx).contains(&site_block)
        {
            return false;
        }
    }
    // (e4) every normal exit (`Return`-terminated block) reachable from the root's
    // definition passes through the site — a bypassing normal path would never
    // release the allocation. Unwind exits (`Resume`) are exempt (status-quo leak
    // on unwind, identical to today's behavior).
    let def_block = catch_recover_root_def_block(func, root);
    let mut stack: Vec<usize> = vec![def_block];
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    while let Some(b) = stack.pop() {
        if b == site_block || !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        if matches!(block.terminator, ArcTerminator::Return { .. }) {
            return false;
        }
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    true
}

/// True iff `block` carries a GENUINE borrow-read of a member — a body
/// `Apply`/`Invoke` borrow or a borrowed terminator-`Invoke` arg — EXCLUDING the
/// closure's own edges (`Let { Var }` hops, `Project` views, the `Construct`
/// wrap, `Jump` threading) and the keep-alive `BurdenInc`/`BurdenDec` churn.
fn block_has_genuine_member_read(
    block: &crate::ir::ArcBlock,
    members: &FxHashSet<ArcVarId>,
) -> bool {
    for instr in &block.body {
        match instr {
            ArcInstr::Let {
                value: ArcValue::Var(_),
                ..
            }
            | ArcInstr::Project { .. }
            | ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::Construct { .. } => continue,
            _ => {}
        }
        if instr.used_vars().iter().any(|v| members.contains(v)) {
            return true;
        }
    }
    if let ArcTerminator::Invoke { .. } = &block.terminator {
        let term = &block.terminator;
        for (pos, v) in term.used_vars().iter().enumerate() {
            if members.contains(v) && !term.is_owned_position(pos) {
                return true;
            }
        }
    }
    false
}

/// The block where `root`'s value becomes live: the defining block for a body
/// `Apply`; the NORMAL successor for a terminator `Invoke` (the result binds on
/// the normal edge). Falls back to the entry block.
fn catch_recover_root_def_block(func: &ArcFunction, root: ArcVarId) -> usize {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let ArcInstr::Apply { dst, .. } = instr {
                if *dst == root {
                    return block_idx;
                }
            }
        }
        if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
            if *dst == root {
                return normal.index();
            }
        }
    }
    func.entry.index()
}
