//! FRESH-sum LIVE-EXTRACT same-alloc lineage treatment (RL-1 + RL-2): the
//! match-extract sibling of `compute_construct_fed_dead_param_lineage`.
//! Shape, over-emission mechanism, cure, and admission gates are documented
//! on [`compute_fresh_sum_live_extract_lineage`].

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, ValueRepr};

use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use super::super::is_provably_scalar_repr;
use super::live_extract_site::{choose_release_site, release_site_sound};
use super::{function_used_vars, ForwarderReleasePos};

/// One root's candidate admission, held until the gate (g) disjointness
/// filter runs over the full candidate set.
struct Candidate {
    root: ArcVarId,
    members: FxHashSet<ArcVarId>,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    dec_var: ArcVarId,
}

/// One retain-aliasing root's candidate admission (the branchy per-path sibling
/// of [`Candidate`]): the same-alloc closure + its per-terminal-path placed
/// releases, held until the gate (g) disjointness filter runs.
struct RaCandidate {
    members: FxHashSet<ArcVarId>,
    releases: Vec<((usize, ForwarderReleasePos), ArcVarId)>,
}

/// Result of [`compute_fresh_sum_live_extract_lineage`]: the same-alloc
/// closure to suppress (Part A) + the single placed release (Part B).
pub(in crate::lower::burden_lower) struct FreshSumLiveExtractLineage {
    /// Every var in an admitted fresh-sum live-extract closure. All carry
    /// spurious keep-alive incs / misplaced releases on ONE allocation;
    /// removed from `owned_vars_needing_rc` so the sole release is the
    /// placed dec below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — exactly ONE whole-var `BurdenDec`
    /// per admitted closure, on the lineage var read at the final site,
    /// placed AFTER that read. Merged into the `forwarder_result_releases`
    /// emission surface (same `ForwarderReleasePos` placement contract).
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// RL-1 + RL-2 treatment for the FRESH-sum live-extract match shape
/// (`match result { Some(s) -> s, .. }` + downstream borrow-reads).
///
/// A FRESH niche-family sum allocation read through `Let { Var }` aliases +
/// a niche-payload `Project` borrow-view and EXTRACTED LIVE to a merge
/// block-param names ONE allocation across the whole closure (TF-4: `Project`
/// is a borrow; a niche-family sum carries no allocation of its own). Per
/// RL-1 NO duplication exists, yet the `use_counts >= 2` proxy classes the
/// Let-Var alias as a duplication and the FRESH-site result inc survives:
/// 2 spurious keep-alive incs + 2 misplaced releases (the sum's arm dec
/// before the live reads + the extract's last-use dec before its final
/// transitive read) net +1 — the caller-owned reference is never released
/// (`RL2_release_exactly_once` violated; leak).
///
/// The cure removes the WHOLE closure from `owned_vars_needing_rc` (both
/// incs + both misplaced releases) and emits EXACTLY ONE whole-var
/// `BurdenDec` on the lineage var read at the closure's execution-FINAL
/// borrow-read, placed AFTER that read (`RL2_dec_at_last_use` — no UAF). The
/// dec targets the final-read var (NOT the root): on a merge path where the
/// carrier param received a DIFFERENT allocation (the `None -> "fallback"`
/// arm's literal), the dec releases that path's allocation — per-path
/// correct where a root dec would no-op and leak the literal.
///
/// Admission gates (ALL must hold per closure; ANY failure declines the root
/// and keeps current behavior — the status-quo leak is FAR safer than an
/// over-fire UAF / double-free):
///  (a) FRESH sum root: a `Construct { ctor: EnumVariant }` dst, OR an
///      `Apply` / `Invoke` dst whose callee contract reports
///      `return_info.uniqueness ∈ {Unique, MaybeShared}` (the same condition
///      under which `fresh_site_burden_inc_dst` emits the spurious result
///      inc) with NO `transfers_through_return` param (a forwarder result is
///      the ARG's allocation — owned by the forwarder scans, disjoint family).
///  (b) root in `owned_vars_needing_rc` (heap-carrying; auto-declines roots
///      claimed by the construct-fed suppression, which runs first) with
///      `Aggregate` repr (an `RcPointer` sum is a BOXED wrapper needing its
///      own release — out of family).
///  (c) niche-family sum type ([`is_niche_family_sum`]): the wrapper's RC
///      identity IS the single live payload, so one release frees the web.
///  (d) vetted same-alloc closure ([`same_alloc_closure_vetted`]): every
///      member use is a borrow-read; any consume / store / capture /
///      `Select`-`IsShared`-`Reset`-`Reuse` use / `Return` / indirect-call
///      arg / non-scalar borrowed-call result declines.
///  (e) at least one closure member is a LIVE block-param (the live extract;
///      a dead-params-only closure is the RL-5 dead-param family).
///  (f) execution-final single release ([`choose_release_site`] +
///      [`release_site_sound`]): the dec lands after the closure's final
///      borrow-read on every normal exit; unwind paths (`Resume`) are exempt
///      (status-quo leak there, no new double-free — the closure carries no
///      other release).
///  (g) pairwise-DISJOINT closures: two admitted roots whose closures share
///      any member converge on ONE per-path allocation web (both match arms
///      `Construct` into the same merge carrier — the caught-panic
///      `catch(expr:)` Ok/Err shape); each admission would place its own dec
///      at the shared final read, double-freeing the web. ALL roots of an
///      overlapping group decline (status quo preserved).
pub(in crate::lower::burden_lower) fn compute_fresh_sum_live_extract_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
) -> FreshSumLiveExtractLineage {
    let mut out = FreshSumLiveExtractLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    let preds = compute_predecessors(func);

    // Per-root candidate admissions; gate (g) filters before application.
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut claimed_roots: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in collect_fresh_sum_roots(func, contracts) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "fresh-sum live-extract root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed + by-value sum repr.
        if !owned_vars_needing_rc.contains(&root)
            || claimed_roots.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::Aggregate))
        {
            decline("b:owned/claimed/repr");
            continue;
        }
        // Gate (c): niche-family sum burden.
        if !is_niche_family_sum(func, root, type_registry) {
            decline("c:niche-family-sum");
            continue;
        }
        // Gate (d): vetted same-alloc closure.
        let Some(members) = same_alloc_closure_vetted(func, root) else {
            decline("d:closure-vet");
            continue;
        };
        // Gate (e): a LIVE block-param member (the live extract).
        let has_live_param = func.blocks.iter().any(|b| {
            b.params
                .iter()
                .any(|&(p, _)| members.contains(&p) && used.contains(&p))
        });
        if !has_live_param {
            decline("e:live-extract-param");
            continue;
        }
        // Gate (f): the placed single release.
        let Some((site_block, site_pos, dec_var)) = choose_release_site(func, &members) else {
            decline("f:release-site");
            continue;
        };
        if !release_site_sound(func, &members, root, site_block, site_pos, &preds) {
            decline("f:site-soundness");
            continue;
        }
        claimed_roots.extend(members.iter().copied());
        candidates.push(Candidate {
            root,
            members,
            site_block,
            site_pos,
            dec_var,
        });
    }

    // Gate (g): decline EVERY candidate whose closure overlaps another
    // candidate's closure — overlapping closures name one per-path allocation
    // web and each would place its own dec at the shared final read.
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
                gate = "g:closure-overlap",
                "fresh-sum live-extract root declined"
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

/// RL-1 + RL-2 + RL-4 treatment for the BRANCHY RETAIN-ALIASING shape
/// (`let v = m[k]; if v.is_some() && v.unwrap().starts_with(..)`).
///
/// A FRESH niche-family sum (`@__index` `Option<str>` result) whose payload is
/// read through `Let { Var }` aliases + niche-payload `Project` views AND
/// through an ACCESSOR-RETAIN call (`@unwrap`/`@get`, a builtin in
/// [`accessor_retain_builtin_names`](crate::borrow::accessor_retain_builtin_names)
/// that self-increments its extracted view — TF-4, the SAME allocation) across
/// a SHORT-CIRCUIT branchy CFG. The base walk emits the spurious dup-alias
/// keep-alive incs + an INLINE source dec BEFORE the accessor-retain call reads
/// the payload → use-after-free + double-free (exit -134:
/// `coll_map_index_int_str` / `set_str_union` / catch-cohort).
///
/// The single-site [`compute_fresh_sum_live_extract_lineage`] is FORECLOSED
/// here (ledger 212): the allocation is live on the SHORT-CIRCUIT-BYPASS branch
/// (`v.is_some() && …`: the `None`/`false` arm reaches `Return` WITHOUT passing
/// the `Some`-path reader), so no single site dominates every normal exit. This
/// consumer reuses the SHARED MULTI-EXIT per-path placement
/// ([`super::multi_exit_borrow_view::place_per_path_releases_with`]).
///
/// MULTI-REFERENCE placement (ledger 214): the accessor-retain chain holds ONE
/// retained reference PER hop on the SAME allocation — the niche-sum root (the
/// `@__index` self-inc on the Option payload) AND each accessor-retain result
/// (the `@unwrap`/`@get` self-inc on its extracted view). A single root-dec
/// releases ONLY the Option-payload reference and LEAKS the `@unwrap` reference.
/// The cure places a per-path release for EACH reference root (the niche-sum
/// root + every accessor-retain result member): RL-2 after the final read on
/// each READING path, RL-4 edge dec on each BYPASS edge — exactly-once-per-path
/// PER reference.
///
/// Admission mirrors [`compute_fresh_sum_live_extract_lineage`] gates
/// (a)/(b)/(c) EXCEPT: the closure grows across accessor-retain results, the
/// all-borrow-reads vet admits an accessor-retain non-scalar result that is
/// itself a closure member (the payload view), the gate is "carries a borrow-
/// read" (the accessor-retain extract), and the per-path multi-reference
/// placement REPLACES the single-site gate (f). Pairwise-disjoint over the
/// candidate set. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4 + TF-4.
pub(in crate::lower::burden_lower) fn compute_retain_aliasing_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> FreshSumLiveExtractLineage {
    let mut out = FreshSumLiveExtractLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let accessor_retain_names = crate::borrow::accessor_retain_builtin_names(interner);
    let preds = compute_predecessors(func);

    let mut candidates: Vec<RaCandidate> = Vec::new();
    let mut claimed: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in collect_fresh_sum_roots(func, contracts) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "retain-aliasing root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed + by-value sum repr.
        if !owned_vars_needing_rc.contains(&root)
            || claimed.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::Aggregate))
        {
            decline("b:owned/claimed/repr");
            continue;
        }
        // Gate (c): niche-family sum burden.
        if !is_niche_family_sum(func, root, type_registry) {
            decline("c:niche-family-sum");
            continue;
        }
        // Gate (d): vetted same-alloc closure (accessor-retain results extend it).
        let Some(members) = retain_aliasing_closure_vetted(func, root, &accessor_retain_names)
        else {
            decline("d:closure-vet");
            continue;
        };
        // Gate (e'): at least one ACCESSOR-RETAIN result member (the
        // retain-aliasing shape; a closure with no accessor-retain read is the
        // live_extract / multi-exit family, owned by their own scans).
        if !members
            .iter()
            .any(|m| accessor_retain_result(func, *m, &accessor_retain_names))
        {
            decline("e:no-accessor-retain");
            continue;
        }
        // Gate (e2): single-site FORECLOSURE — this per-path consumer fires ONLY
        // when the single-site placement is structurally impossible (the branchy
        // reader-BYPASS shape: a normal-exit Return reachable from the root def
        // WITHOUT passing the lineage's reader, so no single site dominates every
        // normal exit — coll_map's `if v.is_some() && v.unwrap()...`). A
        // STRAIGHT-LINE accessor-retain (`let v = o.unwrap(); v == ..`) HAS a
        // viable single dominating site and is already balanced by the existing
        // machinery (RL-1 inc-elision + base-walk last-use dec) — firing the
        // per-path multi-reference treatment there DOUBLE-treats and regresses
        // it (the wrapper_rc_retain / predicate_stack_probe accessor-retain
        // cells). Decline when single-site is viable; defer to the status quo.
        let single_site_viable = choose_release_site(func, &members)
            .is_some_and(|(b, pos, _)| release_site_sound(func, &members, root, b, pos, &preds));
        if single_site_viable {
            decline("e2:single-site-viable");
            continue;
        }
        // Gate (f'): per-path MULTI-REFERENCE placement over DISJOINT live
        // ranges. Each retained reference (the niche-sum root for the `@__index`
        // self-inc + each accessor-retain result for its `@unwrap`/`@get`
        // self-inc) has its OWN live range: the root ref is live across its
        // Let-aliases and DIES at the accessor-retain call that reads it; the
        // accessor-retain result is BORN at that call and dies at ITS last read.
        // Running placement over the WHOLE `members` per ref over-emits — it would
        // dec the root at the accessor-retain RESULT's last-read site (root
        // already dead there) and dec the result on a BYPASS path where it was
        // never born (a use-before-def → malformed IR). Each ref is therefore
        // placed over its OWN sub-closure (the ref var + the alias/projection
        // members reachable WITHOUT crossing another accessor-retain hop), with
        // that sub-closure's reads as the final-read set. Spec: Annex E §AIMS
        // RL-2 + RL-4.
        let releases =
            place_retain_aliasing_per_reference(func, root, &members, &accessor_retain_names);
        if releases.is_empty() {
            decline("f:no-placement");
            continue;
        }
        claimed.extend(members.iter().copied());
        candidates.push(RaCandidate { members, releases });
    }

    // Gate (g): decline EVERY candidate overlapping another's closure.
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
                gate = "g:closure-overlap",
                "retain-aliasing root declined"
            );
            continue;
        }
        out.suppressed_lineage_vars
            .extend(cand.members.iter().copied());
        for (site, var) in cand.releases {
            out.releases.entry(site).or_default().push(var);
        }
    }
    out
}

/// Gate (d) for the retain-aliasing consumer: grow the same-alloc closure over
/// `Let { Var }` aliases, niche-payload `Project` views, `Jump`-arg →
/// block-param hops, AND accessor-retain (`@unwrap`/`@get`) results (the
/// retain-aliasing payload view sharing the sum's allocation). Then vet every
/// member use as a borrow-read. `None` on any vet failure.
fn retain_aliasing_closure_vetted(
    func: &ArcFunction,
    root: ArcVarId,
    accessor_retain_names: &FxHashSet<Name>,
) -> Option<FxHashSet<ArcVarId>> {
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
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value)
                            && !is_provably_scalar_repr(func, *dst)
                            && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    // Accessor-retain growth: a non-scalar `Apply` result whose
                    // callee self-increments a borrow-view of a member receiver
                    // is the SAME allocation.
                    ArcInstr::Apply {
                        dst,
                        func: callee,
                        args,
                        ..
                    } if accessor_retain_names.contains(callee)
                        && args.first().is_some_and(|a| members.contains(a))
                        && !is_provably_scalar_repr(func, *dst)
                        && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
            // Accessor-retain growth at a terminator-`Invoke` result.
            if let ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } = &block.terminator
            {
                if accessor_retain_names.contains(callee)
                    && args.first().is_some_and(|a| members.contains(a))
                    && !is_provably_scalar_repr(func, *dst)
                    && members.insert(*dst)
                {
                    grew = true;
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
    retain_aliasing_member_uses_vetted(func, &members, accessor_retain_names).then_some(members)
}

/// Gate (d) vetting core: every member use is a borrow-read — a balanced
/// keep-alive `BurdenInc`/`BurdenDec`, a `Let`/`Project` closure hop, or an
/// `Apply`/`Invoke` member read at a borrowed position whose result is provably
/// scalar OR the accessor-retain payload view (a closure member). Any
/// owned-consume position / store / capture / `Set`/`Reuse`/`PartialApply`/
/// `ApplyIndirect` / non-`Invoke` terminator escape declines.
fn retain_aliasing_member_uses_vetted(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    accessor_retain_names: &FxHashSet<Name>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().iter().any(|v| members.contains(v)) {
                continue;
            }
            match instr {
                // Balanced keep-alive RC ops + closure-internal alias/projection
                // hops are not consumes — they keep the lineage same-alloc.
                ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
                | ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. } => {}
                ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Select { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. }
                | ArcInstr::PartialApply { .. }
                | ArcInstr::ApplyIndirect { .. } => return false,
                ArcInstr::Apply {
                    dst, func: callee, ..
                } => {
                    // A member at an owned position is a transfer out of family.
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                    // An `Apply` whose result `dst` is itself a member is a
                    // borrow-VIEW into the closure: vetted ONLY when the callee
                    // is accessor-retain (the result shares the payload alloc).
                    // A non-accessor-retain member-producing `Apply` is an
                    // un-vetted alias — decline.
                    if members.contains(dst) && !accessor_retain_names.contains(callee) {
                        return false;
                    }
                }
                _ => {
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return false;
                        }
                    }
                }
            }
        }
        let term = &block.terminator;
        if !term.used_vars().iter().any(|v| members.contains(v)) {
            continue;
        }
        match term {
            ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
            ArcTerminator::Invoke {
                dst, func: callee, ..
            } => {
                // A member at an owned `Invoke` arg = transfer — decline.
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
                // A member-producing terminator-`Invoke` is vetted ONLY when the
                // callee is accessor-retain (the result is the payload view).
                if members.contains(dst) && !accessor_retain_names.contains(callee) {
                    return false;
                }
            }
            ArcTerminator::InvokeIndirect { .. }
            | ArcTerminator::Return { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. } => return false,
        }
    }
    true
}

/// Gate (f') per-path MULTI-REFERENCE placement: one per-path release set per
/// retained reference (the niche-sum root for the `@__index` self-inc + each
/// accessor-retain result for its `@unwrap`/`@get` self-inc), each placed over
/// the reference's OWN disjoint-live-range sub-closure
/// ([`retain_aliasing_ref_sub_closure`]) via the shared MULTI-EXIT placement
/// core. Spec: Annex E §AIMS RL-2 + RL-4.
fn place_retain_aliasing_per_reference(
    func: &ArcFunction,
    root: ArcVarId,
    members: &FxHashSet<ArcVarId>,
    accessor_retain_names: &FxHashSet<Name>,
) -> Vec<((usize, ForwarderReleasePos), ArcVarId)> {
    let reference_roots: Vec<ArcVarId> = std::iter::once(root)
        .chain(
            members
                .iter()
                .copied()
                .filter(|m| *m != root && accessor_retain_result(func, *m, accessor_retain_names)),
        )
        .collect();
    let mut releases: Vec<((usize, ForwarderReleasePos), ArcVarId)> = Vec::new();
    for ref_root in &reference_roots {
        let sub_closure =
            retain_aliasing_ref_sub_closure(func, *ref_root, members, accessor_retain_names);
        releases.extend(super::multi_exit_borrow_view::place_per_path_releases_with(
            func,
            *ref_root,
            &sub_closure,
            ra_body_read,
            ra_term_read,
        ));
    }
    releases
}

/// The per-reference sub-closure of `ref_root` within the full retain-aliasing
/// `members` closure: `ref_root` plus the `Let { Var }` alias + niche-payload
/// `Project` members reachable from it WITHOUT crossing another accessor-retain
/// hop (each accessor-retain result is a DISTINCT reference with its own
/// sub-closure, so it is NOT pulled into `ref_root`'s). This bounds a
/// reference's live range to its own use set: the niche-sum root's sub-closure
/// dies at the accessor-retain call that reads it; an accessor-retain result's
/// sub-closure (just itself + its own aliases) is born at its call and dies at
/// its last read. Per-ref placement over THIS set releases each reference
/// exactly once at its own last use — never on a path where it is unborn/dead.
fn retain_aliasing_ref_sub_closure(
    func: &ArcFunction,
    ref_root: ArcVarId,
    members: &FxHashSet<ArcVarId>,
    accessor_retain_names: &FxHashSet<Name>,
) -> FxHashSet<ArcVarId> {
    let mut sub: FxHashSet<ArcVarId> = FxHashSet::default();
    sub.insert(ref_root);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if sub.contains(src)
                        && members.contains(dst)
                        // Stop at a different reference: an accessor-retain
                        // result is its OWN sub-closure root.
                        && !accessor_retain_result(func, *dst, accessor_retain_names)
                        && sub.insert(*dst) =>
                    {
                        grew = true;
                    }
                    ArcInstr::Project { dst, value, .. }
                        if sub.contains(value)
                            && members.contains(dst)
                            && !is_provably_scalar_repr(func, *dst)
                            && !accessor_retain_result(func, *dst, accessor_retain_names)
                            && sub.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                for (pos, &arg) in args.iter().enumerate() {
                    if sub.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            if members.contains(&param)
                                && !accessor_retain_result(func, param, accessor_retain_names)
                                && sub.insert(param)
                            {
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
    sub
}

/// Body final-read predicate for [`compute_retain_aliasing_lineage`]'s per-path
/// placement: a member read at a BORROWED (non-owned) `Apply` arg position. The
/// accessor-retain result is itself a member (closure-grown), so a downstream
/// borrow of it (`@starts_with(view [borrow])`) is a member read too.
fn ra_body_read(instr: &ArcInstr, members: &FxHashSet<ArcVarId>) -> Option<ArcVarId> {
    match instr {
        ArcInstr::Apply { .. } => instr.used_vars().iter().enumerate().find_map(|(pos, &v)| {
            (members.contains(&v) && !instr.is_owned_position(pos)).then_some(v)
        }),
        _ => None,
    }
}

/// Terminator analogue of [`ra_body_read`]: a member read at a BORROWED
/// terminator-`Invoke` arg position.
fn ra_term_read(term: &ArcTerminator, members: &FxHashSet<ArcVarId>) -> Option<ArcVarId> {
    match term {
        ArcTerminator::Invoke { .. } => {
            term.used_vars().iter().enumerate().find_map(|(pos, &v)| {
                (members.contains(&v) && !term.is_owned_position(pos)).then_some(v)
            })
        }
        _ => None,
    }
}

/// True iff `var` is the result of an ACCESSOR-RETAIN call (`@unwrap`/`@get` —
/// a builtin in `accessor_retain_names` that self-increments its extracted
/// view). Such a result is a DISTINCT retained reference on the shared
/// allocation (a per-hop self-inc), owing its own per-path release.
fn accessor_retain_result(
    func: &ArcFunction,
    var: ArcVarId,
    accessor_retain_names: &FxHashSet<Name>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *dst == var {
                    return accessor_retain_names.contains(callee);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if *dst == var {
                return accessor_retain_names.contains(callee);
            }
        }
    }
    false
}

/// Candidate FRESH-sum roots per gate (a): sum-aggregate `Construct` dsts +
/// `Apply` / `Invoke` results whose callee contract hands the caller an owned
/// reference (`uniqueness ∈ {Unique, MaybeShared}`) without transferring an
/// arg through the return.
fn collect_fresh_sum_roots(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<ArcVarId> {
    let owned_result_non_forwarder = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            matches!(
                c.return_info.uniqueness,
                Uniqueness::Unique | Uniqueness::MaybeShared
            ) && !c.params.iter().any(|p| p.transfers_through_return)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } => roots.push(*dst),
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if owned_result_non_forwarder(callee) => roots.push(*dst),
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_result_non_forwarder(callee) {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Gate (c): the root's type is a niche-family sum — variant entries present,
/// no self heap allocation, no struct fields, no element burden, every variant
/// at most ONE owned payload (transfer-on-match binding or retained field).
/// The wrapper then carries no allocation of its own and its RC identity is
/// the single live payload — one release frees the whole web.
fn is_niche_family_sum(func: &ArcFunction, root: ArcVarId, type_registry: &TypeRegistry) -> bool {
    let ty: TypeRef = idx_to_type_ref(func.var_types[root.index()], type_registry);
    let Some(burden) = lookup_burden(ty, type_registry) else {
        return false;
    };
    if burden.self_heap_alloc()
        || burden.owned_fields().next().is_some()
        || burden.element_burden().is_some()
    {
        return false;
    }
    let mut any_variant = false;
    for v in burden.variant_burdens() {
        any_variant = true;
        if v.transfers_on_match.len() + v.retained_owned.len() > 1 {
            return false;
        }
    }
    any_variant
}

/// Gate (d): grow the same-alloc closure from `root` (Let-Var aliases +
/// non-scalar `Project` borrow-views + `Jump`-arg → block-param hops), then
/// vet every member use as a pure borrow-read. `None` on any vet failure.
fn same_alloc_closure_vetted(func: &ArcFunction, root: ArcVarId) -> Option<FxHashSet<ArcVarId>> {
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
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    // Non-scalar Project dsts are the niche payload
                    // borrow-views sharing the allocation (TF-4); scalar tag
                    // reads drop out of the closure.
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

    member_uses_all_borrow_reads(func, &members).then_some(members)
}

/// Gate (d) vetting core: true iff EVERY use of every closure member is a
/// pure borrow-read (no consume / store / capture / COW-machinery use /
/// escape / non-scalar borrowed-call result).
fn member_uses_all_borrow_reads(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches_member = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches_member {
                continue;
            }
            match instr {
                // Alias hops + borrow-view projections are the closure's own
                // edges (scalar tag projections are borrow-reads too).
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. } => {}
                // COW / conditional-alias / mutation / reuse machinery on a
                // member is a distinct sub-root (the Select-branch shape
                // double-frees under a single-release treatment); a closure
                // capture (`PartialApply`) retains a reference — a genuine
                // duplication; an indirect call (`ApplyIndirect`) has no
                // contract to vet. Decline all.
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
                    // A borrowed read must provably NOT alias the member into
                    // its result: require a provably-scalar result. The
                    // protocol builtins (`__index` self-inc, `iter` consume)
                    // and user callees returning heap all decline.
                    if !is_provably_scalar_repr(func, *dst) {
                        return false;
                    }
                }
                // Owned-position consume at any other instruction = transfer;
                // a list-concat `PrimOp Binary(Add)` consumes its `RcPointer`
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
            // Jump hops are the closure's own param edges; Resume/Unreachable
            // use nothing.
            ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
            ArcTerminator::Invoke { dst, .. } => {
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return false;
                    }
                }
                // Borrowed terminator read: same provably-scalar-result vet
                // as the body `Apply` arm.
                if !is_provably_scalar_repr(func, *dst) {
                    return false;
                }
            }
            // Escapes / unvettable consumers.
            ArcTerminator::InvokeIndirect { .. }
            | ArcTerminator::Return { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. } => return false,
        }
    }
    true
}
